// crates/trading-cli/src/main.rs

use clap::{Parser, Subcommand};
use trading::trading_engine_client::TradingEngineClient;
use trading::{PlaceOrderRequest, DepthRequest, Side};

pub mod trading {
    tonic::include_proto!("trading");
}

#[derive(Parser)]
#[command(name = "Velocity CLI")]
#[command(about = "High-Performance DEX CLI Client", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // Menaruh Limit Order (Buy)
    Buy {
        #[arg(short, long)]
        price: u64,
        #[arg(short, long)]
        quantity: u64,
        #[arg(short, long, default_value_t = 1)]
        user_id: u64,
        #[arg(long, default_value_t = 0)] // Jika 0, generate random
        order_id: u64,
    },
    // Menaruh Limit Order (Sell)
    Sell {
        #[arg(short, long)]
        price: u64,
        #[arg(short, long)]
        quantity: u64,
        #[arg(short, long, default_value_t = 1)]
        user_id: u64,
        #[arg(long, default_value_t = 0)]
        order_id: u64,
    },
    // Membatalkan Order
    Cancel {
        #[arg(short, long)]
        order_id: u64,
        #[arg(short, long, default_value_t = 1)]
        user_id: u64,
    },
    // Melihat Orderbook (Depth)
    Depth {
        #[arg(short, long, default_value_t = 10)]
        limit: u32,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Koneksi ke gRPC Server (Pastikan server jalan di terminal lain)
    let mut client = TradingEngineClient::connect("http://[::1]:50051").await?;

    match cli.command {
        Commands::Buy { price, quantity, user_id, order_id } => {
            let final_oid = if order_id == 0 { rand::random() } else { order_id };
            
            println!("Sending BUY Order... ID: {}", final_oid);

            let request = PlaceOrderRequest {
                user_id,
                order_id: final_oid,
                side: Side::Bid as i32,
                price,
                quantity,
            };
            
            let response = client.place_limit_order(request).await?;
            println!("RESPONSE: {:#?}", response.into_inner());
        }
        Commands::Sell { price, quantity, user_id, order_id } => {
            let final_oid = if order_id == 0 { rand::random() } else { order_id };

            println!("Sending SELL Order... ID: {}", final_oid);

            let request = PlaceOrderRequest {
                user_id,
                order_id: final_oid,
                side: Side::Ask as i32,
                price,
                quantity,
            };

            let response = client.place_limit_order(request).await?;
            println!("RESPONSE: {:#?}", response.into_inner());
        }
        Commands::Cancel { order_id, user_id } => {
            let request = trading::CancelOrderRequest {
                user_id,
                order_id,
            };
            let response = client.cancel_order(request).await?;
            println!("CANCEL RESPONSE: {:#?}", response.into_inner());
        }
        Commands::Depth { limit } => {
            let request = DepthRequest {
                symbol: "SOL_USDC".to_string(),
                limit,
            };
            
            let response = client.get_order_book_depth(request).await?;
            let inner = response.into_inner();
            
            println!("\n=== ORDER BOOK (Top {}) ===", limit);
            println!("ASKS (Jual):");
            // Kita balik urutan asks agar harga termahal di atas (visualisasi standar)
            for level in inner.asks.iter().rev() {
                println!("  Price: {:>6} | Qty: {:>6}", level.price, level.total_quantity);
            }
            
            println!("-----------------------------");
            
            println!("BIDS (Beli):");
            for level in inner.bids.iter() {
                println!("  Price: {:>6} | Qty: {:>6}", level.price, level.total_quantity);
            }
            println!("=============================\n");
        }
    }

    Ok(())
}