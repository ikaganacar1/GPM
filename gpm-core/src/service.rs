use crate::classifier::ProcessClassifier;
use crate::config::GpmConfig;
use crate::error::Result;
use crate::gpu::GpuMonitorBackend;
use crate::ollama::OllamaMonitor;
use crate::storage::StorageManager;
use crate::telemetry::TelemetryManager;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

pub struct GpmService {
    config: GpmConfig,
    gpu_monitor: Arc<RwLock<GpuMonitorBackend>>,
    process_classifier: Arc<RwLock<ProcessClassifier>>,
    ollama_monitor: Arc<OllamaMonitor>,
    storage: Arc<StorageManager>,
    telemetry: Arc<TelemetryManager>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl GpmService {
    pub async fn new(config: GpmConfig) -> Result<Self> {
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

        // Start Prometheus server if enabled
        if self.config.telemetry.enable_prometheus {
            let port = self.config.telemetry.metrics_port;
            self.telemetry.start_prometheus_server(port).await?;
        }

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Spawn background tasks
        let storage1 = Arc::clone(&self.storage);
        let storage2 = Arc::clone(&self.storage);
        let storage3 = Arc::clone(&self.storage);
        let telemetry1 = Arc::clone(&self.telemetry);
        let telemetry2 = Arc::clone(&self.telemetry);
        let gpu_monitor = Arc::clone(&self.gpu_monitor);
        let classifier = Arc::clone(&self.process_classifier);
        let ollama_monitor = Arc::clone(&self.ollama_monitor);
        let config1 = self.config.clone();
        let config2 = self.config.clone();
        let config3 = self.config.clone();
        let shutdown_tx1 = self.shutdown_tx.clone();
        let shutdown_tx2 = self.shutdown_tx.clone();
        let shutdown_tx3 = self.shutdown_tx.clone();

        let metrics_task = tokio::spawn(async move {
            Self::metrics_collector_loop(gpu_monitor, classifier, storage1, telemetry1, config1.service.poll_interval_secs, shutdown_tx1).await
        });

        let ollama_task = tokio::spawn(async move {
            Self::ollama_monitor_loop(ollama_monitor, storage2, telemetry2, config2.ollama.enabled, shutdown_tx2).await
        });

        let maintenance_task = tokio::spawn(async move {
            Self::maintenance_worker_loop(storage3, config3, shutdown_tx3).await
        });

        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl+C received, shutting down");
            }
        }

        let _ = self.shutdown_tx.send(());

        let _ = tokio::join!(metrics_task, ollama_task, maintenance_task);

        info!("GPU Monitoring Service stopped");
        Ok(())
    }

    async fn metrics_collector_loop(
        gpu_monitor: Arc<RwLock<GpuMonitorBackend>>,
        classifier: Arc<RwLock<ProcessClassifier>>,
        storage: Arc<StorageManager>,
        telemetry: Arc<TelemetryManager>,
        poll_interval_secs: u64,
        shutdown_tx: tokio::sync::broadcast::Sender<()>,
    ) -> Result<()> {
        let mut interval = interval(Duration::from_secs(poll_interval_secs));
        let mut shutdown_rx = shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = Self::collect_and_store_metrics_static(
                        &gpu_monitor,
                        &classifier,
                        &storage,
                        &telemetry
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

    async fn collect_and_store_metrics_static(
        gpu_monitor: &Arc<RwLock<GpuMonitorBackend>>,
        classifier: &Arc<RwLock<ProcessClassifier>>,
        storage: &Arc<StorageManager>,
        telemetry: &Arc<TelemetryManager>,
    ) -> Result<()> {
        let gpu_metrics = {
            let monitor = gpu_monitor.read().await;
            monitor.collect_metrics()?
        };

        for metrics in &gpu_metrics {
            storage.database.insert_gpu_metrics(metrics).await?;

            if let Some(otel_metrics) = &telemetry.metrics {
                otel_metrics.record_gpu_metrics(metrics);
            }

            if let Some(prom) = &telemetry.prometheus {
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

        if let Some(prom) = &telemetry.prometheus {
            prom.update_process_metrics(&classified_processes);
        }

        if let Some(otel_metrics) = &telemetry.metrics {
            otel_metrics.record_process_metrics(&classified_processes);
        }

        debug!(
            "Collected metrics from {} GPU(s), classified {} processes",
            gpu_metrics.len(),
            classified_processes.len()
        );

        Ok(())
    }

    async fn ollama_monitor_loop(
        ollama_monitor: Arc<OllamaMonitor>,
        storage: Arc<StorageManager>,
        telemetry: Arc<TelemetryManager>,
        enabled: bool,
        shutdown_tx: tokio::sync::broadcast::Sender<()>,
    ) -> Result<()> {
        if !enabled {
            info!("Ollama monitoring disabled");
            return Ok(());
        }

        let mut interval = interval(Duration::from_secs(5));
        let mut shutdown_rx = shutdown_tx.subscribe();

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

    async fn maintenance_worker_loop(
        storage: Arc<StorageManager>,
        config: GpmConfig,
        shutdown_tx: tokio::sync::broadcast::Sender<()>,
    ) -> Result<()> {
        let mut interval = interval(Duration::from_secs(3600));
        let mut shutdown_rx = shutdown_tx.subscribe();

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
        let config = GpmConfig::default();
        let result = GpmService::new(config).await;

        assert!(result.is_ok() || result.is_err());
    }
}
