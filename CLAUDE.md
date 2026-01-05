# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```bash
# Build the monitoring service (release)
cargo build --release --package gpm-core

# Run the monitoring service (development)
cargo run --package gpm-core

# Run with debug logging
RUST_LOG=debug cargo run --package gpm-core

# Run the web API server (port 8010)
./target/release/gpm-server

# Run all tests
cargo test --package gpm-core

# Run a single test
cargo test --package gpm-core test_name -- --nocapture

# Lint
cargo clippy --package gpm-core

# Format
cargo fmt
```

### Dashboard (Tauri + React)

```bash
cd gpm-dashboard
npm install
npm run dev          # Vite dev server
npm run build        # Build static assets
npm run tauri dev    # Run Tauri desktop app (requires Node.js 20+)
```

### Deployment Scripts

```bash
./scripts/start-web.sh      # Start web API + frontend
./scripts/start-backend.sh  # Start monitoring service only
./scripts/start-all.sh      # Start all services in background
./scripts/stop-all.sh       # Stop all services
```

## Architecture Overview

This is a Rust workspace with two crates:

### gpm-core (main library + binaries)

- **`main.rs`** → `gpm` binary: Background monitoring daemon
- **`bin/web-server.rs`** → `gpm-server` binary: REST API server (port 8010)

Core modules:
- **`service.rs`**: Main orchestrator - spawns three async task loops:
  - Metrics collector (configurable interval, default 2s)
  - Ollama monitor (5s interval)
  - Maintenance worker (hourly archival/cleanup)
- **`gpu/nvml.rs`**: NVML wrapper for GPU metrics with automatic fallback
- **`classifier.rs`**: Process classification (gaming, LLM, ML training, general)
- **`ollama.rs`**: Ollama API monitoring for LLM metrics (TPS, TTFT)
- **`storage/db.rs`**: SQLite operations (metrics, sessions, process events)
- **`storage/parquet.rs`**: Parquet archival for old data
- **`telemetry/prometheus.rs`**: Prometheus metrics endpoint (port 9090)
- **`telemetry/metrics.rs`**: OpenTelemetry metrics
- **`api.rs`**: Axum REST API routes

### gpm-dashboard (Tauri + React)

React TypeScript frontend with Chart.js for visualization. Fetches from web API at port 8010.

## Key Patterns

- **Arc + RwLock/Mutex**: Shared state across async tasks (`GpuMonitorBackend`, `ProcessClassifier`)
- **Broadcast channels**: Graceful shutdown signaling across spawned tasks
- **Tokio intervals**: Scheduled metric collection and maintenance
- **SQLite + Polars**: SQLx for transactions, Polars for batch Parquet operations

## Data Locations

- Database: `~/.local/share/gpm/gpm.db`
- Archives: `~/.local/share/gpm/archive/`
- Config: `~/.config/gpm/config.toml`

## API Endpoints (port 8010)

- `GET /api/info` - Dashboard info
- `GET /api/realtime` - Live GPU metrics
- `GET /api/historical?hours=N` - Historical data
- `GET /api/chart?gpu_id=N&hours=N` - Chart-ready data
- `GET /api/llm-sessions?start_date=&end_date=` - LLM session data

## Prometheus Metrics (port 9090)

Metrics prefixed with `gpm_` (e.g., `gpm_gpu_utilization_percent`, `gpm_llm_tokens_per_second`)
