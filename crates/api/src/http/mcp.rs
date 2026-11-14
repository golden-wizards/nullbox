//! MCP server for AI agents at `/mcp` (Model Context Protocol, streamable HTTP transport).
//!
//! A small hand-rolled JSON-RPC 2.0 handler: `POST /mcp` takes one message (or a batch) and
//! answers with `application/json`. Stateless, so no session id and no SSE stream; `GET /mcp`
//! answers 405 as the spec allows. Methods: `initialize`, `ping`, `tools/list`, `tools/call`
//! (notifications get 202). Every tool calls `TicketService`, like the worker handlers do.
//!
//! Auth: `Authorization: Bearer <MCP_TOKEN>`. Without `MCP_TOKEN` set, every request gets 401,
//! so the feature is off by default.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use nullbox_domain::{Severity, TicketStatus};
use percent_encoding::percent_decode_str;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use uuid::Uuid;

use super::extract::ACTOR_HEADER;
use crate::dto::{CreateTicketRequest, UpdateTicketRequest};
use crate::{ApiError, AppState};

/// Protocol versions we speak, newest first. We echo the client's if we know it.
const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
/// Who tool calls are attributed to, unless the client sends `X-Actor-Name`.
const DEFAULT_ACTOR: &str = "MCP agent";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// `POST /mcp`
pub async fn post(State(state): State<AppState>, headers: HeaderMap, body: axum::body::Bytes) -> Response {
    if let Some(denied) = unauthorized(&state, &headers) {
        return denied;
    }
    let actor = actor_name(&headers);
    let message: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => return Json(error_response(Value::Null, PARSE_ERROR, &format!("parse error: {e}"))).into_response(),
    };

    let responses = match message {
        Value::Array(batch) if !batch.is_empty() => {
            let mut out = Vec::new();
            for item in batch {
                if let Some(response) = handle(&state, &actor, item).await {
                    out.push(response);
                }
            }
            if out.is_empty() { None } else { Some(Value::Array(out)) }
        }
        single => handle(&state, &actor, single).await,
    };
    match responses {
        Some(value) => Json(value).into_response(),
        // Only notifications or responses: nothing to answer.
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// `GET`/`DELETE /mcp`: no server-initiated SSE stream and no sessions here.
pub async fn method_not_allowed(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(denied) = unauthorized(&state, &headers) {
        return denied;
    }
    let mut res = StatusCode::METHOD_NOT_ALLOWED.into_response();
    res.headers_mut().insert(header::ALLOW, HeaderValue::from_static("POST"));
    res
}

/// `None` if the bearer token matches `MCP_TOKEN`, else the 401 to send.
fn unauthorized(state: &AppState, headers: &HeaderMap) -> Option<Response> {
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
        .map(str::trim);
    let ok = match (state.config.mcp_token.as_deref(), presented) {
        (Some(expected), Some(given)) => {
            !expected.is_empty() && constant_time_eq(expected.as_bytes(), given.as_bytes())
        }
        _ => false,
    };
    if ok {
        return None;
    }
    let body = json!({ "code": "unauthorized", "message": "MCP needs Authorization: Bearer <MCP_TOKEN>" });
    let mut res = (StatusCode::UNAUTHORIZED, Json(body)).into_response();
    res.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer realm=\"nullbox-mcp\""));
    Some(res)
}

pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn actor_name(headers: &HeaderMap) -> String {
    headers
        .get(ACTOR_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|raw| percent_decode_str(raw).decode_utf8().ok())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty() && name.chars().count() <= 100)
        .unwrap_or_else(|| DEFAULT_ACTOR.to_owned())
}

