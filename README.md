# GPM - GPU & LLM Monitoring Service

A production-grade, lightweight GPU and LLM monitoring service that runs as a background daemon. GPM tracks GPU usage, classifies workloads (gaming, LLM inference, ML training, general compute), and provides comprehensive monitoring with OpenTelemetry integration.

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

- **Tauri Desktop App**: Native GPU monitoring dashboard
- **Web API Server**: REST endpoints on port 8010
- **Real-time Dashboard**: React + TypeScript UI with Chart.js
  - Circular gauge meters for utilization, memory, temperature, power
  - Historical line charts for trends
  - Multi-GPU support
  - Auto-refresh every 2 seconds
- **Deployment Scripts**: Easy start/stop scripts for all services

## Architecture

```
GPM/
├── gpumon-core/          # Core monitoring service (Rust)
│   ├── src/
│   │   ├── gpu/          # NVML integration & GPU metrics
│   │   ├── storage/      # SQLite + Parquet storage
│   │   ├── telemetry/    # OpenTelemetry & Prometheus
│   │   ├── api.rs        # Web API server
│   │   ├── classifier.rs # Process workload classification
│   │   ├── ollama.rs     # Ollama LLM monitoring
│   │   ├── service.rs    # Main service orchestrator
│   │   └── main.rs       # Binary entry point (gpumon)
│   └── src/bin/
│       └── web-server.rs # Web API entry point
├── gpumon-dashboard/     # Tauri GUI (React + TypeScript)
└── scripts/              # Deployment scripts
```

## Requirements

- **Rust**: 1.70+ (2021 edition)
- **Node.js**: 20+ (for Tauri desktop app)
- **NVIDIA GPU**: With NVML-compatible drivers (470+)
- **CUDA/NVML**: Installed with driver
- **Linux**: Primary target (Windows/macOS support varies)
- **Optional**: Ollama for LLM monitoring

## Installation

### From Source

```bash
# Clone the repository
git clone <your-repo-url>
cd GPM

# Build the service
cargo build --release --package gpumon-core

# The binary will be at: target/release/gpumon
```

### Quick Start

```bash
# Run the monitoring service
cargo run --package gpumon-core

# Or use the compiled binary
./target/release/gpumon
```

The service will:
1. Initialize NVML and connect to your GPU(s)
2. Create a SQLite database at `~/.local/share/gpumon/gpumon.db`
3. Start polling GPU metrics every 2 seconds
4. Classify running processes
5. Monitor Ollama if it's running
6. Archive old data to Parquet files

### Using the Dashboard

#### Option 1: Web Mode (Recommended)

```bash
# Start both web API server and frontend dashboard
./scripts/start-web.sh

# Or run individual components:
./scripts/start-backend.sh  # Start monitoring service
./target/release/web-server  # Start web API on port 8010
cd gpumon-dashboard/dist && python3 -m http.server 8009  # Start frontend
```

Access the dashboard at: **http://localhost:8009**

#### Option 2: Tauri Desktop App (Requires Node.js 20+)

```bash
cd gpumon-dashboard
npm run tauri dev
```

#### Option 3: Production Deployment

```bash
# Start all services in background
./scripts/start-all.sh

# Stop all services
./scripts/stop-all.sh
```

### Web API Endpoints

The web API server (`target/release/web-server`) exposes the following endpoints:

| Endpoint | Description |
|----------|-------------|
| `GET /api/info` | Dashboard info (GPU count, database path) |
| `GET /api/realtime` | Real-time GPU metrics |
| `GET /api/historical?hours=1` | Historical metrics (last N hours) |
| `GET /api/chart?gpu_id=0&hours=1` | Chart data for specific GPU |
| `GET /api/llm-sessions?start_date=&end_date=` | LLM sessions |

## Configuration

GPM looks for configuration in the following order:

1. Default values (hardcoded)
2. Config file at `~/.config/gpumon/config.toml`
3. Environment variables with `GPUMON_` prefix

