use anyhow::Result;
use opentelemetry::global;
use opentelemetry::trace::TraceId;
use opentelemetry::KeyValue;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{Sampler, SdkTracerProvider},
    Resource,
};

pub enum Telemetry {
    Enabled(SdkTracerProvider),
    Disabled,
}
///  Fetch an opentelemetry::trace::TraceId as hex through the full tracing stack
pub fn get_trace_id() -> TraceId {
    use opentelemetry::trace::TraceContextExt as _; // opentelemetry::Context -> opentelemetry::trace::Span
    use tracing_opentelemetry::OpenTelemetrySpanExt as _; // tracing::Span to opentelemetry::Context
    tracing::Span::current().context().span().span_context().trace_id()
}

pub fn init_tracing() -> Result<Telemetry> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT").unwrap_or_default();

    if endpoint.is_empty() {
        return Ok(Telemetry::Disabled);
    }

    let exporter = SpanExporter::builder().with_http().with_endpoint(endpoint).build()?;

    let resource = Resource::builder()
        .with_attributes(vec![
            KeyValue::new("service.name", "nsp-k8s-operator"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ])
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .with_sampler(Sampler::AlwaysOn)
        .build();

    global::set_tracer_provider(provider.clone());
    global::set_text_map_propagator(TraceContextPropagator::new());

    Ok(Telemetry::Enabled(provider))
}

pub fn shutdown_tracing(t: Telemetry) {
    match t {
        Telemetry::Enabled(p) => {
            let _ = p.shutdown();
        }
        Telemetry::Disabled => {}
    }
}