/// One JSON-RPC message in, at most one response out (`None` for notifications).
async fn handle(state: &AppState, actor: &str, message: Value) -> Option<Value> {
    let Value::Object(msg) = message else {
        return Some(error_response(Value::Null, INVALID_REQUEST, "expected a JSON-RPC object"));
    };
    let id = msg.get("id").cloned();
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        // A response from the client (we never send requests) or garbage.
        return id.map(|id| error_response(id, INVALID_REQUEST, "missing method"));
    };
    let id = id?; // notifications (`notifications/initialized`, ...) need no answer
    let params = msg.get("params").cloned().unwrap_or(Value::Null);

    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(state, actor, &params).await,
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        other => Err((METHOD_NOT_FOUND, format!("method not found: {other}"))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error_response(id, code, &message),
    })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn initialize(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested.filter(|v| PROTOCOL_VERSIONS.contains(v)).unwrap_or(PROTOCOL_VERSIONS[1]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "nullbox", "title": "Nullbox helpdesk", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Nullbox is a helpdesk. Work the inbox like a support worker: list_tickets to see \
            active tickets, get_ticket to read the thread, reply_to_ticket to answer the customer (or \
            internal: true for a note), update_ticket to change status/severity/assignee, and \
            resolve_ticket when done. Tickets are addressed by UUID or by their number."
    })
}

fn tools() -> Value {
    let id = json!({ "type": "string", "description": "Ticket UUID, or the ticket number (e.g. \"42\")" });
    let status = json!({
        "type": "string",
        "enum": ["open", "in_progress", "waiting_on_customer", "resolved", "closed"]
    });
    let severity =
        json!({ "type": "string", "enum": ["p1", "p2", "p3", "p4"], "description": "p1 is the most urgent" });
    json!([
        {
            "name": "list_tickets",
            "title": "List tickets",
            "description": "List tickets. Without status: all active ones (open, in_progress, waiting_on_customer).",
            "inputSchema": { "type": "object", "properties": { "status": status }, "additionalProperties": false },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "get_ticket",
            "title": "Get ticket",
            "description": "A ticket with its full thread, internal notes included.",
            "inputSchema": { "type": "object", "properties": { "id": id }, "required": ["id"] },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "reply_to_ticket",
            "title": "Reply to ticket",
            "description": "Post a reply the customer sees, or an internal note (internal: true).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": id,
                    "body": { "type": "string" },
                    "internal": { "type": "boolean", "default": false }
                },
                "required": ["id", "body"]
            }
        },
        {
            "name": "update_ticket",
            "title": "Update ticket",
            "description": "Change status, severity and/or assignee (empty assignee_name unassigns).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": id,
                    "status": status,
                    "severity": severity,
                    "assignee_name": { "type": "string" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "resolve_ticket",
            "title": "Resolve (yeet) ticket",
            "description": "Mark the ticket resolved. A customer reply reopens it.",
            "inputSchema": { "type": "object", "properties": { "id": id }, "required": ["id"] },
            "annotations": { "idempotentHint": false }
        },
        {
            "name": "create_ticket",
            "title": "Create ticket",
            "description": "Open a new ticket on behalf of a customer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "subject": { "type": "string" },
                    "body": { "type": "string" },
                    "severity": severity,
                    "customer_name": { "type": "string" },
                    "customer_email": { "type": "string" }
                },
                "required": ["subject", "body", "severity"]
            }
        },
        {
            "name": "backlog_stats",
            "title": "Backlog stats",
            "description": "Open ticket count, the backlog beast's mood and the oldest open ticket's age.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        }
    ])
}

type RpcResult = Result<Value, (i64, String)>;

async fn call_tool(state: &AppState, actor: &str, params: &Value) -> RpcResult {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err((INVALID_PARAMS, "tools/call needs a tool name".into()));
    };
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let outcome = match name {
        "list_tickets" => list_tickets(state, &args).await,
        "get_ticket" => get_ticket(state, &args).await,
        "reply_to_ticket" => reply_to_ticket(state, actor, &args).await,
        "update_ticket" => update_ticket(state, actor, &args).await,
        "resolve_ticket" => resolve_ticket(state, actor, &args).await,
        "create_ticket" => create_ticket(state, &args).await,
        "backlog_stats" => to_value(state.tickets.backlog_stats().await),
        other => return Err((INVALID_PARAMS, format!("unknown tool: {other}"))),
    };
    // Tool failures are results with `isError`, so the model can read them and recover.
    Ok(match outcome {
        Ok(value) => json!({
            "content": [{ "type": "text", "text": value.to_string() }],
            "structuredContent": if value.is_object() { value.clone() } else { json!({ "result": value }) },
            "isError": false
        }),
        Err(err) => json!({
            "content": [{ "type": "text", "text": format!("{:?}: {err}", err.code()) }],
            "isError": true
        }),
    })
}

