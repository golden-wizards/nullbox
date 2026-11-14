//! OpenTelemetry trace and log export over OTLP http/protobuf.
//!
//! Off unless `OTEL_EXPORTER_OTLP_ENDPOINT` is set (base URL; spans go to `<base>/v1/traces`,
//! logs to `<base>/v1/logs`). `OTEL_EXPORTER_OTLP_HEADERS` (e.g. `Authorization=Bearer …`) is
//! sent with every export and `OTEL_SERVICE_NAME` names the service (default `nullbox-api`).
//!
//! - **Traces:** each HTTP request becomes one server span carrying the same trace id and span id
//!   that [`crate::http::observability`] puts in logs and response headers. When the caller sent a
//!   W3C `traceparent`, the span joins the caller's trace as a child of its span. Handlers can add
//!   attributes by returning a [`SpanAttributes`] response extension (ticket creation records the
//!   ticket id, number and subject this way).
//! - **Logs:** [`LogBridge`] is a `tracing` layer that turns every INFO-and-above event into an
//!   OTLP log record, with the trace id and span id of the request it happened in.
//!
//! Both are batched on background threads and flushed every 2 s; export errors are dropped, never
//! surfaced to requests.

use std::cell::Cell;
use std::fmt::Write as _;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use opentelemetry::logs::{AnyValue, LogRecord as _, Logger as _, LoggerProvider as _, Severity};
use opentelemetry::trace::{
    Span as _, SpanBuilder, SpanContext, SpanId, SpanKind, Status, TraceContextExt as _, TraceFlags, TraceId,
    TraceState, Tracer as _,
};
use opentelemetry::{InstrumentationScope, KeyValue};
use opentelemetry_http::{Bytes, HttpClient, HttpError, Request, Response};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::{BatchLogProcessor, SdkLogger, SdkLoggerProvider};
use opentelemetry_sdk::trace::{
    BatchConfigBuilder, BatchSpanProcessor, IdGenerator, RandomIdGenerator, SdkTracer, SdkTracerProvider,
};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

static TRACES: OnceLock<(SdkTracerProvider, SdkTracer)> = OnceLock::new();
static LOGS: OnceLock<(SdkLoggerProvider, SdkLogger)> = OnceLock::new();

const SCOPE: &str = "nullbox-api";
const FLUSH_EVERY: Duration = Duration::from_secs(2);

