//! Payments through Stripe Checkout (test mode).
//!
//! - `GET  /api/payments/config`: is checkout enabled, what does it cost.
//! - `POST /api/payments/checkout`: creates a Checkout Session, returns its hosted `url`.
//! - `GET  /api/payments/sessions/{id}`: the session's status, for the success page.
//! - `POST /api/payments/webhook`: Stripe events, verified with `Stripe-Signature`.
//!
//! Keys come from the environment only (see [`StripeConfig::from_env`]). Without
//! `STRIPE_SECRET_KEY` the feature is off: these routes answer 503, everything else works.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use hmac::{KeyInit, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::error::{ErrorBody, ErrorCode};

/// How old a webhook signature may be, like Stripe's own libraries.
const SIGNATURE_TOLERANCE_SECS: i64 = 300;

/// Stripe settings, all from the environment. Secrets never have defaults.
#[derive(Clone)]
pub struct StripeConfig {
    /// `STRIPE_SECRET_KEY` (`sk_test_…` or a restricted `rk_test_…`). Unset = payments disabled.
    pub secret_key: Option<String>,
    /// `STRIPE_WEBHOOK_SECRET` (`whsec_…`). Unset = webhook endpoint disabled.
    pub webhook_secret: Option<String>,
    /// `STRIPE_PUBLISHABLE_KEY` (`pk_test_…`), optional, handed to the web app as is.
    pub publishable_key: Option<String>,
    /// `STRIPE_PRICE_ID`: an existing Price. Unset = an inline price from the values below.
    pub price_id: Option<String>,
    /// `STRIPE_AMOUNT` in the currency's smallest unit (default 500).
    pub amount: i64,
    /// `STRIPE_CURRENCY` (default `usd`).
    pub currency: String,
    /// `STRIPE_PRODUCT_NAME` (default `Nullbox Pro`).
    pub product_name: String,
    /// `STRIPE_API_BASE` (default `https://api.stripe.com`); tests point it at a fake.
    pub api_base: String,
    /// Where the web app lives (`PUBLIC_WEB_URL`), for the success and cancel URLs.
    pub public_web_url: String,
}

impl std::fmt::Debug for StripeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print key values.
        f.debug_struct("StripeConfig")
            .field("secret_key", &self.secret_key.as_ref().map(|_| "<set>"))
            .field("webhook_secret", &self.webhook_secret.as_ref().map(|_| "<set>"))
            .field("price_id", &self.price_id)
            .field("amount", &self.amount)
            .field("currency", &self.currency)
            .field("api_base", &self.api_base)
            .finish_non_exhaustive()
    }
}

impl StripeConfig {
    pub fn from_env(public_web_url: &str) -> Self {
        let opt = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        Self {
            secret_key: opt("STRIPE_SECRET_KEY"),
            webhook_secret: opt("STRIPE_WEBHOOK_SECRET"),
            publishable_key: opt("STRIPE_PUBLISHABLE_KEY"),
            price_id: opt("STRIPE_PRICE_ID"),
            amount: opt("STRIPE_AMOUNT").and_then(|v| v.parse().ok()).filter(|a| *a > 0).unwrap_or(500),
            currency: opt("STRIPE_CURRENCY").unwrap_or_else(|| "usd".into()).to_lowercase(),
            product_name: opt("STRIPE_PRODUCT_NAME").unwrap_or_else(|| "Nullbox Pro".into()),
            api_base: opt("STRIPE_API_BASE")
                .unwrap_or_else(|| "https://api.stripe.com".into())
                .trim_end_matches('/')
                .to_owned(),
            public_web_url: public_web_url.trim_end_matches('/').to_owned(),
        }
    }
}

struct Payments {
    config: StripeConfig,
    http: reqwest::Client,
}

/// The `/payments` routes, with their own state so they plug into any router.
pub fn router<S: Clone + Send + Sync + 'static>(config: StripeConfig) -> Router<S> {
    tracing::info!(
        enabled = config.secret_key.is_some(),
        webhook = config.webhook_secret.is_some(),
        "payments (Stripe Checkout)"
    );
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let state = Arc::new(Payments { config, http });
    Router::new()
        .route("/config", get(config_handler))
        .route("/checkout", post(create_checkout))
        .route("/sessions/{id}", get(get_session))
        .route("/webhook", post(webhook))
        .with_state(state)
}

