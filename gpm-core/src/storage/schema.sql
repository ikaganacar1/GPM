-- GPU metrics table
CREATE TABLE IF NOT EXISTS gpu_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp DATETIME NOT NULL,
    gpu_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    utilization_gpu INTEGER NOT NULL,
    utilization_memory INTEGER NOT NULL,
    memory_used BIGINT NOT NULL,
    memory_total BIGINT NOT NULL,
    temperature INTEGER NOT NULL,
    power_usage INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_gpu_metrics_timestamp ON gpu_metrics(timestamp);
CREATE INDEX IF NOT EXISTS idx_gpu_metrics_gpu_id ON gpu_metrics(gpu_id);

-- LLM sessions table
CREATE TABLE IF NOT EXISTS llm_sessions (
    id TEXT PRIMARY KEY,
    start_time DATETIME NOT NULL,
    end_time DATETIME,
    model TEXT NOT NULL,
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    tokens_per_second REAL NOT NULL DEFAULT 0.0,
    time_to_first_token_ms BIGINT,
    time_per_output_token_ms REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_llm_sessions_start_time ON llm_sessions(start_time);
CREATE INDEX IF NOT EXISTS idx_llm_sessions_model ON llm_sessions(model);

-- Process events table
CREATE TABLE IF NOT EXISTS process_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp DATETIME NOT NULL,
    pid INTEGER NOT NULL,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    gpu_memory_mb BIGINT NOT NULL,
    gpu_utilization INTEGER NOT NULL,
    command_line TEXT,
    exe_path TEXT,
    duration_secs INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_process_events_timestamp ON process_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_process_events_category ON process_events(category);
CREATE INDEX IF NOT EXISTS idx_process_events_pid ON process_events(pid);

-- Weekly summaries table
CREATE TABLE IF NOT EXISTS weekly_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    week_start DATE NOT NULL,
    week_end DATE NOT NULL,
    category TEXT NOT NULL,
    total_duration_secs BIGINT NOT NULL DEFAULT 0,
    avg_gpu_utilization REAL NOT NULL DEFAULT 0.0,
    max_gpu_utilization INTEGER NOT NULL DEFAULT 0,
    total_gpu_memory_mb BIGINT NOT NULL DEFAULT 0,
    event_count INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(week_start, category)
);

CREATE INDEX IF NOT EXISTS idx_weekly_summaries_week_start ON weekly_summaries(week_start);
CREATE INDEX IF NOT EXISTS idx_weekly_summaries_category ON weekly_summaries(category);

-- Archive log table
CREATE TABLE IF NOT EXISTS archive_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    archive_date DATE NOT NULL,
    table_name TEXT NOT NULL,
    records_archived INTEGER NOT NULL,
    parquet_file TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_archive_log_date ON archive_log(archive_date);
