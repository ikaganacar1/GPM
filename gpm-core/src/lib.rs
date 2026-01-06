pub mod api;
pub mod classifier;
pub mod config;
pub mod error;
pub mod gpu;
pub mod ollama;
pub mod proxy;
pub mod service;
pub mod storage;
pub mod telemetry;

pub use config::GpmConfig;
pub use error::{GpmError, Result};
pub use service::GpmService;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging() {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,gpm_core=debug"))
        )
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}
