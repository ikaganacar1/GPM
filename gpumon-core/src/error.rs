use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpuMonError {
    #[error("NVML initialization failed: {0}")]
    NvmlInitError(String),

    #[error("NVML operation failed: {0}")]
    NvmlError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Parquet error: {0}")]
    ParquetError(String),

    #[error("Process monitoring error: {0}")]
    ProcessError(String),

    #[error("Ollama monitoring error: {0}")]
    OllamaError(String),

    #[error("Service not available: {0}")]
    ServiceUnavailable(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

pub type Result<T> = std::result::Result<T, GpuMonError>;
