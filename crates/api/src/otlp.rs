//! OpenTelemetry trace export over OTLP http/protobuf.
//!
//! Off unless `OTEL_EXPORTER_OTLP_ENDPOINT` is set (base URL; spans go to `<base>/v1/traces`).
//! `OTEL_EXPORTER_OTLP_HEADERS` (e.g. `Authorization=Bearer …`) is sent with every export and
//! `OTEL_SERVICE_NAME` names the service (default `nullbox-api`).
//!
//! Each HTTP request becomes one server span carrying the same trace id and span id that
//! [`crate::http::observability`] puts in logs and response headers. Spans are batched on a
//! background thread and flushed every 2 s; export errors are dropped, never surfaced to requests.

use std::cell::Cell;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use opentelemetry::trace::{Span as _, SpanBuilder, SpanId, SpanKind, Status, TraceId, Tracer as _};
use opentelemetry::{InstrumentationScope, KeyValue};
use opentelemetry_http::{Bytes, HttpClient, HttpError, Request, Response};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{
    BatchConfigBuilder, BatchSpanProcessor, IdGenerator, RandomIdGenerator, SdkTracer, SdkTracerProvider,
};

static PROVIDER: OnceLock<(SdkTracerProvider, SdkTracer)> = OnceLock::new();

thread_local! {
    /// Ids for the span being started on this thread, so OTLP spans reuse our request ids.
    static NEXT_IDS: Cell<Option<(TraceId, SpanId)>> = const { Cell::new(None) };
}

/// Hands out the ids staged in [`NEXT_IDS`], falling back to random ones.
#[derive(Debug, Default)]
struct StagedIds(RandomIdGenerator);

impl IdGenerator for StagedIds {
    fn new_trace_id(&self) -> TraceId {
        NEXT_IDS.with(|c| c.get()).map_or_else(|| self.0.new_trace_id(), |(t, _)| t)
    }
    fn new_span_id(&self) -> SpanId {
        NEXT_IDS.with(|c| c.get()).map_or_else(|| self.0.new_span_id(), |(_, s)| s)
    }
}

/// Blocking reqwest client (rustls + webpki roots, no OpenSSL). The batch processor runs on its
/// own thread outside tokio, so a blocking client is the right fit.
#[derive(Debug)]
struct BlockingClient(reqwest::blocking::Client);

#[async_trait::async_trait]
impl HttpClient for BlockingClient {
    async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
        let response = self.0.execute(request.try_into()?)?;
        let mut out = Response::builder().status(response.status().as_u16());
        for (name, value) in response.headers() {
            out = out.header(name.as_str(), value.as_bytes());
        }
        Ok(out.body(response.bytes()?)?)
    }
}

fn enabled() -> bool {
    std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok_and(|v| !v.trim().is_empty())
}

/// Start the exporter if configured. Never fails startup: problems are logged and export stays off.
pub fn init() {
    if !enabled() {
        return;
    }
    // The blocking client spins up its own runtime, which must not happen on a tokio worker.
    let built = std::thread::spawn(build).join().unwrap_or_else(|_| Err("exporter setup panicked".into()));
    match built {
        Ok(provider) => {
            let tracer = opentelemetry::trace::TracerProvider::tracer_with_scope(
                &provider,
                InstrumentationScope::builder("nullbox-api").with_version(env!("CARGO_PKG_VERSION")).build(),
            );
            let _ = PROVIDER.set((provider, tracer));
            tracing::info!("OTLP trace export enabled");
        }
        Err(error) => tracing::warn!(%error, "OTLP trace export disabled"),
    }
}

fn build() -> Result<SdkTracerProvider, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build()?;
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_http_client(BlockingClient(client))
        .build()?;
    let processor = BatchSpanProcessor::builder(exporter)
        .with_batch_config(BatchConfigBuilder::default().with_scheduled_delay(Duration::from_secs(2)).build())
        .build();
    let service = std::env::var("OTEL_SERVICE_NAME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "nullbox-api".to_owned());
    Ok(SdkTracerProvider::builder()
        .with_span_processor(processor)
        .with_id_generator(StagedIds::default())
        .with_resource(Resource::builder().with_service_name(service).build())
        .build())
}

/// Flush pending spans on shutdown.
pub fn shutdown() {
    if let Some((provider, _)) = PROVIDER.get() {
        let provider = provider.clone();
        let _ = std::thread::spawn(move || provider.shutdown()).join();
    }
}

/// Record one finished HTTP request as a server span. No-op when export is off.
pub fn record_request(trace_id: &str, span_id: &str, method: &str, route: &str, status: u16, elapsed: Duration) {
    let Some((_, tracer)) = PROVIDER.get() else { return };
    let ids = TraceId::from_hex(trace_id).ok().zip(SpanId::from_hex(span_id).ok());
    let end = SystemTime::now();
    let builder = SpanBuilder::from_name(format!("{method} {route}"))
        .with_kind(SpanKind::Server)
        .with_start_time(end.checked_sub(elapsed).unwrap_or(end))
        .with_attributes([
            KeyValue::new("http.request.method", method.to_owned()),
            KeyValue::new("http.route", route.to_owned()),
            KeyValue::new("http.response.status_code", i64::from(status)),
        ]);
    NEXT_IDS.with(|c| c.set(ids));
    let mut span = tracer.build_with_context(builder, &opentelemetry::Context::new());
    NEXT_IDS.with(|c| c.set(None));
    if status >= 500 {
        span.set_status(Status::error(format!("HTTP {status}")));
    }
    span.end_with_timestamp(end);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_ids_are_used_then_cleared() {
        let ids = StagedIds::default();
        let t = TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e4736").unwrap();
        let s = SpanId::from_hex("00f067aa0ba902b7").unwrap();
        NEXT_IDS.with(|c| c.set(Some((t, s))));
        assert_eq!((ids.new_trace_id(), ids.new_span_id()), (t, s));
        NEXT_IDS.with(|c| c.set(None));
        assert_ne!(ids.new_trace_id(), t);
    }

    #[test]
    fn record_is_a_noop_when_disabled() {
        record_request("4bf92f3577b34da6a3ce929d0e0e4736", "00f067aa0ba902b7", "GET", "/x", 200, Duration::ZERO);
    }
}
