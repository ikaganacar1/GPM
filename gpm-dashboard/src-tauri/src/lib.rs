use gpm_core::{
    config::GpmConfig,
    gpu::{GpuMonitorBackend, GpuMetrics},
    storage::Database,
    GpmError,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Dashboard state shared across Tauri commands
pub struct DashboardState {
    db: Arc<Database>,
    gpu_monitor: Arc<Mutex<Option<GpuMonitorBackend>>>,
    config_path: PathBuf,
}

impl DashboardState {
    async fn new() -> Result<Self, GpmError> {
        // Load config and get database path
        let config = GpmConfig::load().unwrap_or_default();
        let db_path = config.database_path();
        let config_path = config.config_path();

        // Initialize database connection
        let db = Database::new(&db_path).await?;

        // Initialize GPU monitor for real-time metrics
        let gpu_monitor = GpuMonitorBackend::initialize(&config).ok();

        Ok(Self {
            db: Arc::new(db),
            gpu_monitor: Arc::new(Mutex::new(gpu_monitor)),
            config_path,
        })
    }
}

/// GPU metrics for frontend (simplified version for JSON serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMetricData {
    pub timestamp: String,
    pub gpu_id: u32,
    pub name: String,
    pub utilization_gpu: u32,
    pub utilization_memory: u32,
    pub memory_used_mb: f64,
    pub memory_total_mb: f64,
    pub temperature: u32,
    pub power_usage: u32,
    pub memory_percent: f64,
}

impl From<GpuMetrics> for GpuMetricData {
    fn from(m: GpuMetrics) -> Self {
        let memory_used_mb = m.memory_used as f64 / (1024.0 * 1024.0);
        let memory_total_mb = m.memory_total as f64 / (1024.0 * 1024.0);
        let memory_percent = if m.memory_total > 0 {
            (m.memory_used as f64 / m.memory_total as f64) * 100.0
        } else {
            0.0
        };

        Self {
            timestamp: m.timestamp.to_rfc3339(),
            gpu_id: m.gpu_id,
            name: m.name,
            utilization_gpu: m.utilization_gpu,
            utilization_memory: m.utilization_memory,
            memory_used_mb,
            memory_total_mb,
            temperature: m.temperature,
            power_usage: m.power_usage,
            memory_percent,
        }
    }
}

/// LLM session data for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSessionData {
    pub id: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub tokens_per_second: f64,
    pub time_to_first_token_ms: Option<u64>,
    pub time_per_output_token_ms: Option<f64>,
}

/// Dashboard info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardInfo {
    pub gpu_count: u32,
    pub database_path: String,
    pub config_path: String,
    pub has_gpu_monitor: bool,
}

/// Error response for frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============= Tauri Commands =============

/// Get dashboard initialization info
#[tauri::command]
async fn get_dashboard_info(
    state: State<'_, DashboardState>,
) -> Result<DashboardInfo, ErrorResponse> {
    let gpu_monitor = state.gpu_monitor.lock().await;
    let gpu_count = match gpu_monitor.as_ref() {
        Some(m) => m.device_count(),
        None => 0,
    };

    Ok(DashboardInfo {
        gpu_count,
        database_path: state.db.pool.connect_options().to_string(),
        config_path: state.config_path.display().to_string(),
        has_gpu_monitor: gpu_monitor.is_some(),
    })
}

/// Get real-time GPU metrics (current snapshot, not from database)
#[tauri::command]
async fn get_realtime_metrics(
    state: State<'_, DashboardState>,
) -> Result<Vec<GpuMetricData>, ErrorResponse> {
    let gpu_monitor = state.gpu_monitor.lock().await;

    let metrics = match gpu_monitor.as_ref() {
        Some(m) => m.collect_metrics(),
        None => {
            return Err(ErrorResponse {
                error: "GPU monitor not available".to_string(),
            })
        }
    };

    match metrics {
        Ok(m) => Ok(m.into_iter().map(GpuMetricData::from).collect()),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to collect metrics: {}", e),
        }),
    }
}

