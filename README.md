# 🚀 Velocity DEX

![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)
![Tokio](https://img.shields.io/badge/concurrency-Tokio-blue)
![gRPC](https://img.shields.io/badge/transport-gRPC-333333)
![License](https://img.shields.io/badge/license-MIT-green)

**Velocity DEX** is a high-performance, crash-resilient Central Limit Order Book (CLOB) matching engine built from scratch in Rust.

Designed for low latency and high throughput, it utilizes the **Actor Model** to separate the synchronous matching logic from asynchronous network I/O. It supports high-frequency trading via **gRPC**, pushes real-time market data via **WebSockets**, and ensures data durability through an event-sourced **Write-Ahead-Log (WAL)**.

---

## ✨ Key Features

* **⚡ Ultra-Fast Matching Engine:**
    * In-memory Orderbook with **Price-Time Priority** matching.
    * Optimized memory layout using `Slab` allocation (Zero-allocation hot path).
    * **O(1)** order lookup and cancellation via HashMap indexing.
    * Supports **Maker** (Limit) and **Taker** (Market) orders with partial fills.

* **🛡️ Crash Resilience (Persistence):**
    * Implements **Event Sourcing** with a binary Write-Ahead-Log (WAL).
    * Automatic state recovery and log replay upon server restart.
    * Serialized using `bincode` for maximum efficiency.

* **📡 Dual-Interface Architecture:**
    * **Trading API (gRPC):** High-performance protobuf-based API for placing and canceling orders (`Tonic`).
    * **Market Data (WebSocket):** Real-time push notifications for trade execution and order updates (`Axum`).

* **🔐 Safety & Compliance:**
    * **Self-Trade Prevention (STP):** Automatically prevents users from matching against their own orders.
    * Atomic state transitions ensures orderbook consistency.

---

## 📊 Performance Benchmarks

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

---

## 🏗️ Architecture

The system is organized as a Cargo Workspace with modular crates: