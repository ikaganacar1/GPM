use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    gpu::{GpuMonitorBackend, GpuMetrics},
    storage::Database,
};

/// API state shared across routes
#[derive(Clone)]
pub struct ApiState {
    pub db: Arc<Database>,
    pub gpu_monitor: Arc<Mutex<Option<GpuMonitorBackend>>>,
}

/// Create API router
pub fn create_router(state: ApiState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/info", get(get_dashboard_info))
        .route("/api/realtime", get(get_realtime_metrics))
        .route("/api/historical", get(get_historical_metrics))
        .route("/api/chart", get(get_chart_data))
        .route("/api/llm-sessions", get(get_llm_sessions))
        .with_state(state)
        .layer(cors)
}

/// Start the web API server
pub async fn start_server(port: u16, state: ApiState) -> Result<(), crate::error::GpmError> {
    let app = create_router(state);
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| crate::error::GpmError::ServiceUnavailable(format!("Failed to bind to port {}: {}", port, e)))?;

    tracing::info!("Web API server starting on http://localhost:{}", port);

    axum::serve(listener, app)
        .await
        .map_err(|e| crate::error::GpmError::ServiceUnavailable(format!("Server error: {}", e)))?;

    Ok(())
}

// ============= Response Types =============

#[derive(Debug, serde::Serialize)]
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

#[derive(Debug, serde::Serialize)]
pub struct DashboardInfo {
    pub gpu_count: u32,
    pub database_path: String,
    pub config_path: String,
    pub has_gpu_monitor: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct ChartDataResponse {
    pub labels: Vec<String>,
    pub utilization_gpu: Vec<u32>,
    pub utilization_memory: Vec<u32>,
    pub memory_percent: Vec<f64>,
    pub temperature: Vec<u32>,
    pub power_usage: Vec<u32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ChartParams {
    pub gpu_id: u32,
    pub hours: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct HistoricalParams {
    pub hours: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct LlmSessionParams {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, serde::Serialize)]
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

// ============= Handlers =============

async fn get_dashboard_info(State(state): State<ApiState>) -> Result<Json<DashboardInfo>, ApiError> {
    let gpu_monitor = state.gpu_monitor.lock().await;
    let gpu_count = match gpu_monitor.as_ref() {
        Some(m) => m.device_count(),
        None => 0,
    };

    Ok(Json(DashboardInfo {
        gpu_count,
        database_path: "~/.local/share/gpm/gpm.db".to_string(),
        config_path: "~/.config/gpm/config.toml".to_string(),
        has_gpu_monitor: gpu_monitor.is_some(),
    }))
}

async fn get_realtime_metrics(State(state): State<ApiState>) -> Result<Json<Vec<GpuMetricData>>, ApiError> {
    let gpu_monitor = state.gpu_monitor.lock().await;

    let metrics = match gpu_monitor.as_ref() {
        Some(m) => m.collect_metrics(),
        None => return Err(ApiError::BadRequest("GPU monitor not available".to_string())),
    };

    match metrics {
        Ok(m) => Ok(Json(m.into_iter().map(GpuMetricData::from).collect())),
        Err(e) => Err(ApiError::Internal(format!("Failed to collect metrics: {}", e))),
    }
}

async fn get_historical_metrics(
    State(state): State<ApiState>,
    Query(params): Query<HistoricalParams>,
) -> Result<Json<Vec<GpuMetricData>>, ApiError> {
    let metrics = state
        .db
        .get_recent_gpu_metrics(params.hours)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get metrics: {}", e)))?;

    Ok(Json(metrics.into_iter().map(GpuMetricData::from).collect()))
}

async fn get_chart_data(
    State(state): State<ApiState>,
    Query(params): Query<ChartParams>,
) -> Result<Json<ChartDataResponse>, ApiError> {
    let metrics = state
        .db
        .get_recent_gpu_metrics(params.hours)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get metrics: {}", e)))?;

    let gpu_metrics: Vec<_> = metrics
        .into_iter()
        .filter(|m| m.gpu_id == params.gpu_id)
        .map(GpuMetricData::from)
        .collect();

    Ok(Json(ChartDataResponse {
        labels: gpu_metrics.iter().map(|m| m.timestamp.clone()).collect(),
        utilization_gpu: gpu_metrics.iter().map(|m| m.utilization_gpu).collect(),
        utilization_memory: gpu_metrics.iter().map(|m| m.utilization_memory).collect(),
        memory_percent: gpu_metrics.iter().map(|m| m.memory_percent).collect(),
        temperature: gpu_metrics.iter().map(|m| m.temperature).collect(),
        power_usage: gpu_metrics.iter().map(|m| m.power_usage).collect(),
    }))
}

async fn get_llm_sessions(
    State(state): State<ApiState>,
    Query(params): Query<LlmSessionParams>,
) -> Result<Json<Vec<LlmSessionData>>, ApiError> {
    let start = chrono::DateTime::parse_from_rfc3339(&params.start_date)
        .map_err(|_| ApiError::BadRequest("Invalid start_date format".to_string()))?
        .with_timezone(&chrono::Utc);

    let end = chrono::DateTime::parse_from_rfc3339(&params.end_date)
        .map_err(|_| ApiError::BadRequest("Invalid end_date format".to_string()))?
        .with_timezone(&chrono::Utc);

    let sessions = state
        .db
        .get_llm_sessions(start, end)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get LLM sessions: {}", e)))?;

    Ok(Json(sessions
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
        .collect()))
}

// ============= Error Types =============

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
}
