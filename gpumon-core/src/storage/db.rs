use crate::classifier::{ClassifiedProcess, WorkloadCategory};
use crate::error::Result;
use crate::gpu::GpuMetrics;
use crate::ollama::LlmSession;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::Path;
use std::str::FromStr;
use tracing::info;

pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref();

        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))?
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        info!("Database connected at {}", db_path.display());

        let db = Self { pool };
        db.initialize_schema().await?;

        Ok(db)
    }

    async fn initialize_schema(&self) -> Result<()> {
        let schema = include_str!("schema.sql");

        sqlx::query(schema)
            .execute(&self.pool)
            .await?;

        info!("Database schema initialized");
        Ok(())
    }

    pub async fn insert_gpu_metrics(&self, metrics: &GpuMetrics) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO gpu_metrics (
                timestamp, gpu_id, name, utilization_gpu, utilization_memory,
                memory_used, memory_total, temperature, power_usage
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&metrics.timestamp)
        .bind(metrics.gpu_id)
        .bind(&metrics.name)
        .bind(metrics.utilization_gpu)
        .bind(metrics.utilization_memory)
        .bind(metrics.memory_used as i64)
        .bind(metrics.memory_total as i64)
        .bind(metrics.temperature)
        .bind(metrics.power_usage)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_llm_session(&self, session: &LlmSession) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO llm_sessions (
                id, start_time, end_time, model, prompt_tokens, completion_tokens,
                total_tokens, tokens_per_second, time_to_first_token_ms, time_per_output_token_ms
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                end_time = excluded.end_time,
                completion_tokens = excluded.completion_tokens,
                total_tokens = excluded.total_tokens,
                tokens_per_second = excluded.tokens_per_second,
                time_to_first_token_ms = excluded.time_to_first_token_ms,
                time_per_output_token_ms = excluded.time_per_output_token_ms
            "#,
        )
        .bind(&session.id)
        .bind(&session.start_time)
        .bind(&session.end_time)
        .bind(&session.model)
        .bind(session.prompt_tokens as i64)
        .bind(session.completion_tokens as i64)
        .bind(session.total_tokens as i64)
        .bind(session.tokens_per_second)
        .bind(session.time_to_first_token_ms.map(|t| t as i64))
        .bind(session.time_per_output_token_ms)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_process_event(&self, process: &ClassifiedProcess) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO process_events (
                timestamp, pid, name, category, gpu_memory_mb, gpu_utilization,
                command_line, exe_path
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(chrono::Utc::now())
        .bind(process.pid as i64)
        .bind(&process.name)
        .bind(process.category.as_str())
        .bind(process.gpu_memory_mb as i64)
        .bind(process.gpu_utilization)
        .bind(&process.command_line)
        .bind(process.exe_path.as_ref().map(|p| p.to_string_lossy().to_string()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_recent_gpu_metrics(&self, hours: i64) -> Result<Vec<GpuMetrics>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);

        let rows = sqlx::query_as::<_, (String, i64, String, i64, i64, i64, i64, i64, i64)>(
            r#"
            SELECT timestamp, gpu_id, name, utilization_gpu, utilization_memory,
                   memory_used, memory_total, temperature, power_usage
            FROM gpu_metrics
            WHERE timestamp >= ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;

        let metrics = rows
            .into_iter()
            .filter_map(|row| {
                Some(GpuMetrics {
                    timestamp: chrono::DateTime::parse_from_rfc3339(&row.0)
                        .ok()?
                        .with_timezone(&chrono::Utc),
                    gpu_id: row.1 as u32,
                    name: row.2,
                    utilization_gpu: row.3 as u32,
                    utilization_memory: row.4 as u32,
                    memory_used: row.5 as u64,
                    memory_total: row.6 as u64,
                    temperature: row.7 as u32,
                    power_usage: row.8 as u32,
                    processes: Vec::new(),
                })
            })
            .collect();

        Ok(metrics)
    }

    pub async fn get_llm_sessions(
        &self,
        start_date: chrono::DateTime<chrono::Utc>,
        end_date: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<LlmSession>> {
        let rows = sqlx::query_as::<_, (
            String,
            String,
            Option<String>,
            String,
            i64,
            i64,
            i64,
            f64,
            Option<i64>,
            Option<f64>,
        )>(
            r#"
            SELECT id, start_time, end_time, model, prompt_tokens, completion_tokens,
                   total_tokens, tokens_per_second, time_to_first_token_ms, time_per_output_token_ms
            FROM llm_sessions
            WHERE start_time >= ? AND start_time <= ?
            ORDER BY start_time DESC
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        let sessions = rows
            .into_iter()
            .filter_map(|row| {
                Some(LlmSession {
                    id: row.0,
                    start_time: chrono::DateTime::parse_from_rfc3339(&row.1)
                        .ok()?
                        .with_timezone(&chrono::Utc),
                    end_time: row
                        .2
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc)),
                    model: row.3,
                    prompt_tokens: row.4 as u64,
                    completion_tokens: row.5 as u64,
                    total_tokens: row.6 as u64,
                    tokens_per_second: row.7,
                    time_to_first_token_ms: row.8.map(|t| t as u64),
                    time_per_output_token_ms: row.9,
                })
            })
            .collect();

        Ok(sessions)
    }

    pub async fn cleanup_old_data(&self, retention_days: i64) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);

        let result = sqlx::query("DELETE FROM gpu_metrics WHERE timestamp < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        let deleted_count = result.rows_affected() as usize;

        if deleted_count > 0 {
            info!("Cleaned up {} old GPU metrics records", deleted_count);
        }

        Ok(deleted_count)
    }

    pub async fn compute_weekly_summary(
        &self,
        week_start: chrono::NaiveDate,
    ) -> Result<()> {
        let week_end = week_start + chrono::Duration::days(7);

        for category in &[
            WorkloadCategory::Gaming,
            WorkloadCategory::LlmInference,
            WorkloadCategory::MlTraining,
            WorkloadCategory::GeneralCompute,
        ] {
            let category_str = category.as_str();

            let row = sqlx::query_as::<_, (i64, f64, i64, i64, i64)>(
                r#"
                SELECT
                    COUNT(*) as event_count,
                    AVG(gpu_utilization) as avg_util,
                    MAX(gpu_utilization) as max_util,
                    SUM(gpu_memory_mb) as total_mem,
                    SUM(duration_secs) as total_duration
                FROM process_events
                WHERE category = ?
                  AND DATE(timestamp) >= ?
                  AND DATE(timestamp) < ?
                "#,
            )
            .bind(category_str)
            .bind(week_start)
            .bind(week_end)
            .fetch_one(&self.pool)
            .await?;

            if row.0 > 0 {
                sqlx::query(
                    r#"
                    INSERT INTO weekly_summaries (
                        week_start, week_end, category, total_duration_secs,
                        avg_gpu_utilization, max_gpu_utilization, total_gpu_memory_mb, event_count
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(week_start, category) DO UPDATE SET
                        total_duration_secs = excluded.total_duration_secs,
                        avg_gpu_utilization = excluded.avg_gpu_utilization,
                        max_gpu_utilization = excluded.max_gpu_utilization,
                        total_gpu_memory_mb = excluded.total_gpu_memory_mb,
                        event_count = excluded.event_count
                    "#,
                )
                .bind(week_start)
                .bind(week_end)
                .bind(category_str)
                .bind(row.4)
                .bind(row.1)
                .bind(row.2)
                .bind(row.3)
                .bind(row.0)
                .execute(&self.pool)
                .await?;
            }
        }

        Ok(())
    }
}
