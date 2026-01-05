use crate::error::{GpmError, Result};
use nvml_wrapper::{Device, Nvml};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

static NVML_INSTANCE: OnceCell<Arc<Nvml>> = OnceCell::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMetrics {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub gpu_id: u32,
    pub name: String,
    pub utilization_gpu: u32,
    pub utilization_memory: u32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: u32,
    pub power_usage: u32,
    pub processes: Vec<GpuProcess>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuProcess {
    pub pid: u32,
    pub name: String,
    pub used_gpu_memory: u64,
}

pub struct NvmlMonitor {
    nvml: Arc<Nvml>,
    device_count: u32,
}

impl NvmlMonitor {
    pub fn new() -> Result<Self> {
        let nvml = NVML_INSTANCE.get_or_try_init(|| {
            info!("Initializing NVML");
            Nvml::init()
                .map(Arc::new)
                .map_err(|e| {
                    error!("Failed to initialize NVML: {:?}", e);
                    GpmError::NvmlInitError(format!("{:?}", e))
                })
        })?;

        let device_count = nvml.device_count()
            .map_err(|e| {
                error!("Failed to get device count: {:?}", e);
                GpmError::NvmlError(format!("Failed to get device count: {:?}", e))
            })?;

        info!("NVML initialized successfully with {} device(s)", device_count);

        Ok(Self {
            nvml: Arc::clone(nvml),
            device_count,
        })
    }

    pub fn device_count(&self) -> u32 {
        self.device_count
    }

    pub fn collect_metrics(&self) -> Result<Vec<GpuMetrics>> {
        let mut all_metrics = Vec::new();

        for i in 0..self.device_count {
            match self.collect_device_metrics(i) {
                Ok(metrics) => all_metrics.push(metrics),
                Err(e) => {
                    warn!("Failed to collect metrics for GPU {}: {}", i, e);
                }
            }
        }

        if all_metrics.is_empty() && self.device_count > 0 {
            return Err(GpmError::NvmlError(
                "Failed to collect metrics from any GPU".to_string()
            ));
        }

        Ok(all_metrics)
    }

    fn collect_device_metrics(&self, index: u32) -> Result<GpuMetrics> {
        let device = self.nvml.device_by_index(index)
            .map_err(|e| GpmError::NvmlError(format!("Failed to get device {}: {:?}", index, e)))?;

        let name = device.name()
            .unwrap_or_else(|_| format!("GPU {}", index));

        let utilization = device.utilization_rates()
            .map_err(|e| GpmError::NvmlError(format!("Failed to get utilization: {:?}", e)))?;

        let memory_info = device.memory_info()
            .map_err(|e| GpmError::NvmlError(format!("Failed to get memory info: {:?}", e)))?;

        let temperature = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .unwrap_or(0);

        let power_usage = device.power_usage()
            .map(|p| p / 1000)
            .unwrap_or(0);

        let processes = self.get_running_processes(&device)?;

        debug!(
            "GPU {} metrics: util={}%, mem={}%, temp={}°C, power={}W, processes={}",
            index,
            utilization.gpu,
            utilization.memory,
            temperature,
            power_usage,
            processes.len()
        );

        Ok(GpuMetrics {
            timestamp: chrono::Utc::now(),
            gpu_id: index,
            name,
            utilization_gpu: utilization.gpu,
            utilization_memory: utilization.memory,
            memory_used: memory_info.used,
            memory_total: memory_info.total,
            temperature,
            power_usage,
            processes,
        })
    }

    fn get_running_processes(&self, device: &Device) -> Result<Vec<GpuProcess>> {
        let compute_processes = device.running_compute_processes()
            .unwrap_or_else(|_| Vec::new());

        let graphics_processes = device.running_graphics_processes()
            .unwrap_or_else(|_| Vec::new());

        let mut all_processes = Vec::new();

        for proc in compute_processes.into_iter().chain(graphics_processes) {
            let pid = proc.pid;
            let name = Self::get_process_name(pid);
            let used_gpu_memory = match proc.used_gpu_memory {
                nvml_wrapper::enums::device::UsedGpuMemory::Used(bytes) => bytes,
                nvml_wrapper::enums::device::UsedGpuMemory::Unavailable => 0,
            };

            all_processes.push(GpuProcess {
                pid,
                name,
                used_gpu_memory,
            });
        }

        all_processes.sort_by_key(|p| std::cmp::Reverse(p.used_gpu_memory));
        Ok(all_processes)
    }

    fn get_process_name(pid: u32) -> String {
        use sysinfo::{System, ProcessesToUpdate};

        let mut system = System::new();
        let pid_sysinfo = sysinfo::Pid::from_u32(pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[pid_sysinfo]), true);

        system
            .process(pid_sysinfo)
            .map(|p| p.name().to_string_lossy().to_string())
            .unwrap_or_else(|| format!("pid_{}", pid))
    }
}

pub struct NvmlFallbackMonitor;

impl NvmlFallbackMonitor {
    pub fn collect_metrics() -> Result<Vec<GpuMetrics>> {
        warn!("Using nvidia-smi fallback - performance may be degraded");

        let output = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=index,name,utilization.gpu,utilization.memory,memory.used,memory.total,temperature.gpu,power.draw",
                "--format=csv,noheader,nounits"
            ])
            .output()
            .map_err(|e| GpmError::NvmlError(format!("Failed to run nvidia-smi: {}", e)))?;

        if !output.status.success() {
            return Err(GpmError::NvmlError(
                "nvidia-smi command failed".to_string()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut metrics = Vec::new();

        for line in stdout.lines() {
            if let Some(m) = Self::parse_nvidia_smi_line(line) {
                metrics.push(m);
            }
        }

        Ok(metrics)
    }

    fn parse_nvidia_smi_line(line: &str) -> Option<GpuMetrics> {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

        if parts.len() < 8 {
            return None;
        }

        Some(GpuMetrics {
            timestamp: chrono::Utc::now(),
            gpu_id: parts[0].parse().ok()?,
            name: parts[1].to_string(),
            utilization_gpu: parts[2].parse().ok()?,
            utilization_memory: parts[3].parse().ok()?,
            memory_used: parts[4].parse::<u64>().ok()? * 1024 * 1024,
            memory_total: parts[5].parse::<u64>().ok()? * 1024 * 1024,
            temperature: parts[6].parse().ok()?,
            power_usage: parts[7].parse::<f64>().ok()? as u32,
            processes: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nvidia_smi_line() {
        let line = "0, NVIDIA GeForce RTX 3080, 45, 30, 8192, 10240, 65, 250.5";
        let metrics = NvmlFallbackMonitor::parse_nvidia_smi_line(line).unwrap();

        assert_eq!(metrics.gpu_id, 0);
        assert_eq!(metrics.name, "NVIDIA GeForce RTX 3080");
        assert_eq!(metrics.utilization_gpu, 45);
        assert_eq!(metrics.utilization_memory, 30);
        assert_eq!(metrics.temperature, 65);
    }
}
