// crates/engine-core/src/processor.rs

use tokio::sync::mpsc;
use crate::{OrderBook, Side, EngineEvent, OrderLevel}; 

// Command yang bisa dikirim oleh API ke Engine
#[derive(Debug)]
pub enum Command {
    PlaceOrder {
        user_id: u64,
        order_id: u64, // Pre-generated ID
        side: Side,
        price: u64,
        quantity: u64,
        // Channel untuk mengirim balik hasil ke API handler (One-shot)
        responder: tokio::sync::oneshot::Sender<Vec<EngineEvent>>, 
    },
    CancelOrder {
        user_id: u64,
        order_id: u64,
        responder: tokio::sync::oneshot::Sender<Vec<EngineEvent>>,
    },
    GetDepth {
        limit: usize,
        // Responder mengembalikan tuple (Asks, Bids)
        responder: tokio::sync::oneshot::Sender<(Vec<OrderLevel>, Vec<OrderLevel>)>,
    }
}

pub struct MarketProcessor {
    book: OrderBook, // The Engine Core (Sync)
    receiver: mpsc::Receiver<Command>, // Inbox
}

impl MarketProcessor {
    pub fn new(receiver: mpsc::Receiver<Command>) -> Self {
        Self {
            book: OrderBook::new(),
            receiver,
        }
    }

    // Ini akan dijalankan di tokio::spawn_blocking atau thread dedikasi
    pub async fn run(mut self) {
        while let Some(cmd) = self.receiver.recv().await {
            match cmd {
                Command::PlaceOrder { user_id, order_id, side, price, quantity, responder } => {
                    let events = self.book.place_limit_order(order_id, user_id, side, price, quantity);
                    let _ = responder.send(events);
                }
                Command::CancelOrder { user_id, order_id, responder } => {
                    let events = self.book.cancel_order(order_id, user_id);
                    let _ =responder.send(events);
                }
                Command::GetDepth { limit, responder } => {
                    let depth = self.book.get_depth(limit);
                    let _ = responder.send(depth);
                }
            }
        }
    }
}