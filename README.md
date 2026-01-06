# GPM - GPU & LLM Monitoring Service

A production-grade, lightweight GPU and LLM monitoring service that runs as a background daemon. GPM tracks GPU usage, classifies workloads (gaming, LLM inference, ML training, general compute), and provides comprehensive monitoring with OpenTelemetry integration.

> This is a vibe coding project.

## Features

### Phase 1: Core Service (✅ COMPLETED)

- **NVML Integration**: Direct GPU monitoring via NVML with automatic fallback to nvidia-smi
- **Process Classification**: Automatic detection and categorization of workloads:
  - Gaming sessions (Steam integration, pattern matching)
  - LLM inference (Ollama support with token tracking)
  - ML training (PyTorch, TensorFlow, JAX detection)
  - General compute
- **Ollama LLM Monitoring**: Track token counts, TPS (tokens per second), TTFT (time to first token)
- **SQLite Storage**: Local persistence with automatic cleanup
- **Parquet Archival**: Efficient long-term storage of historical data
- **Real-time Metrics**: Poll GPU stats every 2 seconds (configurable)

### Phase 2: OpenTelemetry & Prometheus (✅ COMPLETED)

- **OpenTelemetry Metrics (v0.27)**: GPU, LLM, and process metrics
- **Prometheus Exporter**: `/metrics` endpoint on port 9090
- **OTLP Export**: gRPC export to OpenTelemetry collectors
- **Comprehensive Instrumentation**: All metrics auto-recorded
- **Grafana-Ready**: Compatible with Grafana Cloud and dashboards

### Phase 3: Dashboard & Web API (✅ COMPLETED)

- **Web Dashboard**: React + TypeScript UI with Chart.js
  - Circular gauge meters for utilization, memory, temperature, power
  - Separate historical charts for each metric with trend indicators
  - Min/Avg/Max statistics on charts
  - Multi-GPU support
  - Auto-refresh every 0.5 seconds
  - LLM sessions panel with model performance comparison
- **Transparent Ollama Proxy**: Intercepts LLM requests for automatic session tracking
- **Web API Server**: REST endpoints on port 8010
- **Deployment Scripts**: Easy start/stop scripts for all services

## Architecture

```
GPM/
├── gpm-core/          # Core monitoring service (Rust)
│   ├── src/
│   │   ├── gpu/          # NVML integration & GPU metrics
│   │   ├── storage/      # SQLite + Parquet storage
│   │   ├── telemetry/    # OpenTelemetry & Prometheus
│   │   ├── api.rs        # Web API server
│   │   ├── classifier.rs # Process workload classification
│   │   ├── ollama.rs     # Ollama LLM monitoring
│   │   ├── proxy.rs      # Ollama transparent proxy
│   │   ├── service.rs    # Main service orchestrator
│   │   ├── config.rs     # Configuration management
│   │   ├── error.rs      # Error types
│   │   ├── lib.rs        # Library interface
│   │   └── main.rs       # Binary entry point (gpm)
│   └── src/bin/
│       └── web-server.rs # Web API entry point (gpm-server)
├── gpm-dashboard/     # Web Dashboard (React + TypeScript)
└── scripts/              # Deployment scripts
```

## Requirements

- **Rust**: 1.70+ (2021 edition)
- **Node.js**: 20+ (for dashboard development)
- **NVIDIA GPU**: With NVML-compatible drivers (470+)
- **CUDA/NVML**: Installed with driver
- **Linux**: Primary target (Windows/macOS support varies)
- **Optional**: Ollama for LLM monitoring

## Installation

### Quick Install (Recommended)

```bash
curl -sSL https://raw.githubusercontent.com/ikaganacar1/GPM/main/install.sh | bash
```

This will:
- Detect your architecture (x86_64/arm64)
- Download the latest pre-built binary
- Install to `~/.local/bin`
- Set up systemd service (if available)
- Start the monitoring service

