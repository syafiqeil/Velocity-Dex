// crates/engine-core/src/lib.rs

use std::collections::{BTreeMap, VecDeque};
use slab::Slab;

// ====================================================
// Data Structures (Optimize for Cache Locality & Copy)
// ====================================================
pub type OrderId = u64;
pub type UserId = u64;
pub type Price = u64; // Menggunakan atomic units (misal: satoshi) untuk menghindari Floating Point errors
pub type Quantity = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

impl Side {
    pub fn opposite(&self) -> Self {
        match self {
            Side::Bid => Side::Ask,
            Side::Ask => Side::Bid,
        }
    }
}

// Order struct dibuat "Copy" jika memungkinkan, atau sangat ringan untuk di clone.
#[derive(Debug, Clone)]
pub struct Order {
    pub id: OrderId,
    pub user_id: UserId,
    pub price: Price,
    pub quantity: Quantity,
    pub side: Side,
    pub timestamp: u64,
}

// Event yang dipancarkan engine *Penting untuk decoupling engine dari database/network.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    OrderPlaced {id: OrderId, user_id: UserId, price: Price, quantity:Quantity, side: Side},
    OrderCancelled {id: OrderId},
    TradeExecuted {maker_id: OrderId, taker_id: OrderId, price: Price, quantity: Quantity},
}

// ================================
// The Matching Engine (Core Logic)
// ================================
pub struct OrderBook {
    // Penyimpanan data order sebenarnya. Menggunakan Slab untuk akses O(1) dan reuse memory slot.
    // Ini lebih efisien daripada Box::new() setiap kali order baru masuk.
    order_store: Slab<Order>,

