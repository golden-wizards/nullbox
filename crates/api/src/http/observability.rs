//! Observability: Prometheus metrics, W3C trace context and one structured log
//! line per request.
//!
//! - `GET /metrics` and `GET /api/metrics` serve Prometheus text format.
//! - Every request gets a trace id (taken from an incoming `traceparent` header or
//!   freshly generated) and a span id. Both are recorded on the request's tracing
//!   span, so every log line emitted while handling it carries them, and they are
//!   returned to the client as `traceparent`, `x-trace-id` and `x-request-id`.
//! - Logs are JSON when `LOG_FORMAT=json` (see `main.rs`).
//!
//! No external collector is needed: Prometheus, Grafana Alloy or an OTel
//! collector can scrape `/metrics`, and log shippers can read the JSON lines.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tracing::Instrument;

/// Histogram buckets for request latency, in seconds.
const BUCKETS: [f64; 11] = [0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5];

#[derive(Default)]
struct Histogram {
    buckets: [u64; BUCKETS.len()],
    count: u64,
    sum: f64,
}

impl Histogram {
    fn observe(&mut self, seconds: f64) {
        for (bucket, le) in self.buckets.iter_mut().zip(BUCKETS) {
            if seconds <= le {
                *bucket += 1;
            }
        }
        self.count += 1;
        self.sum += seconds;
    }
}

#[derive(Default)]
struct Series {
    /// (method, route, status) -> count
    requests: BTreeMap<(String, String, u16), u64>,
    /// (method, route) -> latency histogram
    durations: BTreeMap<(String, String), Histogram>,
}

struct Metrics {
    start: SystemTime,
    started: Instant,
    in_flight: AtomicI64,
    series: Mutex<Series>,
}

static METRICS: LazyLock<Metrics> = LazyLock::new(|| Metrics {
    start: SystemTime::now(),
    started: Instant::now(),
    in_flight: AtomicI64::new(0),
    series: Mutex::new(Series::default()),
});

/// Touch the registry so the process start time is the server start, not the first request.
pub fn init() {
    LazyLock::force(&METRICS);
}

fn record(method: &str, route: &str, status: u16, seconds: f64) {
    let mut series = METRICS.series.lock().unwrap_or_else(|e| e.into_inner());
    *series.requests.entry((method.to_owned(), route.to_owned(), status)).or_default() += 1;
    series.durations.entry((method.to_owned(), route.to_owned())).or_default().observe(seconds);
}

/// Escape a Prometheus label value.
fn label(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// Render all metrics in Prometheus text exposition format 0.0.4.
pub fn render() -> String {
    let mut out = String::new();
    let version = env!("CARGO_PKG_VERSION");
    let _ = writeln!(out, "# HELP nullbox_build_info Build information.");
    let _ = writeln!(out, "# TYPE nullbox_build_info gauge");
    let _ = writeln!(out, "nullbox_build_info{{version=\"{version}\"}} 1");

    let start = METRICS.start.duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let _ = writeln!(out, "# HELP process_start_time_seconds Start time of the process since unix epoch in seconds.");
    let _ = writeln!(out, "# TYPE process_start_time_seconds gauge");
    let _ = writeln!(out, "process_start_time_seconds {start}");
    let _ = writeln!(out, "# HELP nullbox_uptime_seconds Seconds since the API started.");
    let _ = writeln!(out, "# TYPE nullbox_uptime_seconds gauge");
    let _ = writeln!(out, "nullbox_uptime_seconds {}", METRICS.started.elapsed().as_secs_f64());

    let _ = writeln!(out, "# HELP http_requests_in_flight HTTP requests currently being served.");
    let _ = writeln!(out, "# TYPE http_requests_in_flight gauge");
    let _ = writeln!(out, "http_requests_in_flight {}", METRICS.in_flight.load(Ordering::Relaxed));

    let series = METRICS.series.lock().unwrap_or_else(|e| e.into_inner());
    let _ = writeln!(out, "# HELP http_requests_total Total HTTP requests by method, route and status.");
    let _ = writeln!(out, "# TYPE http_requests_total counter");
    for ((method, route, status), count) in &series.requests {
        let _ = writeln!(
            out,
            "http_requests_total{{method=\"{}\",route=\"{}\",status=\"{status}\"}} {count}",
            label(method),
            label(route)
        );
    }
    let _ = writeln!(out, "# HELP http_request_duration_seconds HTTP request latency by method and route.");
    let _ = writeln!(out, "# TYPE http_request_duration_seconds histogram");
    for ((method, route), h) in &series.durations {
        let (method, route) = (label(method), label(route));
        for (le, count) in BUCKETS.iter().zip(h.buckets) {
            let _ = writeln!(
                out,
                "http_request_duration_seconds_bucket{{method=\"{method}\",route=\"{route}\",le=\"{le}\"}} {count}"
            );
        }
        let _ = writeln!(
            out,
            "http_request_duration_seconds_bucket{{method=\"{method}\",route=\"{route}\",le=\"+Inf\"}} {}",
            h.count
        );
        let _ = writeln!(out, "http_request_duration_seconds_sum{{method=\"{method}\",route=\"{route}\"}} {}", h.sum);
        let _ =
            writeln!(out, "http_request_duration_seconds_count{{method=\"{method}\",route=\"{route}\"}} {}", h.count);
    }
    out
}

/// `GET /metrics`: Prometheus scrape endpoint.
pub async fn metrics() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], render())
}

