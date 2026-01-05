use crate::classifier::ProcessClassifier;
use crate::config::GpuMonConfig;
use crate::error::Result;
use crate::gpu::GpuMonitorBackend;
use crate::ollama::OllamaMonitor;
use crate::storage::StorageManager;
use crate::telemetry::TelemetryManager;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

pub struct GpuMonService {
    config: GpuMonConfig,
    gpu_monitor: Arc<RwLock<GpuMonitorBackend>>,
    process_classifier: Arc<RwLock<ProcessClassifier>>,
    ollama_monitor: Arc<OllamaMonitor>,
    storage: Arc<StorageManager>,
    telemetry: Arc<TelemetryManager>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl GpuMonService {
    pub async fn new(config: GpuMonConfig) -> Result<Self> {
        info!("Initializing GPU Monitoring Service");

        let gpu_monitor = Arc::new(RwLock::new(
            GpuMonitorBackend::initialize(&config)?
        ));

        let process_classifier = Arc::new(RwLock::new(ProcessClassifier::new()));

        let ollama_monitor = Arc::new(OllamaMonitor::new(config.ollama.api_url.clone()));

        let storage = Arc::new(StorageManager::new(&config).await?);

        let telemetry = Arc::new(TelemetryManager::new(&config)?);

        if telemetry.prometheus.is_some() {
            telemetry.start_prometheus_server(config.telemetry.metrics_port).await?;
        }

        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);

        info!("GPU Monitoring Service initialized");

        Ok(Self {
            config,
            gpu_monitor,
            process_classifier,
            ollama_monitor,
            storage,
            telemetry,
            shutdown_tx,
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting GPU Monitoring Service");

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let metrics_task = self.spawn_metrics_collector();
        let ollama_task = self.spawn_ollama_monitor();
        let maintenance_task = self.spawn_maintenance_worker();

        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl+C received, shutting down");
            }
        }

        let _ = self.shutdown_tx.send(());

        tokio::try_join!(metrics_task, ollama_task, maintenance_task)?;

        info!("GPU Monitoring Service stopped");
        Ok(())
    }

    async fn spawn_metrics_collector(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(self.config.service.poll_interval_secs));
        let gpu_monitor = Arc::clone(&self.gpu_monitor);
        let classifier = Arc::clone(&self.process_classifier);
        let storage = Arc::clone(&self.storage);
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.collect_and_store_metrics(
                        &gpu_monitor,
                        &classifier,
                        &storage
                    ).await {
                        error!("Failed to collect metrics: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Metrics collector shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn collect_and_store_metrics(
        &self,
        gpu_monitor: &Arc<RwLock<GpuMonitorBackend>>,
        classifier: &Arc<RwLock<ProcessClassifier>>,
        storage: &Arc<StorageManager>,
    ) -> Result<()> {
        let gpu_metrics = {
            let monitor = gpu_monitor.read().await;
            monitor.collect_metrics()?
        };

        for metrics in &gpu_metrics {
            storage.database.insert_gpu_metrics(metrics).await?;

            if let Some(otel_metrics) = &self.telemetry.metrics {
                otel_metrics.record_gpu_metrics(metrics);
            }

            if let Some(prom) = &self.telemetry.prometheus {
                prom.update_gpu_metrics(metrics);
            }
        }

        let classified_processes = {
            let mut clf = classifier.write().await;
            clf.classify_gpu_processes(&gpu_metrics)
        };

        for process in &classified_processes {
            storage.database.insert_process_event(process).await?;
        }

        if let Some(prom) = &self.telemetry.prometheus {
            prom.update_process_metrics(&classified_processes);
        }

        if let Some(otel_metrics) = &self.telemetry.metrics {
            otel_metrics.record_process_metrics(&classified_processes);
        }

        debug!(
            "Collected metrics from {} GPU(s), classified {} processes",
            gpu_metrics.len(),
            classified_processes.len()
        );

        Ok(())
    }

    async fn spawn_ollama_monitor(&self) -> Result<()> {
        if !self.config.ollama.enabled {
            info!("Ollama monitoring disabled");
            return Ok(());
        }

        let mut interval = interval(Duration::from_secs(5));
        let ollama_monitor = Arc::clone(&self.ollama_monitor);
        let storage = Arc::clone(&self.storage);
        let telemetry = Arc::clone(&self.telemetry);
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = ollama_monitor.check_and_track_logs().await {
                        warn!("Failed to check Ollama logs: {}", e);
                    }

                    let sessions = ollama_monitor.get_completed_sessions().await;
                    for session in sessions {
                        if let Err(e) = storage.database.insert_llm_session(&session).await {
                            error!("Failed to store LLM session: {}", e);
                        }

                        if let Some(otel_metrics) = &telemetry.metrics {
                            otel_metrics.record_llm_session(&session);
                        }

                        if let Some(prom) = &telemetry.prometheus {
                            prom.record_llm_session(&session);
                        }
                    }
                    ollama_monitor.clear_completed_sessions().await;
                }
                _ = shutdown_rx.recv() => {
                    info!("Ollama monitor shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn spawn_maintenance_worker(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(3600));
        let storage = Arc::clone(&self.storage);
        let config = self.config.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = storage.perform_maintenance(&config).await {
                        error!("Failed to perform maintenance: {}", e);
                    }

                    let current_week = chrono::Utc::now().date_naive().week(chrono::Weekday::Mon).first_day();
                    if let Err(e) = storage.database.compute_weekly_summary(current_week).await {
                        error!("Failed to compute weekly summary: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Maintenance worker shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_initialization() {
        let config = GpuMonConfig::default();
        let result = GpuMonService::new(config).await;

        assert!(result.is_ok() || result.is_err());
    }
}
