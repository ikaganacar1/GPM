use gpumon_core::{
    api::ApiState,
    config::GpuMonConfig,
    gpu::GpuMonitorBackend,
    init_logging,
    storage::Database,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    init_logging();

    info!("GPM - Web Server Mode");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = match GpuMonConfig::load() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            info!("Using default configuration");
            GpuMonConfig::default()
        }
    };

    info!("Configuration loaded");
    info!("  Data directory: {}", config.data_path().display());

    // Initialize database
    let db_path = config.database_path();
    let db = match Database::new(&db_path).await {
        Ok(db) => db,
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize GPU monitor
    let gpu_monitor = GpuMonitorBackend::initialize(&config).ok();

    // Create API state
    let api_state = ApiState {
        db: Arc::new(db),
        gpu_monitor: Arc::new(Mutex::new(gpu_monitor)),
    };

    // Start web server
    let port = 8010; // API server port
    info!("Starting web API server on port {}", port);

    if let Err(e) = gpumon_core::api::start_server(port, api_state).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}
