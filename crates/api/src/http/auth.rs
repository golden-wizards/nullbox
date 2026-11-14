//! Sign-up, login, logout and "who am I" (#14). Sessions live in the database; the browser
//! only holds an `HttpOnly; Secure; SameSite=Lax` cookie.

use axum::Json;
use axum::extract::State;
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::extract::{Actor, ApiJson, session_token};
use crate::audit::{ActorKind, Entry};
use crate::auth::{SESSION_COOKIE, SESSION_TTL_DAYS, User};
use crate::{ApiError, AppState};

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
    /// Only needed for sign-up when the server sets `SIGNUP_INVITE_CODE`.
    #[serde(default)]
    #[ts(optional)]
    pub invite_code: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct Me {
    pub username: String,
}

pub(super) fn session_cookie(value: &str, max_age_secs: i64) -> HeaderValue {
    let cookie = format!("{SESSION_COOKIE}={value}; Path=/; Max-Age={max_age_secs}; HttpOnly; Secure; SameSite=Lax");
    HeaderValue::from_str(&cookie).expect("cookie is ASCII")
}

fn signed_in(status: StatusCode, user: User, session: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, session_cookie(session, SESSION_TTL_DAYS * 24 * 60 * 60));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    (status, headers, Json(Me { username: user.username })).into_response()
}

/// `POST /api/auth/signup`: create an account and sign in.
pub async fn sign_up(State(state): State<AppState>, ApiJson(req): ApiJson<AuthRequest>) -> Result<Response, ApiError> {
    let (user, session) = match state.auth.sign_up(&req.username, &req.password, req.invite_code.as_deref()).await {
        Ok(signed_up) => signed_up,
        Err(err) => {
            // Bad input isn't worth a line; a wrong invite code or a taken name is.
            if matches!(err, ApiError::Forbidden | ApiError::UsernameTaken) {
                let detail = serde_json::json!({ "username": attempted(&req.username), "reason": err.to_string() });
                state.audit.record(Entry::new(ActorKind::Anonymous, "auth.signup").denied().with_detail(detail)).await;
            }
            return Err(err);
        }
    };
    tracing::info!(username = %user.username, "worker signed up");
    state.audit.record(Entry::worker(&user.username, "auth.signup")).await;
    Ok(signed_in(StatusCode::CREATED, user, &session))
}

/// `POST /api/auth/login`: username and password in, session cookie out.
pub async fn log_in(State(state): State<AppState>, ApiJson(req): ApiJson<AuthRequest>) -> Result<Response, ApiError> {
    match state.auth.log_in(&req.username, &req.password).await {
        Ok((user, session)) => {
            state.audit.record(Entry::worker(&user.username, "auth.login")).await;
            Ok(signed_in(StatusCode::OK, user, &session))
        }
        Err(err) => {
            tracing::warn!(username = %req.username.trim(), "failed login");
            if matches!(err, ApiError::InvalidCredentials) {
                let detail = serde_json::json!({ "username": attempted(&req.username) });
                state.audit.record(Entry::new(ActorKind::Anonymous, "auth.login").denied().with_detail(detail)).await;
            }
            Err(err)
        }
    }
}

/// `POST /api/auth/logout`: end the session and clear the cookie. Always 204.
pub async fn log_out(State(state): State<AppState>, parts: Parts) -> Result<Response, ApiError> {
    if let Some(token) = session_token(&parts) {
        if let Some(user) = state.auth.session_user(&token).await? {
            state.audit.record(Entry::worker(&user.username, "auth.logout")).await;
        }
        state.auth.log_out(&token).await?;
    }
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, session_cookie("", 0));
    Ok((StatusCode::NO_CONTENT, headers).into_response())
}

/// The username someone tried, trimmed and cut to a sane length for the audit log.
fn attempted(username: &str) -> String {
    username.trim().chars().take(100).collect()
}

/// `GET /api/auth/me`: the signed-in worker, or 401.
pub async fn me(actor: Actor) -> Json<Me> {
    Json(Me { username: actor.name })
}

/// `GET /api/auth/gate`: for Caddy's `forward_auth` in front of the members-only pages
/// (`/inbox…`). 204 with a valid session; otherwise a 302 to `/login?next=<page>`.
pub async fn gate(State(state): State<AppState>, parts: Parts) -> Response {
    let signed_in = match session_token(&parts) {
        Some(token) => matches!(state.auth.session_user(&token).await, Ok(Some(_))),
        None => false,
    };
    if signed_in {
        return StatusCode::NO_CONTENT.into_response();
    }
    let next = parts
        .headers
        .get("x-forwarded-uri")
        .and_then(|v| v.to_str().ok())
        .filter(|uri| uri.starts_with('/') && !uri.starts_with("//"))
        .unwrap_or("/inbox");
    let location =
        format!("/login?next={}", percent_encoding::utf8_percent_encode(next, percent_encoding::NON_ALPHANUMERIC));
    let mut headers = HeaderMap::new();
    headers.insert(header::LOCATION, HeaderValue::from_str(&location).unwrap_or(HeaderValue::from_static("/login")));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    (StatusCode::FOUND, headers).into_response()
}
