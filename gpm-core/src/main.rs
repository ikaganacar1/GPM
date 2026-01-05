use gpm_core::{init_logging, GpmConfig, GpmService};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    init_logging();

    info!("GPM - GPU & LLM Monitoring Service");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    let config = match GpmConfig::load() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            info!("Using default configuration");
            GpmConfig::default()
        }
    };

    info!("Configuration loaded");
    info!("  Poll interval: {}s", config.service.poll_interval_secs);
    info!("  Data directory: {}", config.data_path().display());
    info!("  Ollama monitoring: {}", config.ollama.enabled);
    info!("  Parquet archival: {}", config.storage.enable_parquet_archival);

    let service = match GpmService::new(config).await {
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