/// Parse a W3C `traceparent` header, returning its trace id.
#[cfg(test)]
fn parse_traceparent(value: &str) -> Option<String> {
    parse_traceparent_ids(value).map(|(trace_id, _)| trace_id)
}

/// Parse a W3C `traceparent` header, returning its trace id and parent span id.
fn parse_traceparent_ids(value: &str) -> Option<(String, String)> {
    let mut parts = value.trim().split('-');
    let (version, trace_id, parent_id, flags) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let hex =
        |s: &str, len: usize| s.len() == len && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let valid = hex(version, 2)
        && version != "ff"
        && hex(trace_id, 32)
        && trace_id.bytes().any(|b| b != b'0')
        && hex(parent_id, 16)
        && hex(flags, 2);
    valid.then(|| (trace_id.to_owned(), parent_id.to_owned()))
}

fn new_trace_id() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

fn new_span_id() -> String {
    let id = uuid::Uuid::now_v7().as_u128() as u64; // the random tail of a v7 uuid
    format!("{:016x}", id.max(1))
}

/// Middleware: trace context, request metrics and a structured access log line.
pub async fn observe(req: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = req.method().as_str().to_owned();
    let route = req.extensions().get::<MatchedPath>().map_or_else(|| "unmatched".to_owned(), |p| p.as_str().to_owned());
    let path = req.uri().path().to_owned();
    let (trace_id, parent_span_id) =
        match req.headers().get("traceparent").and_then(|v| v.to_str().ok()).and_then(parse_traceparent_ids) {
            Some((trace_id, parent)) => (trace_id, Some(parent)),
            None => (new_trace_id(), None),
        };
    let span_id = new_span_id();

    let span = tracing::info_span!(
        "request",
        trace_id = %trace_id,
        span_id = %span_id,
        method = %method,
        route = %route,
    );

    METRICS.in_flight.fetch_add(1, Ordering::Relaxed);
    let mut response = next.run(req).instrument(span.clone()).await;
    METRICS.in_flight.fetch_sub(1, Ordering::Relaxed);

    let elapsed = started.elapsed();
    let status = response.status().as_u16();
    record(&method, &route, status, elapsed.as_secs_f64());
    crate::otlp::record_request(&crate::otlp::RequestSpan {
        trace_id: &trace_id,
        span_id: &span_id,
        parent_span_id: parent_span_id.as_deref(),
        method: &method,
        route: &route,
        status,
        elapsed,
        attributes: response.extensions().get::<crate::otlp::SpanAttributes>(),
    });
    span.in_scope(|| {
        tracing::info!(
            target: "nullbox::access",
            status,
            path = %path,
            latency_ms = elapsed.as_secs_f64() * 1000.0,
            "request completed"
        );
    });

    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&format!("00-{trace_id}-{span_id}-01")) {
        headers.insert("traceparent", value);
    }
    if let Ok(value) = HeaderValue::from_str(&trace_id) {
        headers.insert("x-trace-id", value.clone());
        headers.insert("x-request-id", value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_traceparent() {
        let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        assert_eq!(parse_traceparent(tp).as_deref(), Some("4bf92f3577b34da6a3ce929d0e0e4736"));
    }

    #[test]
    fn parses_traceparent_parent_span_id() {
        let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
        let ids = parse_traceparent_ids(tp).unwrap();
        assert_eq!(ids, ("4bf92f3577b34da6a3ce929d0e0e4736".to_owned(), "00f067aa0ba902b7".to_owned()));
    }

    #[test]
    fn rejects_invalid_traceparent() {
        assert_eq!(parse_traceparent("garbage"), None);
        assert_eq!(parse_traceparent("00-00000000000000000000000000000000-00f067aa0ba902b7-01"), None);
        assert_eq!(parse_traceparent("00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01"), None);
    }

    #[test]
    fn generated_ids_have_w3c_shape() {
        assert_eq!(new_trace_id().len(), 32);
        assert_eq!(new_span_id().len(), 16);
    }

    #[test]
    fn renders_recorded_series() {
        record("GET", "/test/render", 200, 0.003);
        let text = render();
        assert!(text.contains("http_requests_total{method=\"GET\",route=\"/test/render\",status=\"200\"} 1"));
        assert!(
            text.contains("http_request_duration_seconds_bucket{method=\"GET\",route=\"/test/render\",le=\"+Inf\"} 1")
        );
        assert!(text.contains("# TYPE http_request_duration_seconds histogram"));
    }
}
