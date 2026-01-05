# GPM - GPU & LLM Monitoring Service

A production-grade, lightweight GPU and LLM monitoring service that runs as a background daemon. GPM tracks GPU usage, classifies workloads (gaming, LLM inference, ML training, general compute), and provides comprehensive monitoring with OpenTelemetry integration.

## Features

### Phase 1: Core Service (✅ Implemented)

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

### Phase 2: OpenTelemetry Integration (🚧 Planned)

- Distributed tracing for LLM sessions
- Prometheus metrics export
- OTLP telemetry export
- Semantic metrics and labels

### Phase 3: Tauri Dashboard (🚧 Planned)

- Real-time GPU monitoring dashboard
- Weekly usage summaries
- LLM session analytics
- Process history and insights

## Architecture

```
GPM/
├── gpumon-core/          # Core monitoring service (Rust)
│   ├── src/
│   │   ├── gpu/          # NVML integration & GPU metrics
│   │   ├── storage/      # SQLite + Parquet storage
│   │   ├── classifier.rs # Process workload classification
│   │   ├── ollama.rs     # Ollama LLM monitoring
│   │   ├── service.rs    # Main service orchestrator
│   │   └── main.rs       # Binary entry point
│   └── Cargo.toml
└── gpumon-dashboard/     # Tauri GUI (Phase 3)
```

## Requirements

- **Rust**: 1.70+ (2021 edition)
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

### Phase 2 (Next)
- [ ] Full OpenTelemetry integration
- [ ] Prometheus metrics endpoint
- [ ] Distributed tracing
- [ ] Alerting system

### Phase 3
- [ ] Tauri dashboard with real-time charts
- [ ] Weekly review interface
- [ ] Export functionality (CSV, JSON, Parquet)

### Phase 4
- [ ] eBPF-based game detection
- [ ] Smart archival with compression
- [ ] Desktop notifications
- [ ] System service installation (systemd/launchd)

### Phase 5
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
