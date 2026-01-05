// crates/engine-core/src/processor.rs

use tokio::sync::mpsc;
use crate::{OrderBook, Side, EngineEvent}; // Import struct Anda

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
    // Bisa tambah: CancelOrder, Snapshot, dll.
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
        println!("Market Engine Started...");
        
        while let Some(cmd) = self.receiver.recv().await {
            match cmd {
                Command::PlaceOrder { user_id, order_id, side, price, quantity, responder } => {
                    // 1. Execute Logic (CPU Bound - sangat cepat)
                    let events = self.book.place_limit_order(order_id, user_id, side, price, quantity);
                    
                    // 2. Kirim hasil balik ke API (Network Layer)
                    let _ = responder.send(events);
                    
                    // 3. (Optional) Kirim events ke Persistence Layer (Kafka/DB) secara async di sini
                }
            }
        }
    }
}