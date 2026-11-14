//! Admin endpoints. One so far: the export the CEO pastes into a spreadsheet every Monday.

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use nullbox_domain::Ticket;

use super::extract::ADMIN_TOKEN_HEADER;
use super::mcp::constant_time_eq;
use crate::audit::{ActorKind, Entry};
use crate::{ApiError, AppState};

/// Every ticket, with customer names and emails. Guarded by `X-Admin-Token`, which
/// must match `ADMIN_TOKEN` (see `config.rs`). With no `ADMIN_TOKEN` set, nothing matches.
pub async fn export(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Vec<Ticket>>, ApiError> {
    let token = headers.get(ADMIN_TOKEN_HEADER).and_then(|value| value.to_str().ok());
    let ok = match (state.config.admin_token.as_deref(), token) {
        (Some(expected), Some(given)) => constant_time_eq(expected.as_bytes(), given.as_bytes()),
        _ => false,
    };
    if !ok {
        let reason = if token.is_some() { "wrong admin token" } else { "no admin token" };
        let detail = serde_json::json!({ "reason": reason });
        state.audit.record(Entry::new(ActorKind::Anonymous, "admin.export").denied().with_detail(detail)).await;
        return Err(ApiError::Forbidden);
    }
    let tickets = state.tickets.export().await?;
    let detail = serde_json::json!({ "tickets": tickets.len() });
    state.audit.record(Entry::new(ActorKind::Admin, "admin.export").with_detail(detail)).await;
    Ok(Json(tickets))
}