/// Extra attributes for the exported request span, set by a handler as a response extension.
#[derive(Clone, Debug, Default)]
pub struct SpanAttributes(pub Vec<(&'static str, String)>);

/// Everything the request middleware knows about a finished request.
pub struct RequestSpan<'a> {
    pub trace_id: &'a str,
    pub span_id: &'a str,
    /// Span id from the caller's `traceparent`, if any.
    pub parent_span_id: Option<&'a str>,
    pub method: &'a str,
    pub route: &'a str,
    pub status: u16,
    pub elapsed: Duration,
    pub attributes: Option<&'a SpanAttributes>,
}

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

/// Blocking reqwest client (rustls + webpki roots, no OpenSSL). The batch processors run on their
/// own threads outside tokio, so a blocking client is the right fit.
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

fn scope() -> InstrumentationScope {
    InstrumentationScope::builder(SCOPE).with_version(env!("CARGO_PKG_VERSION")).build()
}

/// Start the exporters if configured. Never fails startup: problems are logged and export stays off.
pub fn init() {
    if !enabled() {
        return;
    }
    // The blocking client spins up its own runtime, which must not happen on a tokio worker.
    let built = std::thread::spawn(build).join().unwrap_or_else(|_| Err("exporter setup panicked".into()));
    match built {
        Ok((traces, logs)) => {
            let tracer = opentelemetry::trace::TracerProvider::tracer_with_scope(&traces, scope());
            let logger = logs.logger_with_scope(scope());
            let _ = TRACES.set((traces, tracer));
            let _ = LOGS.set((logs, logger));
            tracing::info!("OTLP trace and log export enabled");
        }
        Err(error) => tracing::warn!(%error, "OTLP export disabled"),
    }
}

type BuildError = Box<dyn std::error::Error + Send + Sync>;

fn build() -> Result<(SdkTracerProvider, SdkLoggerProvider), BuildError> {
    let client = || reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build();
    let service = std::env::var("OTEL_SERVICE_NAME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "nullbox-api".to_owned());
    let resource = Resource::builder().with_service_name(service).build();

    let span_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_http_client(BlockingClient(client()?))
        .build()?;
    let span_processor = BatchSpanProcessor::builder(span_exporter)
        .with_batch_config(BatchConfigBuilder::default().with_scheduled_delay(FLUSH_EVERY).build())
        .build();
    let traces = SdkTracerProvider::builder()
        .with_span_processor(span_processor)
        .with_id_generator(StagedIds::default())
        .with_resource(resource.clone())
        .build();

    let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_http_client(BlockingClient(client()?))
        .build()?;
    let log_processor = BatchLogProcessor::builder(log_exporter)
        .with_batch_config(
            opentelemetry_sdk::logs::BatchConfigBuilder::default().with_scheduled_delay(FLUSH_EVERY).build(),
        )
        .build();
    let logs = SdkLoggerProvider::builder().with_log_processor(log_processor).with_resource(resource).build();

    Ok((traces, logs))
}

/// Flush pending spans and logs on shutdown.
pub fn shutdown() {
    let traces = TRACES.get().map(|(p, _)| p.clone());
    let logs = LOGS.get().map(|(p, _)| p.clone());
    let _ = std::thread::spawn(move || {
        if let Some(p) = traces {
            let _ = p.shutdown();
        }
        if let Some(p) = logs {
            let _ = p.shutdown();
        }
    })
    .join();
}

/// Record one finished HTTP request as a server span. No-op when export is off.
pub fn record_request(req: &RequestSpan<'_>) {
    let Some((_, tracer)) = TRACES.get() else { return };
    let trace_id = TraceId::from_hex(req.trace_id).ok();
    let span_id = SpanId::from_hex(req.span_id).ok();
    let end = SystemTime::now();

    let mut attributes = vec![
        KeyValue::new("http.request.method", req.method.to_owned()),
        KeyValue::new("http.route", req.route.to_owned()),
        KeyValue::new("http.response.status_code", i64::from(req.status)),
    ];
    if let Some(extra) = req.attributes {
        attributes.extend(extra.0.iter().map(|(k, v)| KeyValue::new(*k, v.clone())));
    }
    let builder = SpanBuilder::from_name(format!("{} {}", req.method, req.route))
        .with_kind(SpanKind::Server)
        .with_start_time(end.checked_sub(req.elapsed).unwrap_or(end))
        .with_attributes(attributes);

    // Join the caller's trace. Always sampled: we export every request, whatever the caller's flag.
    let parent = match (trace_id, req.parent_span_id.and_then(|p| SpanId::from_hex(p).ok())) {
        (Some(t), Some(p)) => opentelemetry::Context::new().with_remote_span_context(SpanContext::new(
            t,
            p,
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        )),
        _ => opentelemetry::Context::new(),
    };
    NEXT_IDS.with(|c| c.set(trace_id.zip(span_id)));
    let mut span = tracer.build_with_context(builder, &parent);
    NEXT_IDS.with(|c| c.set(None));
    if req.status >= 500 {
        span.set_status(Status::error(format!("HTTP {}", req.status)));
    }
    span.end_with_timestamp(end);
}

/// Trace id and span id of a `request` span, stored in its extensions by [`LogBridge`].
struct RequestIds(TraceId, SpanId);

/// Picks `trace_id` and `span_id` out of a span's fields.
#[derive(Default)]
struct IdVisitor {
    trace_id: Option<TraceId>,
    span_id: Option<SpanId>,
}

impl Visit for IdVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "trace_id" => self.trace_id = TraceId::from_hex(&format!("{value:?}")).ok(),
            "span_id" => self.span_id = SpanId::from_hex(&format!("{value:?}")).ok(),
            _ => {}
        }
    }
}

