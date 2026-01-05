use crate::gpu::GpuMetrics;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use sysinfo::{ProcessRefreshKind, System};
use tracing::{debug, trace};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkloadCategory {
    Gaming,
    LlmInference,
    MlTraining,
    GeneralCompute,
    Unknown,
}

impl WorkloadCategory {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Gaming => "gaming",
            Self::LlmInference => "llm_inference",
            Self::MlTraining => "ml_training",
            Self::GeneralCompute => "general_compute",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedProcess {
    pub pid: u32,
    pub name: String,
    pub category: WorkloadCategory,
    pub gpu_memory_mb: u64,
    pub gpu_utilization: u32,
    pub command_line: String,
    pub exe_path: Option<PathBuf>,
}

pub struct ProcessClassifier {
    system: System,
    game_patterns: Vec<Regex>,
    ml_patterns: Vec<Regex>,
    steam_library_paths: Vec<PathBuf>,
}

impl ProcessClassifier {
    pub fn new() -> Self {
        let game_patterns = vec![
            Regex::new(r"(?i).*\.exe$").unwrap(),
            Regex::new(r"(?i).*-dx12\.exe$").unwrap(),
            Regex::new(r"(?i).*-vulkan\.exe$").unwrap(),
            Regex::new(r"(?i).*game.*\.exe$").unwrap(),
            Regex::new(r"(?i).*(unity|unreal).*\.exe$").unwrap(),
        ];

        let ml_patterns = vec![
            Regex::new(r"(?i)python.*").unwrap(),
            Regex::new(r"(?i).*jupyter.*").unwrap(),
        ];

        let steam_library_paths = Self::discover_steam_libraries();

        Self {
            system: System::new(),
            game_patterns,
            ml_patterns,
            steam_library_paths,
        }
    }

    pub fn classify_gpu_processes(
        &mut self,
        gpu_metrics: &[GpuMetrics],
    ) -> Vec<ClassifiedProcess> {
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cmd(sysinfo::UpdateKind::OnlyIfNotSet)
                .with_exe(sysinfo::UpdateKind::OnlyIfNotSet)
        );

        let mut classified = Vec::new();
        let mut pid_to_metrics = HashMap::new();

        for metrics in gpu_metrics {
            for proc in &metrics.processes {
                pid_to_metrics.insert(
                    proc.pid,
                    (proc.used_gpu_memory, metrics.utilization_gpu),
                );
            }
        }

        for (pid, (gpu_memory, gpu_util)) in pid_to_metrics {
            if let Some(process_info) = self.classify_process(pid, gpu_memory, gpu_util) {
                classified.push(process_info);
            }
        }

        classified
    }

    fn classify_process(
        &self,
        pid: u32,
        gpu_memory: u64,
        gpu_utilization: u32,
    ) -> Option<ClassifiedProcess> {
        let process = self.system.process(sysinfo::Pid::from_u32(pid))?;

        let name = process.name().to_string_lossy().to_string();
        let command_line = process.cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        let exe_path = process.exe().map(|p| p.to_path_buf());

        let category = self.determine_category(
            &name,
            &command_line,
            exe_path.as_ref(),
            gpu_utilization,
        );

        debug!(
            "Classified process: pid={} name={} category={:?} gpu_mem={}MB",
            pid,
            name,
            category,
            gpu_memory / 1024 / 1024
        );

        Some(ClassifiedProcess {
            pid,
            name,
            category,
            gpu_memory_mb: gpu_memory / 1024 / 1024,
            gpu_utilization,
            command_line,
            exe_path,
        })
    }

    fn determine_category(
        &self,
        name: &str,
        cmdline: &str,
        exe_path: Option<&PathBuf>,
        gpu_util: u32,
    ) -> WorkloadCategory {
        if name.to_lowercase().contains("ollama") {
            return WorkloadCategory::LlmInference;
        }

        if self.is_ml_framework(cmdline) {
            return WorkloadCategory::MlTraining;
        }

        if self.is_python_ml(name, cmdline) {
            if self.looks_like_inference(cmdline) {
                return WorkloadCategory::LlmInference;
            } else {
                return WorkloadCategory::MlTraining;
            }
        }

        if self.is_game(name, exe_path, gpu_util) {
            return WorkloadCategory::Gaming;
        }

        WorkloadCategory::GeneralCompute
    }