### Example Configuration

Create `~/.config/gpumon/config.toml`:

```toml
[service]
poll_interval_secs = 2
data_dir = "~/.local/share/gpumon"

[gpu]
enable_nvml = true
fallback_to_nvidia_smi = false

[ollama]
enabled = true
api_port = 11434
api_url = "http://localhost:11434"

[storage]
retention_days = 7
enable_parquet_archival = true
archive_dir = "~/.local/share/gpumon/archive"

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
export GPUMON_SERVICE_POLL_INTERVAL_SECS=5
export GPUMON_OLLAMA_ENABLED=true
export GPUMON_STORAGE_RETENTION_DAYS=14

cargo run --package gpumon-core
```

## Data Storage

### SQLite Database

Location: `~/.local/share/gpumon/gpumon.db`

Tables:
- `gpu_metrics`: GPU utilization, memory, temperature, power
- `llm_sessions`: Ollama session data with token counts
- `process_events`: Classified process activity
- `weekly_summaries`: Aggregated weekly statistics

### Parquet Archives

Location: `~/.local/share/gpumon/archive/`

Old data (>7 days by default) is automatically archived to Parquet files for efficient storage and querying.

## Usage Examples

### Monitor GPU in Real-time

```bash
# Run with debug logging
RUST_LOG=debug cargo run --package gpumon-core

# Run in background
cargo run --package gpumon-core > /dev/null 2>&1 &
```

### Query Data (SQLite)

```bash
# Access the database
sqlite3 ~/.local/share/gpumon/gpumon.db

# Recent GPU metrics
SELECT timestamp, gpu_id, utilization_gpu, memory_used
FROM gpu_metrics
WHERE timestamp > datetime('now', '-1 hour')
ORDER BY timestamp DESC;

# LLM sessions summary
SELECT model, COUNT(*) as sessions,
       AVG(tokens_per_second) as avg_tps,
       AVG(time_to_first_token_ms) as avg_ttft
FROM llm_sessions
GROUP BY model;

# Process categories
SELECT category, COUNT(*) as count,
       AVG(gpu_memory_mb) as avg_mem
FROM process_events
WHERE timestamp > datetime('now', '-1 day')
GROUP BY category;
```

## Process Classification

GPM automatically classifies GPU-using processes:

### Gaming Detection
- Scans Steam library directories
- Pattern matching on executables (*.exe, *-dx12.exe, *-Vulkan.exe)
- Heuristic: GPU >60% + executable patterns = gaming

### LLM Inference
- Ollama process detection
- Python + ML framework + "generate/inference/predict" in command line

### ML Training
- Python + PyTorch/TensorFlow/JAX in command line
- High GPU memory allocation patterns

### General Compute
- Any other GPU-utilizing process

## Prometheus Metrics

GPM exposes Prometheus metrics on `http://localhost:9090/metrics` (configurable).

### Available Metrics

**GPU Metrics**:
- `gpumon_gpu_utilization_percent` - GPU utilization % (gauge)
- `gpumon_gpu_memory_used_bytes` - VRAM usage (gauge)
- `gpumon_gpu_memory_total_bytes` - Total VRAM (gauge)
- `gpumon_gpu_temperature_celsius` - GPU temperature (gauge)
- `gpumon_gpu_power_watts` - Power draw (gauge)

Labels: `gpu_id`, `gpu_name`

**LLM Metrics**:
- `gpumon_llm_tokens_per_second` - TPS distribution (histogram)
- `gpumon_llm_time_to_first_token_ms` - TTFT latency (histogram)
- `gpumon_llm_session_count` - Session count by model (gauge)

Labels: `model`

**Process Metrics**:
- `gpumon_process_count` - Process count by category (gauge)
- `gpumon_process_gpu_memory_bytes` - GPU memory by category (gauge)

Labels: `category`

### Example Prometheus Query