fn to_value<T: serde::Serialize>(result: Result<T, ApiError>) -> Result<Value, ApiError> {
    result.and_then(|v| serde_json::to_value(v).map_err(|e| ApiError::WorksOnMyMachine(e.to_string())))
}

fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<Option<T>, ApiError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone()).map(Some).map_err(|e| ApiError::Validation(format!("{key}: {e}"))),
    }
}

fn required_arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, ApiError> {
    arg(args, key)?.ok_or_else(|| ApiError::Validation(format!("{key} is required")))
}

/// Accepts a UUID or a ticket number (as number or string), which is what humans and agents quote.
async fn ticket_id(state: &AppState, args: &Value) -> Result<Uuid, ApiError> {
    let raw = args.get("id").ok_or_else(|| ApiError::Validation("id is required".into()))?;
    let number = match raw {
        Value::String(s) => {
            let s = s.trim().trim_start_matches('#');
            if let Ok(id) = Uuid::parse_str(s) {
                return Ok(id);
            }
            s.parse::<i64>().map_err(|_| ApiError::Validation("id must be a UUID or a ticket number".into()))?
        }
        Value::Number(n) => n.as_i64().ok_or_else(|| ApiError::Validation("id must be an integer".into()))?,
        _ => return Err(ApiError::Validation("id must be a UUID or a ticket number".into())),
    };
    state.tickets.export().await?.into_iter().find(|t| t.number == number).map(|t| t.id).ok_or(ApiError::NotFound)
}

async fn list_tickets(state: &AppState, args: &Value) -> Result<Value, ApiError> {
    let status: Option<TicketStatus> = arg(args, "status")?;
    let tickets = state.tickets.list(status).await?;
    Ok(json!({ "count": tickets.len(), "tickets": tickets }))
}

async fn get_ticket(state: &AppState, args: &Value) -> Result<Value, ApiError> {
    let id = ticket_id(state, args).await?;
    to_value(state.tickets.worker_thread(id).await)
}

async fn reply_to_ticket(state: &AppState, actor: &str, args: &Value) -> Result<Value, ApiError> {
    let id = ticket_id(state, args).await?;
    let body: String = required_arg(args, "body")?;
    let internal: bool = arg(args, "internal")?.unwrap_or(false);
    to_value(state.tickets.worker_message(actor, id, &body, internal).await)
}

async fn update_ticket(state: &AppState, actor: &str, args: &Value) -> Result<Value, ApiError> {
    let id = ticket_id(state, args).await?;
    let req = UpdateTicketRequest {
        status: arg(args, "status")?,
        severity: arg::<Severity>(args, "severity")?,
        assignee_name: arg(args, "assignee_name")?,
    };
    to_value(state.tickets.update(actor, id, req).await)
}

async fn resolve_ticket(state: &AppState, actor: &str, args: &Value) -> Result<Value, ApiError> {
    let id = ticket_id(state, args).await?;
    to_value(state.tickets.yeet_ticket(actor, id).await)
}

async fn create_ticket(state: &AppState, args: &Value) -> Result<Value, ApiError> {
    let req = CreateTicketRequest {
        subject: required_arg(args, "subject")?,
        body: required_arg(args, "body")?,
        severity: required_arg(args, "severity")?,
        customer_name: arg(args, "customer_name")?,
        customer_email: arg(args, "customer_email")?,
    };
    let created = state.tickets.create(req).await?;
    to_value(Ok(created.ticket))
}
