//! Request extractors. `Actor` is the auth seam.

use axum::Json;
use axum::extract::{FromRequest, FromRequestParts, Path, Query};
use axum::http::header;
use axum::http::request::Parts;

use crate::audit::{ActorKind, Entry};
use crate::auth::SESSION_COOKIE;
use crate::{ApiError, AppState};

pub const ACTOR_HEADER: &str = "x-actor-name";
pub const ADMIN_TOKEN_HEADER: &str = "x-admin-token";
pub const TICKET_TOKEN_HEADER: &str = "x-ticket-token";

/// The signed-in worker making the request.
///
/// Identity comes from the server-side session (#14): the `nullbox_session` cookie set by
/// `POST /api/auth/login`, or `Authorization: Bearer <session token>` for API clients.
/// The old `X-Actor-Name` header is no longer trusted. No valid session: 401, recorded in the
/// audit log as `auth.refused` (#13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub name: String,
}

/// The session token from the request: the session cookie, else a bearer token.
pub fn session_token(parts: &Parts) -> Option<String> {
    let from_cookie = parts
        .headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value.trim().to_owned());
    let from_bearer = || {
        parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|v| v.trim().to_owned())
    };
    from_cookie.or_else(from_bearer).filter(|token| !token.is_empty())
}

impl FromRequestParts<AppState> for Actor {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let reason = match session_token(parts) {
            None => "no session",
            Some(token) => match state.auth.session_user(&token).await? {
                Some(user) => return Ok(Actor { name: user.username }),
                None => "unknown or expired session",
            },
        };
        // Who it claimed to be, if anyone: the old header, still sent by some clients.
        let claimed = parts
            .headers
            .get(ACTOR_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.chars().take(100).collect::<String>());
        let mut detail = serde_json::json!({ "reason": reason });
        if let Some(claimed) = claimed {
            detail["claimed_actor"] = claimed.into();
        }
        state.audit.record(Entry::new(ActorKind::Anonymous, "auth.refused").denied().with_detail(detail)).await;
        Err(ApiError::MissingActor)
    }
}

/// The customer's secret, from `X-Ticket-Token`. The frontend reads it from the URL fragment.
pub struct TicketToken(pub String);

impl<S: Send + Sync> FromRequestParts<S> for TicketToken {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .headers
            .get(TICKET_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty())
            .map(|v| TicketToken(v.to_owned()))
            .ok_or(ApiError::MissingTicketToken)
    }
}

/// `Json`, `Path` and `Query` with rejections in our `{ code, message }` shape.
#[derive(FromRequest)]
#[from_request(via(Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(Path), rejection(ApiError))]
pub struct ApiPath<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(Query), rejection(ApiError))]
pub struct ApiQuery<T>(pub T);
