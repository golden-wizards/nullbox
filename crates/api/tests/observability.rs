//! Observability middleware: trace headers and Prometheus metrics, end to end.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use http_body_util::BodyExt;
use nullbox_api::http::observability;
use tower::ServiceExt;

fn app() -> Router {
    Router::new()
        .route("/obs-test/{id}", get(|| async { "hi" }))
        .route("/metrics", get(observability::metrics))
        .layer(axum::middleware::from_fn(observability::observe))
}

#[tokio::test]
async fn propagates_trace_context_and_counts_requests() {
    let app = app();

    let incoming = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let res = app
        .clone()
        .oneshot(Request::get("/obs-test/42").header("traceparent", incoming).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["x-trace-id"], "4bf92f3577b34da6a3ce929d0e0e4736");
    let tp = res.headers()["traceparent"].to_str().unwrap();
    assert!(tp.starts_with("00-4bf92f3577b34da6a3ce929d0e0e4736-"), "{tp}");

    // Without an incoming header a fresh trace id is generated.
    let res = app.clone().oneshot(Request::get("/obs-test/7").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.headers()["x-trace-id"].len(), 32);

    let res = app.oneshot(Request::get("/metrics").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers()["content-type"].to_str().unwrap().starts_with("text/plain"));
    let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    // Route templates, not raw paths, keep cardinality bounded.
    assert!(body.contains(r#"http_requests_total{method="GET",route="/obs-test/{id}",status="200"} 2"#), "{body}");
    assert!(body.contains("# TYPE http_request_duration_seconds histogram"));
}