/// Get historical GPU metrics from database
#[tauri::command]
async fn get_historical_metrics(
    state: State<'_, DashboardState>,
    hours: i64,
) -> Result<Vec<GpuMetricData>, ErrorResponse> {
    let metrics = state
        .db
        .get_recent_gpu_metrics(hours)
        .await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to get metrics: {}", e),
        })?;

    Ok(metrics.into_iter().map(GpuMetricData::from).collect())
}

/// Get LLM sessions from database
#[tauri::command]
async fn get_llm_sessions(
    state: State<'_, DashboardState>,
    start_date: String,
    end_date: String,
) -> Result<Vec<LlmSessionData>, ErrorResponse> {
    use chrono::DateTime;

    let start = DateTime::parse_from_rfc3339(&start_date)
        .map_err(|_| ErrorResponse {
            error: "Invalid start_date format".to_string(),
        })?
        .with_timezone(&chrono::Utc);

    let end = DateTime::parse_from_rfc3339(&end_date)
        .map_err(|_| ErrorResponse {
            error: "Invalid end_date format".to_string(),
        })?
        .with_timezone(&chrono::Utc);

    let sessions = state
        .db
        .get_llm_sessions(start, end)
        .await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to get LLM sessions: {}", e),
        })?;

    Ok(sessions
        .into_iter()
        .map(|s| LlmSessionData {
            id: s.id,
            start_time: s.start_time.to_rfc3339(),
            end_time: s.end_time.map(|t| t.to_rfc3339()),
            model: s.model,
            prompt_tokens: s.prompt_tokens,
            completion_tokens: s.completion_tokens,
            total_tokens: s.total_tokens,
            tokens_per_second: s.tokens_per_second,
            time_to_first_token_ms: s.time_to_first_token_ms,
            time_per_output_token_ms: s.time_per_output_token_ms,
        })
        .collect())
}

/// Get metrics aggregated for chart display (simplified for frontend)
#[tauri::command]
async fn get_chart_data(
    state: State<'_, DashboardState>,
    gpu_id: u32,
    hours: i64,
) -> Result<ChartDataResponse, ErrorResponse> {
    let metrics = state
        .db
        .get_recent_gpu_metrics(hours)
        .await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to get metrics: {}", e),
        })?;

    // Filter by GPU ID and convert to chart format
    let gpu_metrics: Vec<_> = metrics
        .into_iter()
        .filter(|m| m.gpu_id == gpu_id)
        .map(GpuMetricData::from)
        .collect();

    Ok(ChartDataResponse {
        labels: gpu_metrics.iter().map(|m| m.timestamp.clone()).collect(),
        utilization_gpu: gpu_metrics.iter().map(|m| m.utilization_gpu).collect(),
        utilization_memory: gpu_metrics.iter().map(|m| m.utilization_memory).collect(),
        memory_percent: gpu_metrics.iter().map(|m| m.memory_percent).collect(),
        temperature: gpu_metrics.iter().map(|m| m.temperature).collect(),
        power_usage: gpu_metrics.iter().map(|m| m.power_usage).collect(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataResponse {
    pub labels: Vec<String>,
    pub utilization_gpu: Vec<u32>,
    pub utilization_memory: Vec<u32>,
    pub memory_percent: Vec<f64>,
    pub temperature: Vec<u32>,
    pub power_usage: Vec<u32>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,gpm_core=debug,gpm_dashboard=debug"))
        )
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();

    // Create runtime for async initialization
    let runtime = tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime");

    // Initialize dashboard state
    let dashboard_state = runtime
        .block_on(DashboardState::new())
        .expect("Failed to initialize dashboard state");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(dashboard_state)
        .invoke_handler(tauri::generate_handler![
            get_dashboard_info,
            get_realtime_metrics,
            get_historical_metrics,
            get_llm_sessions,
            get_chart_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
