# GPM Quick Start Guide

Get GPM up and running in 5 minutes!

## Prerequisites

```bash
# Verify you have Rust installed
rustc --version  # Should be 1.70+

# Verify you have NVIDIA drivers and NVML
nvidia-smi

# (Optional) If you want Ollama monitoring
ollama list
```

## Installation

### Step 1: Build GPM

```bash
cd /mnt/2tb_ssd/GPM
cargo build --release --package gpumon-core
```

The binary will be at: `target/release/gpumon`

### Step 2: (Optional) Configure

```bash
# Create config directory
mkdir -p ~/.config/gpumon

# Copy example config
cp config.example.toml ~/.config/gpumon/config.toml

# Edit if needed
nano ~/.config/gpumon/config.toml
```

### Step 3: Run GPM

```bash
# Run directly
./target/release/gpumon

# Or with cargo
cargo run --release --package gpumon-core

# Run in background
nohup ./target/release/gpumon > /tmp/gpumon.log 2>&1 &
```

## Verify It's Working

### Check the Logs

```bash
# If running in foreground, you'll see:
# INFO GPM - GPU & LLM Monitoring Service
# INFO Version: 0.1.0
# INFO NVML initialized successfully with X device(s)
# INFO Storage manager initialized
```

### Check the Database

```bash
# After a few seconds, check that data is being collected
sqlite3 ~/.local/share/gpumon/gpumon.db

# Query recent metrics
SELECT COUNT(*) FROM gpu_metrics;

# Should show increasing number of records
```

### View Real-time Data

```bash
# Watch the database grow
watch -n 1 'sqlite3 ~/.local/share/gpumon/gpumon.db "SELECT COUNT(*) as total_metrics FROM gpu_metrics"'

# View latest GPU metrics
sqlite3 ~/.local/share/gpumon/gpumon.db "
SELECT
    datetime(timestamp) as time,
    gpu_id,
    utilization_gpu as gpu_util,
    memory_used / 1024 / 1024 as mem_mb,
    temperature as temp_c,
    power_usage as power_w
FROM gpu_metrics
ORDER BY timestamp DESC
LIMIT 10;
"
```

## Usage Examples

### Monitor Gaming Session

1. Start GPM: `./target/release/gpumon`
2. Launch a game from Steam
3. After your session, query:

```sql
sqlite3 ~/.local/share/gpumon/gpumon.db "
SELECT
    name,
    category,
    datetime(timestamp) as time,
    gpu_memory_mb,
    gpu_utilization
FROM process_events
WHERE category = 'gaming'
ORDER BY timestamp DESC
LIMIT 20;
"
```

### Monitor Ollama Sessions

1. Start Ollama: `ollama run llama2`
2. GPM will automatically track sessions
3. Query results:

```sql
sqlite3 ~/.local/share/gpumon/gpumon.db "
SELECT
    model,
    datetime(start_time) as started,
    prompt_tokens,
    completion_tokens,
    ROUND(tokens_per_second, 2) as tps,
    time_to_first_token_ms as ttft_ms
FROM llm_sessions
ORDER BY start_time DESC
LIMIT 10;
"
```

### Weekly Summary

```sql
sqlite3 ~/.local/share/gpumon/gpumon.db "
SELECT
    category,
    event_count,
    ROUND(avg_gpu_utilization, 2) as avg_gpu_util,
    ROUND(total_duration_secs / 3600.0, 2) as hours
FROM weekly_summaries
ORDER BY week_start DESC, hours DESC;
"
```

## Troubleshooting

### Problem: "NVML initialization failed"

```bash
# Check NVIDIA drivers
nvidia-smi

# If that fails, install drivers:
# Ubuntu/Debian:
sudo apt install nvidia-driver-XXX

# Then restart GPM
```

**Workaround:** Enable fallback mode in config:
```toml
[gpu]
fallback_to_nvidia_smi = true
```

### Problem: "Ollama API not reachable"

```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# Start Ollama if needed
ollama serve

# Or disable Ollama monitoring in config:
[ollama]
enabled = false
```

### Problem: High disk usage

```bash
# Check database size
du -h ~/.local/share/gpumon/gpumon.db

# Enable archival in config:
[storage]
enable_parquet_archival = true
retention_days = 3  # Reduce retention

# Manually clean old data:
sqlite3 ~/.local/share/gpumon/gpumon.db "
DELETE FROM gpu_metrics WHERE timestamp < datetime('now', '-3 days');
VACUUM;
"
```

## Next Steps

1. **Explore the data**: Try different SQL queries on your GPU usage
2. **Customize config**: Adjust polling interval, retention, etc.
3. **Set up systemd** (optional): Run GPM as a system service
4. **Wait for Phase 2**: OpenTelemetry and Prometheus metrics
5. **Wait for Phase 3**: Beautiful Tauri dashboard!

## Stopping GPM

```bash
# If running in foreground: Ctrl+C

# If running in background:
pkill gpumon

# Or find and kill:
ps aux | grep gpumon
kill <PID>
```

## Useful Queries

### GPU Utilization Over Time

```sql
SELECT
    datetime(timestamp) as time,
    AVG(utilization_gpu) as avg_util,
    MAX(utilization_gpu) as max_util
FROM gpu_metrics
WHERE timestamp > datetime('now', '-1 hour')
GROUP BY strftime('%Y-%m-%d %H:%M', timestamp)
ORDER BY time DESC;
```

### Process Summary

```sql
SELECT
    category,
    COUNT(*) as processes,
    AVG(gpu_memory_mb) as avg_mem_mb,
    MAX(gpu_utilization) as max_util
FROM process_events
WHERE timestamp > datetime('now', '-1 day')
GROUP BY category
ORDER BY processes DESC;
```

### LLM Performance Stats

```sql
SELECT
    model,
    COUNT(*) as sessions,
    AVG(tokens_per_second) as avg_tps,
    AVG(time_to_first_token_ms) as avg_ttft,
    SUM(total_tokens) as total_tokens
FROM llm_sessions
GROUP BY model
ORDER BY sessions DESC;
```

## Get Help

- Read the full README: [README.md](README.md)
- Check configuration: [config.example.toml](config.example.toml)
- Report issues: [GitHub Issues](your-repo-url/issues)

Happy monitoring! 🚀
