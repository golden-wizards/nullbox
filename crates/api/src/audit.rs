//! Audit log (#13): who did what to which ticket, sign-ins, and refused requests.
//!
//! Entries go to the append-only `audit_log` table (see the migration) and to `tracing`
//! (target `nullbox::audit`). Domain events arrive through [`EventSink`]; things that aren't
//! domain events (refused requests, logins, ticket reads, the admin export) call
//! [`AuditLog::record`] directly.
//!
//! The client address, method and path come from [`with_request_context`], a middleware that
//! stores them for the rest of the request. Nothing secret is ever recorded: no tokens, no
//! passwords, no message bodies.

use async_trait::async_trait;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use chrono::{SecondsFormat, Utc};
use nullbox_domain::DomainEvent;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use ts_rs::TS;
use uuid::Uuid;

use crate::ApiError;
use crate::events::EventSink;

/// Who acted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ActorKind {
    /// A signed-in worker.
    Worker,
    /// A customer, with their ticket link.
    Customer,
    /// The admin export, with `X-Admin-Token`.
    Admin,
    /// An AI agent on `/mcp`.
    Mcp,
    /// Nobody we could identify.
    Anonymous,
}

impl ActorKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Worker => "worker",
            Self::Customer => "customer",
            Self::Admin => "admin",
            Self::Mcp => "mcp",
            Self::Anonymous => "anonymous",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Outcome {
    Ok,
    Denied,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Denied => "denied",
        }
    }
}

/// One thing to record. Build it with [`Entry::new`] and the `with_*` helpers.
#[derive(Debug, Clone)]
pub struct Entry {
    pub actor: Option<String>,
    pub actor_kind: ActorKind,
    pub action: &'static str,
    pub outcome: Outcome,
    pub ticket_id: Option<Uuid>,
    pub detail: Value,
}

impl Entry {
    pub fn new(actor_kind: ActorKind, action: &'static str) -> Self {
        Self { actor: None, actor_kind, action, outcome: Outcome::Ok, ticket_id: None, detail: json!({}) }
    }

    /// A signed-in worker did something.
    pub fn worker(name: &str, action: &'static str) -> Self {
        Self::new(ActorKind::Worker, action).with_actor(Some(name))
    }

    pub fn with_actor(mut self, actor: Option<&str>) -> Self {
        self.actor = actor.map(str::to_owned);
        self
    }

    pub fn denied(mut self) -> Self {
        self.outcome = Outcome::Denied;
        self
    }

    pub fn with_ticket(mut self, id: Uuid) -> Self {
        self.ticket_id = Some(id);
        self
    }

    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = detail;
        self
    }
}

/// A recorded entry, as `GET /api/audit` returns it.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct AuditEntry {
    pub id: String,
    /// RFC 3339, UTC.
    pub at: String,
    pub actor: Option<String>,
    pub actor_kind: ActorKind,
    pub action: String,
    pub outcome: Outcome,
    pub ticket_id: Option<String>,
    pub ip: Option<String>,
    pub method: Option<String>,
    pub path: Option<String>,
    #[ts(type = "Record<string, unknown>")]
    pub detail: Value,
}

/// Filters for `GET /api/audit`. Newest first; page with `before` (an entry id).
#[derive(Debug, Default, Deserialize)]
pub struct AuditQuery {
    pub ticket_id: Option<Uuid>,
    pub action: Option<String>,
    pub actor: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
}

const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 500;

/// Where the request came from, kept for the duration of one request.
#[derive(Debug, Clone, Default)]
struct RequestContext {
    ip: Option<String>,
    method: String,
    path: String,
}

tokio::task_local! {
    static REQUEST: RequestContext;
}

/// Middleware: remembers the client address, method and path so entries recorded while
/// handling this request carry them.
///
/// The client address is the first `X-Forwarded-For` hop. That's trustworthy only because the
/// API is reachable through Caddy alone, which overwrites the header for untrusted clients.
pub async fn with_request_context(req: Request, next: Next) -> Response {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.chars().take(64).collect());
    let context = RequestContext { ip, method: req.method().to_string(), path: req.uri().path().to_owned() };
    REQUEST.scope(context, next.run(req)).await
}

pub struct AuditLog {
    pool: SqlitePool,
}