#[derive(Debug)]
enum PaymentError {
    Disabled,
    BadRequest(String),
    NotFound,
    Upstream(String),
}

impl IntoResponse for PaymentError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Disabled => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::ServiceUnavailable,
                "payments are not configured".to_owned(),
            ),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, ErrorCode::ValidationFailed, m),
            Self::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound, "not found".to_owned()),
            Self::Upstream(detail) => {
                tracing::error!(error = %detail, "stripe request failed");
                (StatusCode::BAD_GATEWAY, ErrorCode::ServiceUnavailable, "payment provider error".to_owned())
            }
        };
        (status, Json(ErrorBody { code, message })).into_response()
    }
}

#[derive(Serialize)]
struct PaymentsConfig {
    enabled: bool,
    publishable_key: Option<String>,
    amount: i64,
    currency: String,
    product_name: String,
}

async fn config_handler(State(p): State<Arc<Payments>>) -> Json<PaymentsConfig> {
    let c = &p.config;
    Json(PaymentsConfig {
        enabled: c.secret_key.is_some(),
        publishable_key: c.publishable_key.clone(),
        amount: c.amount,
        currency: c.currency.clone(),
        product_name: c.product_name.clone(),
    })
}

#[derive(Debug, Default, Deserialize)]
struct CheckoutRequest {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Serialize)]
struct CheckoutResponse {
    id: String,
    url: String,
}

async fn create_checkout(
    State(p): State<Arc<Payments>>,
    body: Option<Json<CheckoutRequest>>,
) -> Result<Json<CheckoutResponse>, PaymentError> {
    let c = &p.config;
    let key = c.secret_key.as_deref().ok_or(PaymentError::Disabled)?;
    let request = body.map(|Json(b)| b).unwrap_or_default();

    let mut form: Vec<(String, String)> = vec![
        ("mode".into(), "payment".into()),
        ("success_url".into(), format!("{}/pay/success?session_id={{CHECKOUT_SESSION_ID}}", c.public_web_url)),
        ("cancel_url".into(), format!("{}/pay?canceled=1", c.public_web_url)),
        ("line_items[0][quantity]".into(), "1".into()),
        ("metadata[app]".into(), "nullbox".into()),
    ];
    match &c.price_id {
        Some(price) => form.push(("line_items[0][price]".into(), price.clone())),
        None => form.extend([
            ("line_items[0][price_data][currency]".into(), c.currency.clone()),
            ("line_items[0][price_data][unit_amount]".into(), c.amount.to_string()),
            ("line_items[0][price_data][product_data][name]".into(), c.product_name.clone()),
        ]),
    }
    if let Some(email) = request.email.map(|e| e.trim().to_owned()).filter(|e| e.contains('@') && e.len() <= 254) {
        form.push(("customer_email".into(), email));
    }

    let res = p
        .http
        .post(format!("{}/v1/checkout/sessions", c.api_base))
        .bearer_auth(key)
        .form(&form)
        .send()
        .await
        .map_err(|e| PaymentError::Upstream(e.to_string()))?;
    let status = res.status();
    let body: Value = res.json().await.map_err(|e| PaymentError::Upstream(e.to_string()))?;
    if !status.is_success() {
        return Err(PaymentError::Upstream(format!("{status}: {}", body["error"]["message"])));
    }
    match (body["id"].as_str(), body["url"].as_str()) {
        (Some(id), Some(url)) => {
            tracing::info!(session = %id, "checkout session created");
            Ok(Json(CheckoutResponse { id: id.to_owned(), url: url.to_owned() }))
        }
        _ => Err(PaymentError::Upstream("checkout session without id/url".into())),
    }
}

#[derive(Serialize)]
struct SessionStatus {
    id: String,
    status: Option<String>,
    payment_status: Option<String>,
    amount_total: Option<i64>,
    currency: Option<String>,
}

