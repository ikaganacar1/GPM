use crate::error::Result;
use axum::{routing::get, Router};
use prometheus::{Encoder, GaugeVec, HistogramVec, Opts, Registry, TextEncoder};
use std::sync::Arc;
use tracing::info;

pub struct PrometheusExporter {
    registry: Registry,

    // GPU metrics
    gpu_utilization: GaugeVec,
    gpu_memory_used: GaugeVec,
    gpu_memory_total: GaugeVec,
    gpu_temperature: GaugeVec,
    gpu_power: GaugeVec,

    // LLM metrics
    llm_tokens_per_second: HistogramVec,
    llm_time_to_first_token: HistogramVec,
    llm_session_count: GaugeVec,

    // Process metrics
    process_count: GaugeVec,
    process_gpu_memory: GaugeVec,
}

impl PrometheusExporter {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();

        let gpu_utilization = GaugeVec::new(
            Opts::new("gpm_gpu_utilization_percent", "GPU utilization percentage"),
            &["gpu_id", "gpu_name"],
        )?;

        let gpu_memory_used = GaugeVec::new(
            Opts::new("gpm_gpu_memory_used_bytes", "GPU memory used in bytes"),
            &["gpu_id", "gpu_name"],
        )?;

        let gpu_memory_total = GaugeVec::new(
            Opts::new("gpm_gpu_memory_total_bytes", "GPU total memory in bytes"),
            &["gpu_id", "gpu_name"],
        )?;

        let gpu_temperature = GaugeVec::new(
            Opts::new("gpm_gpu_temperature_celsius", "GPU temperature in Celsius"),
            &["gpu_id", "gpu_name"],
        )?;

        let gpu_power = GaugeVec::new(
            Opts::new("gpm_gpu_power_watts", "GPU power consumption in watts"),
            &["gpu_id", "gpu_name"],
        )?;

        let llm_tokens_per_second = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "gpm_llm_tokens_per_second",
                "LLM tokens per second",
            )
            .buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0]),
            &["model"],
        )?;

        let llm_time_to_first_token = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "gpm_llm_time_to_first_token_ms",
                "Time to first token in milliseconds",
            )
            .buckets(vec![10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0]),
            &["model"],
        )?;

        let llm_session_count = GaugeVec::new(
            Opts::new("gpm_llm_session_count", "Number of LLM sessions by model"),
            &["model"],
        )?;

        let process_count = GaugeVec::new(
            Opts::new("gpm_process_count", "Number of GPU processes by category"),
            &["category"],
        )?;

        let process_gpu_memory = GaugeVec::new(
            Opts::new(
                "gpm_process_gpu_memory_bytes",
                "GPU memory used by process category",
            ),
            &["category"],
        )?;

        registry.register(Box::new(gpu_utilization.clone()))?;
        registry.register(Box::new(gpu_memory_used.clone()))?;
        registry.register(Box::new(gpu_memory_total.clone()))?;
        registry.register(Box::new(gpu_temperature.clone()))?;
        registry.register(Box::new(gpu_power.clone()))?;
        registry.register(Box::new(llm_tokens_per_second.clone()))?;
        registry.register(Box::new(llm_time_to_first_token.clone()))?;
        registry.register(Box::new(llm_session_count.clone()))?;
        registry.register(Box::new(process_count.clone()))?;
        registry.register(Box::new(process_gpu_memory.clone()))?;

        Ok(Self {
            registry,
            gpu_utilization,
            gpu_memory_used,
            gpu_memory_total,
            gpu_temperature,
            gpu_power,
            llm_tokens_per_second,
            llm_time_to_first_token,
            llm_session_count,
            process_count,
            process_gpu_memory,
        })
    }

    pub fn update_gpu_metrics(&self, metrics: &crate::gpu::GpuMetrics) {
        let gpu_id_str = metrics.gpu_id.to_string();
        let labels = &[gpu_id_str.as_str(), metrics.name.as_str()];

        self.gpu_utilization
            .with_label_values(labels)
            .set(metrics.utilization_gpu as f64);

        self.gpu_memory_used
            .with_label_values(labels)
            .set(metrics.memory_used as f64);

        self.gpu_memory_total
            .with_label_values(labels)
            .set(metrics.memory_total as f64);

        self.gpu_temperature
            .with_label_values(labels)
            .set(metrics.temperature as f64);

        self.gpu_power
            .with_label_values(labels)
            .set(metrics.power_usage as f64);
    }

    pub fn record_llm_session(&self, session: &crate::ollama::LlmSession) {
        self.llm_tokens_per_second
            .with_label_values(&[&session.model])
            .observe(session.tokens_per_second);

        if let Some(ttft) = session.time_to_first_token_ms {
            self.llm_time_to_first_token
                .with_label_values(&[&session.model])
                .observe(ttft as f64);
        }

        self.llm_session_count
            .with_label_values(&[&session.model])
            .inc();
    }

    pub fn update_process_metrics(&self, processes: &[crate::classifier::ClassifiedProcess]) {
        use std::collections::HashMap;

        let mut category_counts: HashMap<&str, f64> = HashMap::new();
        let mut category_memory: HashMap<&str, f64> = HashMap::new();

        for proc in processes {
            let category = proc.category.as_str();
            *category_counts.entry(category).or_insert(0.0) += 1.0;
            *category_memory.entry(category).or_insert(0.0) +=
                (proc.gpu_memory_mb * 1024 * 1024) as f64;
        }

        for (category, count) in category_counts {
            self.process_count.with_label_values(&[category]).set(count);
        }

        for (category, memory) in category_memory {
            self.process_gpu_memory
                .with_label_values(&[category])
                .set(memory);
        }
    }

    pub fn render_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();

        encoder.encode(&metric_families, &mut buffer).unwrap();

        String::from_utf8(buffer).unwrap()
    }

    pub async fn serve(self: Arc<Self>, port: u16) -> Result<()> {
        let app = Router::new().route("/metrics", get(move || async move {
            self.render_metrics()
        }));

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        info!("Prometheus metrics server listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}