impl AuditLog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Appends an entry. A failed write is logged and doesn't fail the request.
    pub async fn record(&self, entry: Entry) {
        let context = REQUEST.try_with(Clone::clone).unwrap_or_default();
        let id = Uuid::now_v7();
        let at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let method = Some(context.method).filter(|m| !m.is_empty());
        let path = Some(context.path).filter(|p| !p.is_empty());
        tracing::info!(
            target: "nullbox::audit",
            action = entry.action,
            outcome = entry.outcome.as_str(),
            actor = entry.actor.as_deref().unwrap_or("-"),
            actor_kind = entry.actor_kind.as_str(),
            ticket_id = entry.ticket_id.map(|id| id.to_string()).unwrap_or_default(),
            ip = context.ip.as_deref().unwrap_or("-"),
            path = path.as_deref().unwrap_or("-"),
            detail = %entry.detail,
            "audit"
        );
        let written = sqlx::query(
            "INSERT INTO audit_log (id, at, actor, actor_kind, action, outcome, ticket_id, ip, method, path, detail)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(at)
        .bind(entry.actor)
        .bind(entry.actor_kind.as_str())
        .bind(entry.action)
        .bind(entry.outcome.as_str())
        .bind(entry.ticket_id.map(|id| id.to_string()))
        .bind(context.ip)
        .bind(method)
        .bind(path)
        .bind(entry.detail.to_string())
        .execute(&self.pool)
        .await;
        if let Err(e) = written {
            tracing::error!(target: "nullbox::audit", error = %e, action = entry.action, "audit write failed");
        }
    }

    /// Entries matching the filters, newest first.
    pub async fn list(&self, query: &AuditQuery) -> Result<Vec<AuditEntry>, ApiError> {
        let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT id, at, actor, actor_kind, action, outcome, ticket_id, ip, method, path, detail
             FROM audit_log
             WHERE (?1 IS NULL OR ticket_id = ?1)
               AND (?2 IS NULL OR action = ?2)
               AND (?3 IS NULL OR actor = ?3 COLLATE nocase)
               AND (?4 IS NULL OR id < ?4)
             ORDER BY id DESC
             LIMIT ?5",
        )
        .bind(query.ticket_id.map(|id| id.to_string()))
        .bind(query.action.as_deref().filter(|v| !v.is_empty()))
        .bind(query.actor.as_deref().filter(|v| !v.is_empty()))
        .bind(query.before.as_deref().filter(|v| !v.is_empty()))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::from(crate::repo::RepoError::Database(e)))?;
        Ok(rows.into_iter().map(into_entry).collect())
    }
}

/// An `audit_log` row, in column order.
type Row = (
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

fn into_entry(row: Row) -> AuditEntry {
    let (id, at, actor, actor_kind, action, outcome, ticket_id, ip, method, path, detail) = row;
    let actor_kind = serde_json::from_value(Value::String(actor_kind)).unwrap_or(ActorKind::Anonymous);
    let outcome = serde_json::from_value(Value::String(outcome)).unwrap_or(Outcome::Denied);
    let detail = serde_json::from_str(&detail).unwrap_or_else(|_| json!({}));
    AuditEntry { id, at, actor, actor_kind, action, outcome, ticket_id, ip, method, path, detail }
}

/// Domain events, as audit entries. `actor: None` on a ticket event means the customer.
#[async_trait]
impl EventSink for AuditLog {
    async fn emit(&self, event: DomainEvent) {
        let entry = match event {
            DomainEvent::TicketCreated { ticket_id, number, severity, subject } => {
                Entry::new(ActorKind::Customer, "ticket.created")
                    .with_ticket(ticket_id)
                    .with_detail(json!({ "number": number, "severity": severity, "subject": subject }))
            }
            DomainEvent::StatusChanged { ticket_id, from, to, actor } => ticket_entry(actor, "ticket.status_changed")
                .with_ticket(ticket_id)
                .with_detail(json!({ "from": from, "to": to })),
            DomainEvent::MessageAdded { ticket_id, message_id, internal, actor } => {
                ticket_entry(actor, "message.added")
                    .with_ticket(ticket_id)
                    .with_detail(json!({ "message_id": message_id, "internal": internal }))
            }
            DomainEvent::TicketYeeted { ticket_id, actor } => {
                Entry::worker(&actor, "ticket.yeeted").with_ticket(ticket_id)
            }
            DomainEvent::TicketUpdated { ticket_id, actor, changes } => Entry::worker(&actor, "ticket.updated")
                .with_ticket(ticket_id)
                .with_detail(json!({ "changes": changes })),
        };
        self.record(entry).await;
    }
}

fn ticket_entry(actor: Option<String>, action: &'static str) -> Entry {
    match actor {
        Some(name) => Entry::worker(&name, action),
        None => Entry::new(ActorKind::Customer, action),
    }
}
