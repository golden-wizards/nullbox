//! `GET /api/audit` (#13): the audit log, newest first, for signed-in workers.

use axum::Json;
use axum::extract::State;

use super::extract::{Actor, ApiQuery};
use crate::audit::{AuditEntry, AuditQuery};
use crate::policy::{self, Action, Resource};
use crate::{ApiError, AppState};

/// Filters: `ticket_id`, `action`, `actor`; paging: `before` (an entry id) and `limit` (max 500).
pub async fn list(
    State(state): State<AppState>,
    actor: Actor,
    ApiQuery(query): ApiQuery<AuditQuery>,
) -> Result<Json<Vec<AuditEntry>>, ApiError> {
    policy::ensure(&actor, Action::ViewAuditLog, Resource::AuditLog)?;
    Ok(Json(state.audit.list(&query).await?))
}
