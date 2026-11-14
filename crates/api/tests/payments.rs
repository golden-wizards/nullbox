//! Payments routes against a fake Stripe API on localhost (no network, no real keys).

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::{get, post};
use http_body_util::BodyExt;
use nullbox_api::http::payments::{self, StripeConfig};
use serde_json::{Value, json};
use tower::ServiceExt;

type Seen = Arc<Mutex<Vec<(String, String)>>>;

/// Starts a fake `api.stripe.com`; records `(authorization, form body)` of each create call.
async fn fake_stripe() -> (String, Seen) {
    let seen: Seen = Arc::default();
    let app = Router::new()
        .route(
            "/v1/checkout/sessions",
            post(|State(seen): State<Seen>, headers: HeaderMap, body: String| async move {
                let auth = headers.get("authorization").unwrap().to_str().unwrap().to_owned();
                seen.lock().unwrap().push((auth, body));
                axum::Json(json!({ "id": "cs_test_123", "url": "https://checkout.stripe.com/c/pay/cs_test_123" }))
            }),
        )
        .route(
            "/v1/checkout/sessions/{id}",
            get(|| async {
                axum::Json(json!({
                    "id": "cs_test_123", "status": "complete", "payment_status": "paid",
                    "amount_total": 500, "currency": "usd"
                }))
            }),
        )
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), seen)
}

fn config(api_base: &str, enabled: bool) -> StripeConfig {
    StripeConfig {
        secret_key: enabled.then(|| "sk_test_fake".to_owned()),
        webhook_secret: enabled.then(|| "whsec_fake".to_owned()),
        publishable_key: None,
        price_id: None,
        amount: 500,
        currency: "usd".into(),
        product_name: "Nullbox Pro".into(),
        api_base: api_base.into(),
        public_web_url: "https://example.test".into(),
    }
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::post(uri).header("content-type", "application/json").body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn checkout_creates_a_stripe_session() {
    let (base, seen) = fake_stripe().await;
    let app: Router = payments::router(config(&base, true));

    let (status, body) = send(&app, Request::get("/config").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], true);

    let (status, body) = send(&app, post_json("/checkout", json!({ "email": "a@b.test" }))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["id"], "cs_test_123");
    assert_eq!(body["url"], "https://checkout.stripe.com/c/pay/cs_test_123");

    let (auth, form) = seen.lock().unwrap()[0].clone();
    assert_eq!(auth, "Bearer sk_test_fake");
    assert!(form.contains("mode=payment"), "{form}");
    assert!(form.contains("unit_amount%5D=500"), "{form}");
    assert!(form.contains("customer_email=a%40b.test"), "{form}");
    assert!(form.contains("success_url=https%3A%2F%2Fexample.test%2Fpay%2Fsuccess"), "{form}");

    let (status, body) = send(&app, Request::get("/sessions/cs_test_123").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["payment_status"], "paid");

    let (status, _) = send(&app, Request::get("/sessions/..%2Fevil").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn disabled_without_keys() {
    let app: Router = payments::router(config("http://127.0.0.1:9", false));
    let (status, body) = send(&app, Request::get("/config").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);
    let (status, body) = send(&app, post_json("/checkout", json!({}))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["code"], "service_unavailable");
    let (status, _) = send(&app, post_json("/webhook", json!({}))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn webhook_checks_the_signature() {
    let app: Router = payments::router(config("http://127.0.0.1:9", true));
    let event = json!({
        "id": "evt_1", "type": "checkout.session.completed",
        "data": { "object": { "id": "cs_test_123", "payment_status": "paid", "amount_total": 500 } }
    })
    .to_string();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    let signed = Request::post("/webhook")
        .header("stripe-signature", payments::sign("whsec_fake", event.as_bytes(), now))
        .body(Body::from(event.clone()))
        .unwrap();
    let (status, body) = send(&app, signed).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["received"], true);

    let forged = Request::post("/webhook")
        .header("stripe-signature", payments::sign("whsec_wrong", event.as_bytes(), now))
        .body(Body::from(event.clone()))
        .unwrap();
    assert_eq!(send(&app, forged).await.0, StatusCode::BAD_REQUEST);

    let unsigned = Request::post("/webhook").body(Body::from(event)).unwrap();
    assert_eq!(send(&app, unsigned).await.0, StatusCode::BAD_REQUEST);
}
