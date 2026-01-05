pub mod nvml;

pub use nvml::{GpuMetrics, GpuProcess, NvmlMonitor, NvmlFallbackMonitor};

use crate::{config::GpuMonConfig, error::Result};
use tracing::{info, warn};

pub enum GpuMonitorBackend {
    Nvml(NvmlMonitor),
    Fallback,
}

impl GpuMonitorBackend {
    pub fn initialize(config: &GpuMonConfig) -> Result<Self> {
        if config.gpu.enable_nvml {
            match NvmlMonitor::new() {
                Ok(monitor) => {
                    info!("Using NVML backend");
                    return Ok(Self::Nvml(monitor));
                }
                Err(e) => {
                    warn!("NVML initialization failed: {}", e);
                    if config.gpu.fallback_to_nvidia_smi {
                        info!("Falling back to nvidia-smi");
                        return Ok(Self::Fallback);
                    }
                    return Err(e);
                }
            }
        }

        if config.gpu.fallback_to_nvidia_smi {
            info!("Using nvidia-smi backend (by configuration)");
            Ok(Self::Fallback)
        } else {
            Err(crate::error::GpuMonError::ServiceUnavailable(
                "No GPU monitoring backend available".to_string()
            ))
        }
    }

    pub fn collect_metrics(&self) -> Result<Vec<GpuMetrics>> {
        match self {
            Self::Nvml(monitor) => monitor.collect_metrics(),
            Self::Fallback => NvmlFallbackMonitor::collect_metrics(),
        }
    }

    pub fn device_count(&self) -> u32 {
        match self {
            Self::Nvml(monitor) => monitor.device_count(),
            Self::Fallback => {
                NvmlFallbackMonitor::collect_metrics()
                    .map(|m| m.len() as u32)
                    .unwrap_or(0)
            }
        }
    }
}