    fn is_game(&self, name: &str, exe_path: Option<&PathBuf>, gpu_util: u32) -> bool {
        if let Some(path) = exe_path {
            if self.is_in_steam_library(path) {
                trace!("Process {} is in Steam library", name);
                return true;
            }

            let path_str = path.to_string_lossy();
            if path_str.to_lowercase().contains("game") {
                return true;
            }
        }

        for pattern in &self.game_patterns {
            if pattern.is_match(name) && gpu_util > 60 {
                trace!("Process {} matches game pattern with high GPU usage", name);
                return true;
            }
        }

        false
    }

    fn is_python_ml(&self, name: &str, cmdline: &str) -> bool {
        if !name.to_lowercase().contains("python") {
            return false;
        }

        let ml_keywords = [
            "transformers", "torch", "tensorflow", "keras",
            "pytorch", "jax", "flax", "diffusers", "vllm",
            "llama", "huggingface", "model.py", "train.py"
        ];

        ml_keywords.iter().any(|kw| cmdline.to_lowercase().contains(kw))
    }

    fn is_ml_framework(&self, cmdline: &str) -> bool {
        let cmdline_lower = cmdline.to_lowercase();
        cmdline_lower.contains("tensorflow") ||
        cmdline_lower.contains("torch") ||
        cmdline_lower.contains("jax") ||
        cmdline_lower.contains("mxnet")
    }

    fn looks_like_inference(&self, cmdline: &str) -> bool {
        let cmdline_lower = cmdline.to_lowercase();
        cmdline_lower.contains("generate") ||
        cmdline_lower.contains("inference") ||
        cmdline_lower.contains("predict") ||
        cmdline_lower.contains("serve") ||
        cmdline_lower.contains("api")
    }

    fn is_in_steam_library(&self, path: &PathBuf) -> bool {
        for steam_path in &self.steam_library_paths {
            if path.starts_with(steam_path) {
                return true;
            }
        }
        false
    }

    fn discover_steam_libraries() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(home) = dirs::home_dir() {
            let steam_dir = home.join(".steam/steam/steamapps/common");
            if steam_dir.exists() {
                paths.push(steam_dir);
            }

            let steam_flatpak = home.join(".var/app/com.valvesoftware.Steam/.steam/steam/steamapps/common");
            if steam_flatpak.exists() {
                paths.push(steam_flatpak);
            }
        }

        #[cfg(target_os = "windows")]
        {
            let program_files = std::env::var("ProgramFiles(x86)")
                .or_else(|_| std::env::var("ProgramFiles"))
                .ok();

            if let Some(pf) = program_files {
                let steam_dir = PathBuf::from(pf)
                    .join("Steam")
                    .join("steamapps")
                    .join("common");
                if steam_dir.exists() {
                    paths.push(steam_dir);
                }
            }
        }

        paths
    }
}

impl Default for ProcessClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_detection() {
        let classifier = ProcessClassifier::new();
        let category = classifier.determine_category(
            "ollama",
            "/usr/bin/ollama serve",
            None,
            50,
        );
        assert_eq!(category, WorkloadCategory::LlmInference);
    }

    #[test]
    fn test_python_ml_training() {
        let classifier = ProcessClassifier::new();
        let category = classifier.determine_category(
            "python3",
            "python3 train.py --model transformer --epochs 10",
            None,
            80,
        );
        assert_eq!(category, WorkloadCategory::MlTraining);
    }

    #[test]
    fn test_python_inference() {
        let classifier = ProcessClassifier::new();
        let category = classifier.determine_category(
            "python3",
            "python3 inference.py --model llama --generate",
            None,
            60,
        );
        assert_eq!(category, WorkloadCategory::LlmInference);
    }
}
