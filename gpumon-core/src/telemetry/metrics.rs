use crate::classifier::ClassifiedProcess;
use crate::gpu::GpuMetrics;
use crate::ollama::LlmSession;
use opentelemetry::{metrics::*, KeyValue};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::sync::Arc;

pub struct MetricsCollector {
    _meter: Meter,
    // GPU metrics
    gpu_utilization: Gauge<f64>,
    gpu_memory_used: Gauge<u64>,
    gpu_temperature: Gauge<f64>,
    gpu_power: Gauge<f64>,

    // LLM metrics
    llm_tokens_per_second: Histogram<f64>,
    llm_time_to_first_token: Histogram<f64>,
    llm_total_tokens: Counter<u64>,

    // Process metrics
    process_gpu_duration: Counter<f64>,
    process_gpu_memory: Gauge<u64>,
    process_count: Gauge<u64>,
}

impl MetricsCollector {
    pub fn new(meter_provider: Arc<SdkMeterProvider>) -> Self {
        let meter = meter_provider.meter("gpumon");

        let gpu_utilization = meter
            .f64_gauge("gpu.utilization.percent")
            .with_description("GPU utilization percentage")
            .with_unit("%")
            .build();

        let gpu_memory_used = meter
            .u64_gauge("gpu.memory.used.bytes")
            .with_description("GPU memory used in bytes")
            .with_unit("bytes")
            .build();

        let gpu_temperature = meter
            .f64_gauge("gpu.temperature.celsius")
            .with_description("GPU temperature in Celsius")
            .with_unit("°C")
            .build();

        let gpu_power = meter
            .f64_gauge("gpu.power.watts")
            .with_description("GPU power consumption in watts")
            .with_unit("W")
            .build();

        let llm_tokens_per_second = meter
            .f64_histogram("llm.tokens_per_second")
            .with_description("LLM generation tokens per second")
            .with_unit("tokens/s")
            .build();

        let llm_time_to_first_token = meter
            .f64_histogram("llm.time_to_first_token.ms")
            .with_description("Time to first token in milliseconds")
            .with_unit("ms")
            .build();

        let llm_total_tokens = meter
            .u64_counter("llm.tokens.total")
            .with_description("Total tokens processed")
            .with_unit("tokens")
            .build();

        let process_gpu_duration = meter
            .f64_counter("process.gpu_duration.seconds")
            .with_description("GPU usage duration by process category")
            .with_unit("s")
            .build();

        let process_gpu_memory = meter
            .u64_gauge("process.gpu_memory.bytes")
            .with_description("GPU memory used by process")
            .with_unit("bytes")
            .build();

        let process_count = meter
            .u64_gauge("process.count")
            .with_description("Number of processes by category")
            .build();

        Self {
            _meter: meter,
            gpu_utilization,
            gpu_memory_used,
            gpu_temperature,
            gpu_power,
            llm_tokens_per_second,
            llm_time_to_first_token,
            llm_total_tokens,
            process_gpu_duration,
            process_gpu_memory,
            process_count,
        }
    }

    pub fn record_gpu_metrics(&self, metrics: &GpuMetrics) {
        let labels = &[
            KeyValue::new("gpu_id", metrics.gpu_id.to_string()),
            KeyValue::new("gpu_name", metrics.name.clone()),
        ];

        self.gpu_utilization.record(metrics.utilization_gpu as f64, labels);
        self.gpu_memory_used.record(metrics.memory_used, labels);
        self.gpu_temperature.record(metrics.temperature as f64, labels);
        self.gpu_power.record(metrics.power_usage as f64, labels);
    }

    pub fn record_llm_session(&self, session: &LlmSession) {
        let labels = &[KeyValue::new("model", session.model.clone())];

        self.llm_tokens_per_second.record(session.tokens_per_second, labels);

        if let Some(ttft) = session.time_to_first_token_ms {
            self.llm_time_to_first_token.record(ttft as f64, labels);
        }

        self.llm_total_tokens.add(session.total_tokens, labels);
    }

    pub fn record_process_metrics(&self, processes: &[ClassifiedProcess]) {
        use std::collections::HashMap;

        let mut category_counts: HashMap<String, u64> = HashMap::new();
        let mut category_memory: HashMap<String, u64> = HashMap::new();

        for proc in processes {
            let cat_str = proc.category.as_str().to_string();
            *category_counts.entry(cat_str.clone()).or_insert(0) += 1;
            *category_memory.entry(cat_str.clone()).or_insert(0) += proc.gpu_memory_mb;

            let labels = vec![
                KeyValue::new("category", proc.category.as_str().to_string()),
                KeyValue::new("process_name", proc.name.clone()),
                KeyValue::new("pid", proc.pid.to_string()),
            ];

            self.process_gpu_memory.record(
                proc.gpu_memory_mb * 1024 * 1024,
                &labels,
            );
        }

        for (category, count) in category_counts {
            let labels = vec![KeyValue::new("category", category)];
            self.process_count.record(count, &labels);
        }

        for (category, memory_mb) in category_memory {
            let labels = vec![KeyValue::new("category", category)];
            self.process_gpu_memory.record(memory_mb * 1024 * 1024, &labels);
        }
    }
}