async fn get_session(
    State(p): State<Arc<Payments>>,
    Path(id): Path<String>,
) -> Result<Json<SessionStatus>, PaymentError> {
    let key = p.config.secret_key.as_deref().ok_or(PaymentError::Disabled)?;
    let valid = id.starts_with("cs_") && id.len() <= 255 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !valid {
        return Err(PaymentError::NotFound);
    }
    let res = p
        .http
        .get(format!("{}/v1/checkout/sessions/{id}", p.config.api_base))
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| PaymentError::Upstream(e.to_string()))?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(PaymentError::NotFound);
    }
    let status = res.status();
    let body: Value = res.json().await.map_err(|e| PaymentError::Upstream(e.to_string()))?;
    if !status.is_success() {
        return Err(PaymentError::Upstream(format!("{status}: {}", body["error"]["message"])));
    }
    let s = |k: &str| body[k].as_str().map(str::to_owned);
    Ok(Json(SessionStatus {
        id,
        status: s("status"),
        payment_status: s("payment_status"),
        amount_total: body["amount_total"].as_i64(),
        currency: s("currency"),
    }))
}

async fn webhook(State(p): State<Arc<Payments>>, headers: HeaderMap, body: Bytes) -> Result<Json<Value>, PaymentError> {
    let secret = p.config.webhook_secret.as_deref().ok_or(PaymentError::Disabled)?;
    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| PaymentError::BadRequest("missing Stripe-Signature".into()))?;
    verify_signature(secret, signature, &body, unix_now())
        .map_err(|reason| PaymentError::BadRequest(format!("invalid signature: {reason}")))?;

    let event: Value = serde_json::from_slice(&body).map_err(|e| PaymentError::BadRequest(e.to_string()))?;
    let kind = event["type"].as_str().unwrap_or("unknown");
    let object = &event["data"]["object"];
    match kind {
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => tracing::info!(
            event = %event["id"].as_str().unwrap_or_default(),
            session = %object["id"].as_str().unwrap_or_default(),
            payment_status = %object["payment_status"].as_str().unwrap_or_default(),
            amount_total = object["amount_total"].as_i64().unwrap_or_default(),
            "payment received"
        ),
        other => tracing::info!(event = %event["id"].as_str().unwrap_or_default(), kind = %other, "stripe event"),
    }
    Ok(Json(json!({ "received": true })))
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or_default()
}

/// Checks a `Stripe-Signature` header (`t=…,v1=…[,v1=…]`): HMAC-SHA256 of `"{t}.{body}"`
/// with the webhook secret, compared in constant time, and `t` within the tolerance.
pub fn verify_signature(secret: &str, header: &str, payload: &[u8], now: i64) -> Result<(), &'static str> {
    let mut timestamp = None;
    let mut candidates = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", t)) => timestamp = t.parse::<i64>().ok(),
            Some(("v1", sig)) => candidates.extend(decode_hex(sig)),
            _ => {}
        }
    }
    let timestamp = timestamp.ok_or("no timestamp")?;
    if candidates.is_empty() {
        return Err("no v1 signature");
    }
    if (now - timestamp).abs() > SIGNATURE_TOLERANCE_SECS {
        return Err("timestamp outside tolerance");
    }
    for candidate in candidates {
        let mut mac = hmac::Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| "bad secret")?;
        mac.update(timestamp.to_string().as_bytes());
        mac.update(b".");
        mac.update(payload);
        if mac.verify_slice(&candidate).is_ok() {
            return Ok(());
        }
    }
    Err("no matching signature")
}

/// Signs like Stripe does; for tests and local `curl` experiments.
pub fn sign(secret: &str, payload: &[u8], timestamp: i64) -> String {
    let mut mac = hmac::Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac accepts any key length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    let hex: String = mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    format!("t={timestamp},v1={hex}")
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_round_trip() {
        let header = sign("whsec_test", b"{}", 1_000);
        assert_eq!(verify_signature("whsec_test", &header, b"{}", 1_010), Ok(()));
        assert!(verify_signature("whsec_other", &header, b"{}", 1_010).is_err());
        assert!(verify_signature("whsec_test", &header, b"{ }", 1_010).is_err());
        assert_eq!(verify_signature("whsec_test", &header, b"{}", 2_000), Err("timestamp outside tolerance"));
        assert_eq!(verify_signature("whsec_test", "garbage", b"{}", 1_000), Err("no timestamp"));
    }
}
