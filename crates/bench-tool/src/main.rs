// crates/bench-tool/src/main.rs

use clap::Parser;
use rand::Rng;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Barrier;
use tonic::transport::Channel;
use trading::trading_engine_client::TradingEngineClient;
use trading::{PlaceOrderRequest, Side};
use hdrhistogram::Histogram;

pub mod trading {
    tonic::include_proto!("trading");
}

#[derive(Parser, Debug)]
#[command(name = "Velocity Bencmark")]
struct Args {
    // Total error yang dikirim
    #[arg(short, long, default_value_t = 10_000)]
    count: usize,

    // Jumlah koneksi concurrent (Virtual Users)
    #[arg(short, long, default_value_t = 50)]
    concurrency: usize,

    // URL Server gRPC
    #[arg(short, long, default_value = "http://[::1]:50051")]
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("Starting Benchmark: {} orders | {} users", args.count, args.concurrency);
    println!("Target: {}", args.url);

    // 1. Setup koneksi (channel pool sederhana)
    let mut channels = Vec::new();
    for _ in 0..args.concurrency {
        let channel = Channel::from_shared(args.url.clone())?
            .connect()
            .await?;
        channels.push(channel);
    }

    let orders_per_user = args.count / args.concurrency;
    let barrier = Arc::new(Barrier::new(args.concurrency));
    let mut handles = Vec::new();

    let start_time = Instant::now();

    // 2. Spawn Virtual Users
    for i in 0..args.concurrency {
        let channel = channels[i].clone();
        let barrier = barrier.clone();
        let count = orders_per_user;

        let handle = tokio::spawn(async move {
            let mut client = TradingEngineClient::new(channel);
            let mut latencies = Vec::with_capacity(count);

            // Tunggu semua user siap
            barrier.wait().await;

            // Mulai!
            for _ in 0..count {
                // Variabel `rng` dibuat dan dihancurkan di dalam block ini
                // Sehingga `rng` tidak pernah "menyeberang" (cross) ke baris `.await` di bawahnya
                let (side, price, quantity, user_id, order_id) = {
                    let mut rng = rand::rng();
                    (
                        if rng.random_bool(0.5) { Side::Bid } else { Side::Ask },
                        rng.random_range(90..110),
                        rng.random_range(1..100),
                        rng.random_range(1..1000),
                        rng.random::<u64>()
                    )
                };

                let request = PlaceOrderRequest {
                    user_id,
                    order_id,
                    side: side as i32,
                    price,
                    quantity,
                };

                let start = Instant::now();
                let _ = client.place_limit_order(request).await;
                let duration = start.elapsed();

                latencies.push(duration.as_micros() as u64);
            }
            latencies
        });
        handles.push(handle);
    }

    // 3. Collect Results
    let mut total_hist = Histogram::<u64>::new(3).unwrap();

    for handle in handles {
        let latencies = handle.await?;
        for lat in latencies {
            total_hist += lat;
        }
    }

    let total_duration = start_time.elapsed();
    let throughput = args.count as f64 / total_duration.as_secs_f64();

    // 4. Print Report (The "Money Shot")
    println!("\n========================================");
    println!("BENCHMARK COMPLETE");
    println!("========================================");
    println!("Total Time     : {:.2?}", total_duration);
    println!("Throughput     : {:.2} Orders/sec", throughput);
    println!("----------------------------------------");
    println!("LATENCY (Microseconds):");
    println!("   Avg            : {:.2} us", total_hist.mean());
    println!("   Min            : {} us", total_hist.min());
    println!("   p50 (Median)   : {} us", total_hist.value_at_quantile(0.5));
    println!("   p90            : {} us", total_hist.value_at_quantile(0.9));
    println!("   p99            : {} us", total_hist.value_at_quantile(0.99));
    println!("   Max            : {} us", total_hist.max());
    println!("========================================");

    Ok(())
}