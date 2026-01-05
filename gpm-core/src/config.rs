use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpmConfig {
    pub service: ServiceConfig,
    pub gpu: GpuConfig,
    pub ollama: OllamaConfig,
    pub storage: StorageConfig,
    pub telemetry: TelemetryConfig,
    pub alerts: AlertConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,

    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    #[serde(default = "default_true")]
    pub enable_nvml: bool,

    #[serde(default)]
    pub fallback_to_nvidia_smi: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_ollama_port")]
    pub api_port: u16,

    #[serde(default = "default_ollama_url")]
    pub api_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    #[serde(default = "default_true")]
    pub enable_parquet_archival: bool,

    #[serde(default = "default_archive_dir")]
    pub archive_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    #[serde(default = "default_true")]
    pub enable_opentelemetry: bool,

    #[serde(default = "default_otlp_endpoint")]
    pub otlp_endpoint: String,

    #[serde(default = "default_true")]
    pub enable_prometheus: bool,

    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    #[serde(default = "default_temp_threshold")]
    pub temp_threshold_celsius: f64,

    #[serde(default = "default_mem_threshold")]
    pub memory_threshold_percent: f64,

    #[serde(default)]
    pub enable_desktop_notifications: bool,
}

impl Default for GpmConfig {
    fn default() -> Self {
        Self {
            service: ServiceConfig {
                poll_interval_secs: default_poll_interval(),
                data_dir: default_data_dir(),
            },
            gpu: GpuConfig {
                enable_nvml: true,
                fallback_to_nvidia_smi: false,
            },
            ollama: OllamaConfig {
                enabled: true,
                api_port: default_ollama_port(),
                api_url: default_ollama_url(),
            },
            storage: StorageConfig {
                retention_days: default_retention_days(),
                enable_parquet_archival: true,
                archive_dir: default_archive_dir(),
            },
            telemetry: TelemetryConfig {
                enable_opentelemetry: true,
                otlp_endpoint: default_otlp_endpoint(),
                enable_prometheus: true,
                metrics_port: default_metrics_port(),
            },
            alerts: AlertConfig {
                temp_threshold_celsius: default_temp_threshold(),
                memory_threshold_percent: default_mem_threshold(),
                enable_desktop_notifications: false,
            },
        }
    }
}

impl GpmConfig {
    pub fn load() -> crate::error::Result<Self> {
        let config_path = Self::config_path();

        let builder = config::Config::builder()
            .add_source(config::Config::try_from(&GpmConfig::default())?)
            .add_source(
                config::File::from(config_path)
                    .required(false)
            )
            .add_source(
                config::Environment::with_prefix("GPM")
                    .separator("_")
            );

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gpm")
            .join("config.toml")
    }

    pub fn data_path(&self) -> PathBuf {
        if self.service.data_dir.is_absolute() {
            self.service.data_dir.clone()
        } else {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("gpm")
        }
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_path().join("gpm.db")
    }
}

fn default_poll_interval() -> u64 { 2 }
fn default_retention_days() -> u32 { 7 }
fn default_ollama_port() -> u16 { 11434 }
fn default_ollama_url() -> String { "http://localhost:11434".to_string() }
fn default_metrics_port() -> u16 { 9090 }
fn default_temp_threshold() -> f64 { 85.0 }
fn default_mem_threshold() -> f64 { 90.0 }
fn default_otlp_endpoint() -> String { "http://localhost:4317".to_string() }
fn default_true() -> bool { true }

fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gpm")
}

fn default_archive_dir() -> PathBuf {
    default_data_dir().join("archive")
}