```bash
# View all metrics
curl http://localhost:9090/metrics

# GPU utilization
curl http://localhost:9090/metrics | grep gpumon_gpu_utilization_percent

# LLM performance
curl http://localhost:9090/metrics | grep gpumon_llm_tokens_per_second
```

### Grafana Integration

Import metrics into Grafana using Prometheus data source:
1. Add Prometheus datasource: `http://localhost:9090`
2. Create dashboards with queries like:
   - `gpumon_gpu_utilization_percent{gpu_id="0"}`
   - `rate(gpumon_llm_session_count[5m])`
   - `gpumon_process_count{category="gaming"}`

## Ollama Integration

GPM monitors Ollama LLM sessions and tracks:

- **Model name**: Which model is being used
- **Prompt tokens**: Input token count
- **Completion tokens**: Generated token count
- **Tokens per second (TPS)**: Generation speed
- **Time to first token (TTFT)**: Latency metric
- **Time per output token (TPOT)**: Per-token generation time

### Viewing LLM Stats

```bash
sqlite3 ~/.local/share/gpumon/gpumon.db

SELECT
    model,
    COUNT(*) as sessions,
    SUM(total_tokens) as total_tokens,
    AVG(tokens_per_second) as avg_tps,
    AVG(time_to_first_token_ms) as avg_ttft_ms
FROM llm_sessions
GROUP BY model
ORDER BY sessions DESC;
```

## Development

### Project Structure

```
gpumon-core/
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
│   ├── service.rs          # Main service orchestrator
│   ├── config.rs           # Configuration management
│   ├── error.rs            # Error types
│   ├── lib.rs              # Library interface
│   └── main.rs             # Binary entry point
└── Cargo.toml
```

### Running Tests

```bash
# Run all tests
cargo test --package gpumon-core

# Run with output
cargo test --package gpumon-core -- --nocapture

# Run specific test
cargo test --package gpumon-core test_ollama_detection
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint
cargo clippy --package gpumon-core

# Check without building
cargo check --package gpumon-core
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

### No Ollama Sessions Detected

```
Ollama API not reachable
```

**Solutions:**
- Ensure Ollama is running: `ollama list`
- Check API port: `curl http://localhost:11434/api/tags`
- Verify config: `api_url = "http://localhost:11434"`

### High Memory Usage

**Solutions:**
- Reduce retention: Set `retention_days = 3` in config
- Enable archival: Set `enable_parquet_archival = true`
- Run maintenance manually (implemented in service)

## Performance

- **CPU Usage**: <1% idle, ~2% during polling
- **Memory**: <50MB RAM typical usage
- **Storage**: ~1MB per day of metrics (compressed with Parquet)
- **Polling**: 2-second intervals (configurable)

## Roadmap

### Phase 1 (✅ Completed)
- [x] Core GPU monitoring service
- [x] Process classification (Gaming, LLM, ML, General)
- [x] SQLite storage with Parquet archival
- [x] Ollama LLM monitoring

### Phase 2 (✅ Completed)
- [x] OpenTelemetry metrics
- [x] Prometheus metrics endpoint
- [x] Distributed tracing
- [x] Grafana integration

### Phase 3 (✅ Completed)
- [x] Tauri desktop dashboard
- [x] Web API server
- [x] Real-time charts with Chart.js
- [x] Deployment scripts

### Phase 4 (Planned)
- [ ] Web-compatible frontend (fetch-based)
- [ ] eBPF-based game detection
- [ ] Smart archival with compression
- [ ] Desktop notifications
- [ ] System service installation (systemd/launchd)

### Phase 5 (Planned)
- [ ] Cross-platform support (Windows, macOS)
- [ ] Performance optimizations
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

## Support

For issues and questions:
- GitHub Issues: [your-repo-url]/issues
- Documentation: [your-docs-url]

## Acknowledgments

- NVML wrapper: [nvml-wrapper](https://github.com/Cldfire/nvml-wrapper)
- Polars: High-performance DataFrame library
- SQLx: Async SQL toolkit
- Tokio: Async runtime
- Sysinfo: System information library
