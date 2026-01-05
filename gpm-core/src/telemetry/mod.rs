pub mod metrics;
pub mod prometheus;
pub mod distributed_tracing;

use crate::config::GpmConfig;
use crate::error::{GpmError, Result};
use opentelemetry::global;
use opentelemetry_otlp::{WithExportConfig, MetricExporter, SpanExporter};
use opentelemetry_sdk::{
    metrics::{PeriodicReader, SdkMeterProvider},
    runtime,
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions as semconv;
use std::sync::Arc;
use std::time::Duration;

pub use self::metrics::MetricsCollector;
pub use self::prometheus::PrometheusExporter;
pub use self::distributed_tracing::TracingCollector;

pub struct TelemetryManager {
    pub metrics: Option<Arc<MetricsCollector>>,
    pub tracing: Option<Arc<TracingCollector>>,
    pub prometheus: Option<Arc<PrometheusExporter>>,
    meter_provider: Option<Arc<SdkMeterProvider>>,
    tracer_provider: Option<Arc<TracerProvider>>,
}

impl TelemetryManager {
    pub fn new(config: &GpmConfig) -> Result<Self> {
        let mut meter_provider = None;
        let mut tracer_provider = None;
        let mut metrics_collector = None;
        let mut tracing_collector = None;
        let mut prometheus_exporter = None;

        if config.telemetry.enable_opentelemetry {
            tracing::info!("Initializing OpenTelemetry");

            let resource = Resource::new(vec![
                opentelemetry::KeyValue::new(semconv::resource::SERVICE_NAME, "gpm"),
                opentelemetry::KeyValue::new(semconv::resource::SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
                opentelemetry::KeyValue::new(
                    "host.name",
                    hostname::get()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                ),
            ]);

            let mp = init_meter_provider(&config.telemetry.otlp_endpoint, resource.clone())?;
            let tp = init_tracer_provider(&config.telemetry.otlp_endpoint, resource)?;

            let mc = MetricsCollector::new(Arc::clone(&mp));
            let tc = TracingCollector::new(Arc::clone(&tp));

            meter_provider = Some(mp);
            tracer_provider = Some(tp);
            metrics_collector = Some(Arc::new(mc));
            tracing_collector = Some(Arc::new(tc));

            tracing::info!("OpenTelemetry initialized successfully");
        }

        if config.telemetry.enable_prometheus {
            tracing::info!("Initializing Prometheus exporter");
            let prom = PrometheusExporter::new()?;
            prometheus_exporter = Some(Arc::new(prom));
            tracing::info!("Prometheus exporter initialized");
        }

        Ok(Self {
            metrics: metrics_collector,
            tracing: tracing_collector,
            prometheus: prometheus_exporter,
            meter_provider,
            tracer_provider,
        })
    }

    pub async fn start_prometheus_server(&self, port: u16) -> Result<()> {
        if let Some(prometheus) = &self.prometheus {
            let prometheus_clone = Arc::clone(prometheus);
            tokio::spawn(async move {
                if let Err(e) = prometheus_clone.serve(port).await {
                    tracing::error!("Prometheus server error: {}", e);
                }
            });
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        if let Some(mp) = &self.meter_provider {
            if let Err(e) = mp.shutdown() {
                tracing::error!("Failed to shutdown meter provider: {:?}", e);
            }
        }

        if let Some(tp) = &self.tracer_provider {
            if let Err(e) = tp.shutdown() {
                tracing::error!("Failed to shutdown tracer provider: {:?}", e);
            }
        }

        global::shutdown_tracer_provider();
    }
}

fn init_meter_provider(
    otlp_endpoint: &str,
    resource: Resource,
) -> Result<Arc<SdkMeterProvider>> {
    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| GpmError::ServiceUnavailable(format!("OTLP metrics exporter: {}", e)))?;

    let reader = PeriodicReader::builder(exporter, runtime::Tokio)
        .with_interval(Duration::from_secs(10))
        .build();

    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build();

    Ok(Arc::new(provider))
}

fn init_tracer_provider(otlp_endpoint: &str, resource: Resource) -> Result<Arc<TracerProvider>> {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| GpmError::ServiceUnavailable(format!("OTLP trace exporter: {}", e)))?;

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_resource(resource)
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .build();

    global::set_tracer_provider(provider.clone());

    Ok(Arc::new(provider))
}