### From Release Binaries

Download from [GitHub Releases](https://github.com/ikaganacar1/GPM/releases):

```bash
# Download and extract
wget https://github.com/ikaganacar1/GPM/releases/latest/download/gpm-x86_64-unknown-linux-gnu.tar.gz
tar xzf gpm-x86_64-unknown-linux-gnu.tar.gz

# Install binaries
sudo cp gpm /usr/local/bin/
sudo cp gpm-server /usr/local/bin/
```

### Arch Linux (AUR)

Coming soon! PKGBUILD is included in the repo for manual builds:

```bash
git clone https://github.com/ikaganacar1/GPM.git
cd GPM
makepkg -si
```

### From Source

```bash
# Clone the repository
git clone https://github.com/ikaganacar1/GPM.git
cd GPM

# Build the service
cargo build --release --package gpm-core

# The binaries will be at: target/release/gpm and target/release/gpm-server
```

### Quick Start

```bash
# Run the monitoring service
./target/release/gpm

# Or with cargo
cargo run --package gpm-core
```

The service will:
1. Initialize NVML and connect to your GPU(s)
2. Create a SQLite database at `~/.local/share/gpm/gpm.db`
3. Start polling GPU metrics every 2 seconds
4. Classify running processes
5. Start Ollama proxy if enabled
6. Archive old data to Parquet files

### Uninstall

```bash
# Stop services
systemctl --user stop gpm gpm-server 2>/dev/null || true
pkill gpm 2>/dev/null || true

# Remove binaries
rm -f ~/.local/bin/gpm ~/.local/bin/gpm-server
sudo rm -f /usr/local/bin/gpm /usr/local/bin/gpm-server 2>/dev/null || true

# Remove systemd services
rm -f ~/.config/systemd/user/gpm.service
rm -f ~/.config/systemd/user/gpm-server.service

# Optionally remove data
# rm -rf ~/.local/share/gpm ~/.config/gpm
```

### Using the Dashboard

#### Start All Services

```bash
# Start all services (monitoring, API, dashboard) in foreground
./scripts/start-all.sh

# Start in background mode
./scripts/start-all.sh --background

# Stop all services
./scripts/stop-all.sh
```

#### Access Points

| Service | URL | Port |
|---------|-----|------|
| Dashboard | http://localhost:8011 | 8011 |
| Web API | http://localhost:8010 | 8010 |
| Ollama Proxy | http://localhost:11434 | 11434 |
| Prometheus | http://localhost:9090/metrics | 9090 |

#### Dashboard Development

```bash
cd gpm-dashboard

# Install dependencies
npm install

# Development mode (with hot reload)
npm run dev

# Production build
npm run build

# Serve production build
python3 server.py 8011
```

### Ollama Proxy Setup

The transparent proxy intercepts Ollama API calls to track LLM sessions:

```bash
# 1. Start GPM with proxy enabled (default: enabled)
./scripts/start-all.sh

# 2. Run Ollama on port 11435 (backend)
OLLAMA_HOST=127.0.0.1:11435 ollama serve

# 3. Use Ollama through the proxy (port 11434)
curl http://localhost:11434/api/generate -d '{"model": "qwen2:0.5b", "prompt": "Hello"}'
```

The proxy tracks:
- Model name and version
- Token counts (prompt, completion, total)
- Tokens per second (TPS)
- Time to first token (TTFT)
- Session duration

## Configuration

GPM looks for configuration in the following order:

1. Default values (hardcoded)
2. Config file at `~/.config/gpm/config.toml`
3. Environment variables with `GPM_` prefix

### Example Configuration

Create `~/.config/gpm/config.toml`:

```toml
[service]
poll_interval_secs = 2
data_dir = "~/.local/share/gpm"

[gpu]
enable_nvml = true
fallback_to_nvidia_smi = false

[ollama]
enabled = true
enable_proxy = true          # Enable transparent proxy
proxy_port = 11434           # Proxy port (use this for your apps)
backend_url = "http://localhost:11435"  # Real Ollama instance
api_port = 11434
api_url = "http://localhost:11434"

[storage]
retention_days = 7
enable_parquet_archival = true
archive_dir = "~/.local/share/gpm/archive"

[telemetry]
enable_opentelemetry = true
otlp_endpoint = "http://localhost:4317"
enable_prometheus = true
metrics_port = 9090

[alerts]
temp_threshold_celsius = 85.0
memory_threshold_percent = 90.0
enable_desktop_notifications = false
```

### Environment Variables

```bash
# Override any config with environment variables
export GPM_SERVICE_POLL_INTERVAL_SECS=5
export GPM_OLLAMA_ENABLED=true
export GPM_OLLAMA_ENABLE_PROXY=true
export GPM_STORAGE_RETENTION_DAYS=14

cargo run --package gpm-core
```

## Web API Endpoints

The web API server (`gpm-server`) exposes the following endpoints:

| Endpoint | Description |
|----------|-------------|
| `GET /api/info` | Dashboard info (GPU count, database path) |
| `GET /api/realtime` | Real-time GPU metrics |
| `GET /api/historical?hours=1` | Historical metrics (last N hours) |
| `GET /api/chart?gpu_id=0&hours=1` | Chart data for specific GPU |
| `GET /api/llm-sessions?start_date=&end_date=` | LLM sessions (RFC3339 dates) |

## Dashboard Features

### GPU Monitoring
- **Real-time gauges**: GPU utilization, memory, temperature, power
- **Color-coded warnings**: Red >85°C temp, >90% memory
- **Multi-GPU support**: Switch between GPUs in header

### Historical Charts
- **Separate metric charts**: GPU utilization, memory, temperature, power
- **Trend indicators**: ▲/▼/─ showing recent % change
- **Statistics row**: Min/Avg/Max for selected time range
- **Time ranges**: 1h, 6h, 24h with adaptive downsampling

### LLM Sessions Panel
- **6 stat cards**: Sessions, Avg TPS, Avg TTFT, Total Tokens, Avg Duration, Best Model
- **Model comparison**: Shows TPS for each model used
- **Sessions table**: Time, model, tokens, TPS, TTFT, duration
- **24-hour rolling window**

## Data Storage

### SQLite Database

Location: `~/.local/share/gpm/gpm.db`

Tables:
- `gpu_metrics`: GPU utilization, memory, temperature, power
- `llm_sessions`: Ollama session data with token counts
- `process_events`: Classified process activity
- `weekly_summaries`: Aggregated weekly statistics

### Parquet Archives

Location: `~/.local/share/gpm/archive/`

Old data (>7 days by default) is automatically archived to Parquet files for efficient storage and querying.

## Prometheus Metrics

GPM exposes Prometheus metrics on `http://localhost:9090/metrics` (configurable).

### Available Metrics

**GPU Metrics**:
- `gpm_gpu_utilization_percent` - GPU utilization % (gauge)
- `gpm_gpu_memory_used_bytes` - VRAM usage (gauge)
- `gpm_gpu_memory_total_bytes` - Total VRAM (gauge)
- `gpm_gpu_temperature_celsius` - GPU temperature (gauge)
- `gpm_gpu_power_watts` - Power draw (gauge)

Labels: `gpu_id`, `gpu_name`

**LLM Metrics**:
- `gpm_llm_tokens_per_second` - TPS distribution (histogram)
- `gpm_llm_time_to_first_token_ms` - TTFT latency (histogram)
- `gpm_llm_session_count` - Session count by model (gauge)

Labels: `model`

## Development

### Project Structure

```
gpm-core/
├── src/
│   ├── gpu/
│   │   ├── nvml.rs         # NVML wrapper with fallback
│   │   └── mod.rs          # GPU monitoring backend
│   ├── storage/
│   │   ├── db.rs           # SQLite operations
│   │   ├── parquet.rs      # Parquet archival
│   │   ├── schema.sql      # Database schema
│   │   └── mod.rs          # Storage manager
│   ├── classifier.rs       # Process classification
│   ├── ollama.rs           # Ollama LLM monitoring
│   ├── proxy.rs            # Transparent Ollama proxy
│   ├── service.rs          # Main service orchestrator
│   ├── config.rs           # Configuration management
│   ├── error.rs            # Error types
│   ├── lib.rs              # Library interface
│   └── main.rs             # Binary entry point
└── Cargo.toml

gpm-dashboard/
├── src/
│   ├── App.tsx             # Main dashboard component
│   ├── App.css             # Dashboard styles
│   └── main.tsx            # Entry point
├── server.py               # Simple dev server
└── package.json
```

### Running Tests

```bash
# Run all tests
cargo test --package gpm-core

# Run with output
cargo test --package gpm-core -- --nocapture
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint
cargo clippy --package gpm-core

# Check without building
cargo check --package gpm-core
```

## Troubleshooting

### NVML Initialization Failed

```
Error: NVML initialization failed
```

**Solutions:**
- Ensure NVIDIA drivers are installed: `nvidia-smi`
- Check NVML library is available: `ldconfig -p | grep nvidia-ml`
- Try fallback mode: Set `fallback_to_nvidia_smi = true` in config

### Ollama Proxy Not Working

```
Error: Connection refused to Ollama backend
```

**Solutions:**
- Ensure Ollama is running on port 11435: `curl http://localhost:11435/api/tags`
- Check proxy is enabled in config: `enable_proxy = true`
- Verify backend_url points to correct Ollama instance

### Dashboard Not Loading

**Solutions:**
- Check API server is running: `curl http://localhost:8010/api/info`
- Rebuild frontend: `cd gpm-dashboard && npm run build`
- Check browser console for errors

### High Memory Usage

**Solutions:**
- Reduce retention: Set `retention_days = 3` in config
- Enable archival: Set `enable_parquet_archival = true`
- Run maintenance manually (implemented in service)

## Performance

- **CPU Usage**: <1% idle, ~2% during polling
- **Memory**: <50MB RAM for service, ~100MB for dashboard
- **Storage**: ~1MB per day of metrics (compressed with Parquet)
- **Polling**: 2-second intervals (configurable)
- **Dashboard refresh**: 0.5 seconds

## Roadmap

### Phase 1 (✅ Completed)
- [x] Core GPU monitoring service
- [x] Process classification (Gaming, LLM, ML, General)
- [x] SQLite storage with Parquet archival
- [x] Ollama LLM monitoring

### Phase 2 (✅ Completed)
- [x] OpenTelemetry metrics
- [x] Prometheus metrics endpoint
- [x] Grafana integration

### Phase 3 (✅ Completed)
- [x] Web dashboard with React + TypeScript
- [x] Web API server
- [x] Transparent Ollama proxy
- [x] Deployment scripts

### Phase 4 (Planned)
- [ ] Desktop notifications for alerts
- [ ] System service installation (systemd/launchd)
- [ ] Additional chart types (heatmaps, histograms)
- [ ] Export functionality for reports

### Phase 5 (Planned)
- [ ] Cross-platform support (Windows, macOS)
- [ ] Integration tests
- [ ] CI/CD pipeline

## License

MIT License - See LICENSE file for details

## Contributing

Contributions welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Run tests: `cargo test`
4. Format code: `cargo fmt`
5. Submit a pull request

## Acknowledgments

- NVML wrapper: [nvml-wrapper](https://github.com/Cldfire/nvml-wrapper)
- Polars: High-performance DataFrame library
- SQLx: Async SQL toolkit
- Tokio: Async runtime
- Chart.js: JavaScript charting library
- Axum: Web framework
