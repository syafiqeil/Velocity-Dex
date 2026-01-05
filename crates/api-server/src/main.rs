// crates/api-server/src/main.rs

use tonic::{transport::Server, Request, Response, Status};
use tokio::sync::{mpsc, oneshot};
use engine_core::processor::{MarketProcessor, Command};
use engine_core::{Side as EngineSide, EngineEvent};
use trading::trading_engine_server::{TradingEngine, TradingEngineServer};
use trading:: {
    PlaceOrderRequest, PlaceOrderResponse, CancelOrderRequest, CancelOrderResponse, 
    DepthRequest, DepthResponse, OrderLevel as ProtoOrderLevel, TradeExecution, Side as ProtoSide
};

pub mod trading {
    tonic::include_proto!("trading");
}

// Struct Service gRPC
pub struct TradingService {
    // Channel untuk mengirim command ke MarketProcessor (Actor)
    processor_sender: mpsc::Sender<Command>,
}

#[tonic::async_trait]
impl TradingEngine for TradingService {
    async fn place_limit_order(
        &self,
        request: Request<PlaceOrderRequest>,
    ) -> Result<Response<PlaceOrderResponse>, Status> {
        let req = request.into_inner();

        // 1. Validasi & Konversi Input (Proto -> Internal)
        let side = match ProtoSide::try_from(req.side).unwrap_or(ProtoSide::Unspecified) {
            ProtoSide::Bid => EngineSide::Bid,
            ProtoSide::Ask => EngineSide::Ask,
            ProtoSide::Unspecified => return Err(Status::invalid_argument("Side is required")),
        };

        // 2. Siapkan Response Channel (One-Shot)
        let (resp_tx, resp_rx) = oneshot::channel();

        // 3. Kirim Command ke Engine
        let command = Command::PlaceOrder {
            user_id: req.user_id,
            order_id: req.order_id,
            side,
            price: req.price,
            quantity: req.quantity,
            responder: resp_tx,
        };

        // Kirim ke actor (jika channel penuh/tutup, berarti engine mati)
        self.processor_sender
            .send(command)
            .await
            .map_err(|_| Status::internal("Engine is down"))?;

        // 4. Tunggu Hasil dari Engine
        let events = resp_rx.await.map_err(|_| Status::internal("Engine failed to respond"))?;

        // 5. Konversi Event Engine ke Response Proto
        let mut fills = Vec::new();
        let mut success = false;

        for event in events {
            match event {
                EngineEvent::OrderPlaced { id, .. } if id == req.order_id => {
                    success = true; // Order masuk book (Maker)
                }
                EngineEvent::TradeExecuted { maker_id, taker_id, price, quantity } => {
                    // Jika kita adalah taker, catat eksekusi ini
                    if taker_id == req.order_id {
                        fills.push(TradeExecution {
                            maker_order_id: maker_id,
                            price,
                            quantity,
                        });
                        success = true; // Terjadi trade (Taker)
                    }
                }
                EngineEvent::OrderCancelled { .. } => {
                    // Handle jika perlu (misal STP)
                }
                _ => {}
            }
        }

        Ok(Response::new(PlaceOrderResponse {
            success,
            message: if success { "Order Processed".to_string() } else { "Order Rejected".to_string() },
            fills,
        }))
    }

    async fn cancel_order(
        &self,
        request: Request<CancelOrderRequest>,
    ) -> Result<Response<CancelOrderResponse>, Status> {
        let req = request.into_inner();
        let (resp_tx, resp_rx) = oneshot::channel();

        // 1. Kirim Command ke Actor
        self.processor_sender
            .send(Command::CancelOrder {
                user_id: req.user_id,
                order_id: req.order_id,
                responder: resp_tx,
            })
            .await
            .map_err(|_| Status::internal("Engine down"))?;

        // 2. Tunggu hasil
        let events = resp_rx.await.map_err(|_| Status::internal("No response"))?;

        // 3. Cek apakah ada event OrderCancelled
        let success = events.iter().any(|e| matches!(e, EngineEvent::OrderCancelled { .. }));

        Ok(Response::new(CancelOrderResponse {
            success,
            remaining_qty: 0, // Opsional: bisa diimprovisasi untuk return sisa qty
        }))
    }

    async fn get_order_book_depth(
        &self,
        request: Request<DepthRequest>,
    ) -> Result<Response<DepthResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit == 0 { 10 } else { req.limit as usize };

        let (resp_tx, resp_rx) = oneshot::channel();

        // Kirim command ke Engine Actor
        self.processor_sender
            .send(Command::GetDepth {
                limit,
                responder: resp_tx,
            })
            .await
            .map_err(|_| Status::internal("Engine down"))?;

        // Tunggu hasil (Sync operation di dalam Actor sangat cepat)
        let (asks, bids) = resp_rx.await.map_err(|_| Status::internal("No response"))?;

        // Mapping dari Engine struct ke Proto struct
        let proto_asks = asks.into_iter().map(|l| ProtoOrderLevel {
            price: l.price,
            total_quantity: l.quantity,
        }).collect();

        let proto_bids = bids.into_iter().map(|l| ProtoOrderLevel {
            price: l.price,
            total_quantity: l.quantity,
        }).collect();

        Ok(Response::new(DepthResponse {
            bids: proto_bids,
            asks: proto_asks,
            sequence_id: 0, // TODO: Implement sequence number nanti
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Channel: Buffer 1024 command antrian
    let (tx, rx) = mpsc::channel(1024);

    // 2. Spawn Market Processor (The Engine) di background thread
    let processor = MarketProcessor::new(rx);
    tokio::spawn(async move {
        processor.run().await;
    });

    // 3. Setup gRPC Server
    let addr = "[::1]:50051".parse()?;
    let trading_service = TradingService {
        processor_sender: tx,
    };

    println!("Velocity DEX Engine listening on {}", addr);

    Server::builder()
        .add_service(TradingEngineServer::new(trading_service))
        .serve(addr)
        .await?;

    Ok(())
}