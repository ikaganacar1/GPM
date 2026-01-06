# GPM Quick Start Guide

Get GPM (GPU & LLM Monitoring Service) up and running in 5 minutes!

## Prerequisites

```bash
# Verify Rust installation
rustc --version  # Should be 1.70+

# Verify NVIDIA drivers
nvidia-smi

# (Optional) For Ollama monitoring
ollama list
```

## Installation

### Step 1: Build GPM

```bash
cd GPM
cargo build --release --package gpm-core
```

Binaries will be at: `target/release/gpm` and `target/release/gpm-server`

### Step 2: (Optional) Configure

```bash
# Create config directory
mkdir -p ~/.config/gpm

# Default config works, but you can customize
nano ~/.config/gpm/config.toml
```

### Step 3: Start All Services

```bash
# Start everything (monitoring, API, dashboard)
./scripts/start-all.sh

# Or run in background
./scripts/start-all.sh --background

# Stop all services
./scripts/stop-all.sh
```

## Access Points

| Service | URL | Port |
|---------|-----|------|
| Dashboard | http://localhost:8011 | 8011 |
| Web API | http://localhost:8010 | 8010 |
| Ollama Proxy | http://localhost:11434 | 11434 |
| Prometheus | http://localhost:9090/metrics | 9090 |

## Using the Dashboard

Open http://localhost:8011 in your browser:

- **Real-time gauges** for GPU utilization, memory, temperature, power
- **Historical charts** with trend indicators (1h, 6h, 24h views)
- **LLM sessions panel** showing model performance stats
- **Multi-GPU support** - switch GPUs in the header

## Using Ollama Through the Proxy

```bash
# Start Ollama on port 11435 (backend)
OLLAMA_HOST=127.0.0.1:11435 ollama serve

# Use through the GPM proxy on port 11434
ollama run qwen2:0.5b "Hello"

# Or with curl
curl http://localhost:11434/api/generate -d '{
  "model": "qwen2:0.5b",
  "prompt": "Hello"
}'
```

GPM automatically tracks:
- Model name and version
- Token counts (prompt, completion, total)
- Tokens per second (TPS)
- Time to first token (TTFT)
- Session duration

## Verify It's Working

### Check API Server

```bash
curl http://localhost:8010/api/info
```

### Check Database

```bash
sqlite3 ~/.local/share/gpm/gpm.db

# Count GPU metrics
SELECT COUNT(*) FROM gpu_metrics;

# View latest metrics
SELECT datetime(timestamp), utilization_gpu, temperature
FROM gpu_metrics
ORDER BY timestamp DESC LIMIT 10;

# View LLM sessions
SELECT model, total_tokens, tokens_per_second
FROM llm_sessions
ORDER BY start_time DESC LIMIT 10;
```

### View Prometheus Metrics

```bash
curl http://localhost:9090/metrics
```

## Troubleshooting

### "NVML initialization failed"

```bash
# Check NVIDIA drivers
nvidia-smi

# Enable fallback mode in ~/.config/gpm/config.toml:
[gpu]
fallback_to_nvidia_smi = true
```

### "Ollama proxy not working"

```bash
# Check Ollama is running on port 11435
curl http://localhost:11435/api/tags

# Verify proxy is enabled in config:
[ollama]
enable_proxy = true
proxy_port = 11434
backend_url = "http://localhost:11435"
```

### "Dashboard not loading"

```bash
# Check API server
curl http://localhost:8010/api/info

# Rebuild frontend
cd gpm-dashboard
npm run build
```

## Configuration Example

Create `~/.config/gpm/config.toml`:

```toml
[service]
poll_interval_secs = 2

[gpu]
enable_nvml = true
fallback_to_nvidia_smi = false

[ollama]
enabled = true
enable_proxy = true
proxy_port = 11434
backend_url = "http://localhost:11435"

[storage]
retention_days = 7
enable_parquet_archival = true

[telemetry]
enable_prometheus = true
metrics_port = 9090
```

## Development Mode

```bash
cd gpm-dashboard
npm install
npm run dev    # Development server with hot reload
npm run build  # Production build
```

## Get Help

- Full documentation: [README.md](README.md)
- Report issues on GitHub

Happy monitoring! 🚀