/// Turns an event's fields into a log body (`message`) and attributes (everything else).
#[derive(Default)]
struct RecordVisitor {
    message: String,
    attributes: Vec<(&'static str, AnyValue)>,
}

impl Visit for RecordVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, AnyValue::from(value.to_owned()));
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, AnyValue::from(value));
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, i64::try_from(value).map_or_else(|_| AnyValue::from(value.to_string()), AnyValue::from));
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.put(field, AnyValue::from(value));
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, AnyValue::from(value));
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            self.put(field, AnyValue::from(format!("{value:?}")));
        }
    }
}

impl RecordVisitor {
    fn put(&mut self, field: &Field, value: AnyValue) {
        if field.name() == "message" {
            if let AnyValue::String(s) = &value {
                self.message.push_str(s.as_str());
            }
        } else {
            self.attributes.push((field.name(), value));
        }
    }
}

/// Events from the exporter's own HTTP stack would loop back into the exporter.
fn is_exporter_noise(target: &str) -> bool {
    ["opentelemetry", "reqwest", "hyper", "h2", "rustls", "tokio_util"].iter().any(|p| target.starts_with(p))
}

fn severity(level: Level) -> (Severity, &'static str) {
    match level {
        Level::ERROR => (Severity::Error, "ERROR"),
        Level::WARN => (Severity::Warn, "WARN"),
        Level::INFO => (Severity::Info, "INFO"),
        Level::DEBUG => (Severity::Debug, "DEBUG"),
        Level::TRACE => (Severity::Trace, "TRACE"),
    }
}

/// `tracing` layer that exports INFO-and-above events as OTLP log records. Always safe to
/// install: it does nothing until [`init`] has enabled export.
#[derive(Clone, Copy, Debug, Default)]
pub struct LogBridge;

impl<S> Layer<S> for LogBridge
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut ids = IdVisitor::default();
        attrs.record(&mut ids);
        if let (Some(t), Some(s), Some(span)) = (ids.trace_id, ids.span_id, ctx.span(id)) {
            span.extensions_mut().insert(RequestIds(t, s));
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let Some((_, logger)) = LOGS.get() else { return };
        let meta = event.metadata();
        if matches!(*meta.level(), Level::DEBUG | Level::TRACE) || is_exporter_noise(meta.target()) {
            return;
        }
        let mut fields = RecordVisitor::default();
        event.record(&mut fields);

        let (number, text) = severity(*meta.level());
        let now = SystemTime::now();
        let mut record = logger.create_log_record();
        record.set_timestamp(now);
        record.set_observed_timestamp(now);
        record.set_severity_number(number);
        record.set_severity_text(text);
        record.set_target(meta.target().to_owned());
        record.set_body(AnyValue::from(fields.message));
        record.add_attributes(fields.attributes);
        let ids = ctx.event_scope(event).and_then(|scope| {
            scope.into_iter().find_map(|span| span.extensions().get::<RequestIds>().map(|r| (r.0, r.1)))
        });
        if let Some((trace_id, span_id)) = ids {
            record.set_trace_context(trace_id, span_id, Some(TraceFlags::SAMPLED));
        }
        logger.emit(record);
    }
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
        record_request(&RequestSpan {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736",
            span_id: "00f067aa0ba902b7",
            parent_span_id: None,
            method: "GET",
            route: "/x",
            status: 200,
            elapsed: Duration::ZERO,
            attributes: None,
        });
    }

    #[test]
    fn exporter_noise_is_filtered() {
        assert!(is_exporter_noise("hyper_util::client"));
        assert!(is_exporter_noise("opentelemetry_sdk"));
        assert!(!is_exporter_noise("nullbox::tickets"));
    }
}
