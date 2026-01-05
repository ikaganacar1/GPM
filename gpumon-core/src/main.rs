use gpumon_core::{init_logging, GpuMonConfig, GpuMonService};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    init_logging();

    info!("GPM - GPU & LLM Monitoring Service");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    let config = match GpuMonConfig::load() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            info!("Using default configuration");
            GpuMonConfig::default()
        }
    };

    info!("Configuration loaded");
    info!("  Poll interval: {}s", config.service.poll_interval_secs);
    info!("  Data directory: {}", config.data_path().display());
    info!("  Ollama monitoring: {}", config.ollama.enabled);
    info!("  Parquet archival: {}", config.storage.enable_parquet_archival);

    let service = match GpuMonService::new(config).await {
        Ok(service) => service,
        Err(e) => {
            error!("Failed to initialize service: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = service.run().await {
        error!("Service error: {}", e);
        std::process::exit(1);
    }

    info!("GPM service terminated gracefully");
}