    // Indeks Harga -> Antrian Order ID
    // Bids diurutkan Descending / Asks diurutkan Ascending
    bids: BTreeMap<Price, VecDeque<usize>>, // usize adalah key dari Slab
    asks: BTreeMap<Price, VecDeque<usize>>, 
    sequence: u64, // Tambahan untuk internal unique ID
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            order_store: Slab::with_capacity(10_000), // Pre-allocate memory
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            sequence: 0,
        }
    }

    // Fungsi utama untuk memproses Limit Order
    // Mengembalikan daftar event yang terjadi (Trade, Placement, dll)
    pub fn place_limit_order(
        &mut self,
        order_id: OrderId, // ID dilempar dari luar (API/Sequencer)
        user_id: UserId,
        side: Side,
        price: Price,
        mut quantity: Quantity
    ) -> Vec<EngineEvent> {
        let mut events = Vec::new();

        // 1. Matching Process (Taker Phase)
        // Mencoba mencocokkan order yang masuk dengan order yang ada di buku
        loop {
            if quantity == 0 {
                break;
            }

            // Cari order lawan terbaik (Best Bid or Best Ask)
            let best_match_option = match side {
                Side::Bid => self.asks.iter_mut().next(), // Lowest Ask
                Side::Ask => self.bids.iter_mut().next_back(), // Highest Bid
            };

            // Jika tidak ada liquidity, stop matching
            let (best_price, order_queue) = match best_match_option {
                Some((p, q)) => (*p, q),
                None => break,
            };

            // Cek apakah harga memenuhi syarat
            // Bid: beli jika harga lawan <= harga limit saya
            // Ask: jual jika harga lawan >= harga limit saya
            let is_matchable = match side {
                Side::Bid => best_price <= price,
                Side::Ask => best_price >= price,
            };

            if !is_matchable {
                break;
            }

            // Proses queue pada harga terbaik
            while let Some(&maker_idx) = order_queue.front() {
                // Ambil referensi mutable ke maker order
                let maker_order = self.order_store.get_mut(maker_idx).expect("Stale index in queue");

                // Self-Trade Prevention 
                if maker_order.user_id == user_id {
                    // Kebijakan: Cancel Maker (Resting Order dibuang)
                    // Agar loop tidak macet, kita harus pop order ini.
                    order_queue.pop_front();
                    
                    // Emit event cancel jika perlu
                    events.push(EngineEvent::OrderCancelled { id: maker_order.id });
                    
                    // Hapus dari Slab
                    self.order_store.remove(maker_idx);
                    
                    // Lanjut ke order berikutnya di antrian yang sama
                    continue; 
                }

                // Hitung jumlah yang bisa di-trade
                let trade_qty = std::cmp::min(quantity, maker_order.quantity);

                // Emit Trade Event
                events.push(EngineEvent::TradeExecuted {
                    maker_id: maker_order.id, // ID eksternal
                    taker_id: order_id, // Menggunakan ID Taker yang sedang diproses
                    price: best_price,
                    quantity: trade_qty,
                });

                // Update quantity
                quantity -= trade_qty;
                maker_order.quantity -= trade_qty;

                // Jika maker order habis, hapus dari buku
                if maker_order.quantity == 0 {
                    order_queue.pop_front();
                    self.order_store.remove(maker_idx);
                }

                if quantity == 0 {
                    break;
                }
            }

            // Bersihkan entry harga jika queue kosong
            if order_queue.is_empty() {
                match side {
                    Side::Bid => { self.asks.remove(&best_price); },
                    Side::Ask => { self.bids.remove(&best_price); },
                }
            }
        }

        // 2. PLACEMENT PROCESS (Maker Phase)
        if quantity > 0 {
            let new_order = Order {
                id: order_id,
                user_id,
                price,
                quantity,
                side,
                timestamp: 0, // Sebaiknya ambil dari input function atau SystemTime
            };

            // Simpan ke Slab
            let idx = self.order_store.insert(new_order.clone());

            // Masukkan index ke queue yang sesuai
            let queue = match side {
                Side::Bid => self.bids.entry(price).or_insert_with(VecDeque::new),
                Side::Ask => self.asks.entry(price).or_insert_with(VecDeque::new),
            };
            queue.push_back(idx);

            events.push(EngineEvent::OrderPlaced {
                id: order_id,
                user_id,
                price,
                quantity,
                side,
            });
        }

        events
    }
    
    // Helper untuk debug: Melihat kedalaman pasar
    pub fn get_depth(&self, levels: usize) {
        println!("--- ASK ---");
        for (price, queue) in self.asks.iter().take(levels) {
            let vol: u64 = queue.iter().map(|&i| self.order_store[i].quantity).sum();
            println!("Price: {} | Vol: {}", price, vol);
        }
        println!("--- BID ---");
        for (price, queue) in self.bids.iter().rev().take(levels) {
            let vol: u64 = queue.iter().map(|&i| self.order_store[i].quantity).sum();
            println!("Price: {} | Vol: {}", price, vol);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limit_order_placement_no_match() {
        let mut book = OrderBook::new();
        // User 1 place bid @ 100, Qty 10
        let events = book.place_limit_order(1, 101, Side::Bid, 100, 10);

        assert_eq!(events.len(), 1);
        if let EngineEvent::OrderPlaced {id, ..} = events[0] {
            assert_eq!(id, 1);
        } else {
            panic!("Event salah!");
        }

        // Cek kedalaman
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 0);
    }

    #[test]
    fn test_full_match_execution() {
        let mut book = OrderBook::new();

        // 1. Maker: User 1 Sell (Ask) @ 100, Qty 10
        book.place_limit_order(1, 101, Side::Ask, 100, 10);

        // 2. Taker: User 2 Buy (Bid) @ 100, Qty 10
        let events = book.place_limit_order(2, 102, Side::Bid, 100, 10);

        // Harus ada TradeExecuted
        let trade_event = events.iter()find(|e| matches!(e, EngineEvent::TradeExecuted {..}));

        if let EngineEvent::TradeExecuted {maker_id, taker_id. price, quantity} = trade_event.unwrap() {
            assert_eq!(*maker_id, 1);
            assert_eq!(*taker_id, 2);
            assert_eq!(*price, 100);
            assert_eq!(*quantity, 10);
        }

        // Book harus kosong karena full match
        assert_eq!(book.bids.is_empty());
        assert_eq!(book.asks.is_empty());
    }

    #[test]
    fn test_partial_match() {
        let mut book = OrderBook::new();

        // Maker: Sell 20 @ 100
        book.place_limit_order(1, 101, Side::Ask, 100, 20);

        // Taker: Buy 20 @ 100
        let events = book.place_limit_order(2, 102, Side::Bid, 100, 10);

        // Harusnya ada Trade 10, dan sisa Ask 10 di book
        assert!(events.iter().any(|e| matches!(e, EngineEvent::TradeExecuted {quantity: 10, ..})));

        // Cek sisa order di Slab
        // Note: Ini akses internal, di real test mungkin butuh helper method
        let maker_idx = *book.asks.get(&100).unwrap().front().unwrap();
        let maker_order = book.order_store.get(maker_idx).unwrap();
        assert_eq!(maker_order.quantity, 10);
    }

    #[test]
    fn test_self_trade_prevention_cancel_maker() {
        let mut book = OrderBook::new();

        // User 1: Sell 10 @ 100 (Maker)
        book.place_limit_order(100, 1, Side:Ask, 100, 10);

        // Same user 1: Buy 10 @ 100 (Taker)
        let events = book.place_limit_order(200, 1, Side::Bid, 100, 10);

        // Ekspektasi (Sesuai kode):
        // 1. Maker di-cancel.
        // 2. Taker (Buy order) masuk ke book sebagai order baru (karena maker sudah hilang).
        
        let cancel_event = events.iter().find(|e| matches!(e, EngineEvent::OrderCancelled { .. }));
        assert!(cancel_event.is_some(), "Harusnya ada event cancel maker");

        let place_event = events.iter().find(|e| matches!(e, EngineEvent::OrderPlaced {..}));
        assert!(place_event.is_some(), "Taker order harusnya masuk book setelah maker dicancel");

        // Book sekarang isinya Bid dari Taker, Ask kosong
        assert!(!book.bids.is_empty());
        assert!(book.asks.is_empty());
    }
}