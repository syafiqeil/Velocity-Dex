// crates/engine-core/src/processor.rs

use tokio::sync::{mpsc, broadcast};
use crate::{OrderBook, Side, EngineEvent, OrderLevel, LogEntry};
use crate::wal::WalHandler; 

#[derive(Debug)]
pub enum Command {
    PlaceOrder {
        user_id: u64,
        order_id: u64, 
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
    book: OrderBook, 
    receiver: mpsc::Receiver<Command>,
    wal: WalHandler,
    pub event_broadcaster: broadcast::Sender<EngineEvent>,
}

impl MarketProcessor {
    pub fn new(receiver: mpsc::Receiver<Command>, broadcaster: broadcast::Sender<EngineEvent>) -> Self {
        let wal_path = "velocity.wal";
        
        // 1. Recovery Phase
        println!("Recovering state from WAL...");
        let mut book = OrderBook::new();
        
        // Load log lama jika ada
        if let Ok(entries) = WalHandler::read_all(wal_path) {
            println!("Replaying {} events...", entries.len());
            for entry in entries {
                match entry {
                    LogEntry::Place { order_id, user_id, side, price, quantity } => {
                        book.place_limit_order(order_id, user_id, side, price, quantity);
                    }
                    LogEntry::Cancel { order_id, user_id } => {
                        book.cancel_order(order_id, user_id);
                    }
                }
            }
        } else {
            println!("No WAL found, starting fresh.");
        }

        // 2. Open WAL for Writing
        let wal = WalHandler::new(wal_path).expect("Failed to open WAL file");

        Self {
            book,
            receiver,
            wal,
            event_broadcaster: broadcaster,
        }
    }

    // Ini akan dijalankan di tokio::spawn_blocking atau thread dedikasi
    pub async fn run(mut self) {
        println!("Market Engine Started & Persisted.");

        while let Some(cmd) = self.receiver.recv().await {
            match cmd {
                Command::PlaceOrder { user_id, order_id, side, price, quantity, responder } => {
                    // 1. (WAL) Persistence First (Write-Ahead)
                    let log_entry = LogEntry::Place { order_id, user_id, side, price, quantity };
                    
                    if let Err(e) = self.wal.write_entry(&log_entry) {
                        eprintln!("CRITICAL: Failed to write to WAL: {}", e);
                    }

                    // 2. Mmemory Execution
                    let events = self.book.place_limit_order(order_id, user_id, side, price, quantity);

                    // 3. Broadcast (Pub/Sub) 
                    // Kirim copy event ke semua subscriber WebSocket
                    for event in &events {
                        // Hanya broadcast event publik (Trade). Private info (OrderPlaced) opsional.
                        // Di sini broadcast semuanya agar dashboard terlihat hidup
                        let _ = self.event_broadcaster.send(event.clone());
                    }

                    // 4. Respond (gRPC)
                    let _ = responder.send(events);
                }
                
                Command::CancelOrder { user_id, order_id, responder } => {
                    // 1. Persistence First
                    let log_entry = LogEntry::Cancel { order_id, user_id };
                    
                    if let Err(e) = self.wal.write_entry(&log_entry) {
                        eprintln!("CRITICAL: Failed to write to WAL: {}", e);
                    }

                    // 2. Memory Execution
                    let events = self.book.cancel_order(order_id, user_id);
                    
                    // Broadcast Cancel
                    for event in &events {
                        let _ = self.event_broadcaster.send(event.clone());
                    }

                    let _ = responder.send(events);
                }

                Command::GetDepth { limit, responder } => {
                    // Read-only command tidak perlu ditulis ke WAL
                    let depth = self.book.get_depth(limit);
                    let _ = responder.send(depth);
                }
            }
        }
    }
}