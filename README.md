# Velocity DEX

![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)
![Tokio](https://img.shields.io/badge/concurrency-Tokio-blue)
![gRPC](https://img.shields.io/badge/transport-gRPC-333333)
![License](https://img.shields.io/badge/license-MIT-green)

**Velocity DEX** is a high-performance, crash-resilient Central Limit Order Book (CLOB) matching engine built from scratch in Rust.

Designed for low latency and high throughput, it utilizes the **Actor Model** to separate the synchronous matching logic from asynchronous network I/O. It supports high-frequency trading via **gRPC**, pushes real-time market data via **WebSockets**, and ensures data durability through an event-sourced **Write-Ahead-Log (WAL)**.

## Key Features

* **Ultra-Fast Matching Engine:**
    * In-memory Orderbook with **Price-Time Priority** matching.
    * Optimized memory layout using `Slab` allocation (Zero-allocation hot path).
    * **O(1)** order lookup and cancellation via HashMap indexing.
    * Supports **Maker** (Limit) and **Taker** (Market) orders with partial fills.

* **Crash Resilience (Persistence):**
    * Implements **Event Sourcing** with a binary Write-Ahead-Log (WAL).
    * Automatic state recovery and log replay upon server restart.
    * Serialized using `bincode` for maximum efficiency.

* **Dual-Interface Architecture:**
    * **Trading API (gRPC):** High-performance protobuf-based API for placing and canceling orders (`Tonic`).
    * **Market Data (WebSocket):** Real-time push notifications for trade execution and order updates (`Axum`).

* **Safety & Compliance:**
    * **Self-Trade Prevention (STP):** Automatically prevents users from matching against their own orders.
    * Atomic state transitions ensures orderbook consistency.

## Performance Benchmarks

Velocity DEX was stress-tested using a custom load generation tool (`crates/bench-tool`).

**Hardware:** Lenovo ThinkPad T470s (Intel Core i5-6300U @ 2.40GHz - Dual Core)  
**Conditions:** 50 Concurrent Users, 50,000 Orders  
**Result:**

| Metric | Result |
| :--- | :--- |
| **Throughput** | **~4,410 TPS** (Orders/Sec) |
| **Avg Latency** | **11.2 ms** (End-to-End) |
| **p99 Latency** | **22.5 ms** |

> *Note: These results are constrained by legacy dual-core hardware. On modern bare-metal servers, throughput is expected to exceed 50k+ TPS.*


## Architecture
The system is organized as a Cargo Workspace with modular crates:

    velocity-dex/
    ├── crates/
    │   ├── engine-core/      # The Brain: Pure matching logic, Orderbook struct, WAL handler.
    │   ├── api-server/       # The Mouth: gRPC server (Port 50051) & WebSocket server (Port 3000).
    │   ├── trading-cli/      # The Hand: Command-line interface for manual trading.
    │   └── bench-tool/       # The Hammer: Load testing tool for performance metrics.
    └── proto/                # Shared Protobuf definitions.

### The Actor Model
Velocity uses `tokio::sync::mpsc` channels to funnel concurrent requests into a single-threaded **MarketProcessor**. This eliminates the need for complex Mutex locking on the orderbook, ensuring deterministic execution and reducing thread contention.

## Getting Started

### Prerequisites
* **Rust** (latest stable)
* **Protoc** (Protocol Buffer Compiler)

### 1. Run the Server
The server starts both the gRPC Trading Engine (port 50051) and WebSocket Market Data feed (port 3000).

    # Run in release mode for best performance
    cargo run --release -p api-server

## 2. Run the CLI Client
Open a new terminal to interact with the engine.   

    # Check Orderbook Depth
    cargo run -p trading-cli -- depth

    # Place a Sell Order (Maker)
    cargo run -p trading-cli -- sell --price 100 --quantity 50 --user-id 1 --order-id 1001

    # Place a Buy Order (Taker - Matches immediately)
    cargo run -p trading-cli -- buy --price 100 --quantity 10 --user-id 2 --order-id 2001    

## 3. Connect to WebSocket
You can use any WebSocket client (like browser extensions or wscat) to listen to live market data.

* URL: ws://127.0.0.1:3000/ws

Sample JSON Output:

    {
        "type": "ORDER_PLACED",
        "id": 1001,
        "price": 100,
        "qty": 50,
        "side": "Ask"
    }

## Running Benchmarks
To reproduce the performance metrics:

First, Start the server in release mode (cargo run --release -p api-server).
Then, Run the benchmark tool in a separate terminal:
    
    # Simulate 50 concurrent users sending 50,000 orders
    cargo run --release -p bench-tool -- --count 50000 --concurrency 50

## Tech Stack
1. Language: Rust 🦀
2. Runtime: Tokio (Async I/O)
3. gRPC: Tonic (Prost)
4. Web Framework: Axum (WebSockets)
5. Serialization: Serde & Bincode
6. Memory: Slab (Arena allocation)
7. Metrics: HdrHistogram

## License
This project is open-source and available under the MIT License.