use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use nullbox_domain::Message;
use uuid::Uuid;

use super::extract::{ApiJson, ApiPath, TicketToken};
use crate::audit::{ActorKind, Entry};
use crate::dto::{CreateTicketRequest, CreateTicketResponse, CustomerReplyRequest, TicketThread};
use crate::otlp::SpanAttributes;
use crate::{ApiError, AppState};

pub async fn create_ticket(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CreateTicketRequest>,
) -> Result<(StatusCode, Extension<SpanAttributes>, Json<CreateTicketResponse>), ApiError> {
    let created = state.tickets.create(req).await?;
    let access_link = format!("{}/t/{}#{}", state.config.public_web_url, created.ticket.id, created.access_token);
    // Exported on this request's OTLP span, so a ticket can be found by its subject.
    let span_attributes = SpanAttributes(vec![
        ("ticket.id", created.ticket.id.to_string()),
        ("ticket.number", created.ticket.number.to_string()),
        ("ticket.subject", created.ticket.subject.clone()),
    ]);
    Ok((
        StatusCode::CREATED,
        Extension(span_attributes),
        Json(CreateTicketResponse { ticket: created.ticket, access_token: created.access_token, access_link }),
    ))
}

pub async fn get_ticket(
    State(state): State<AppState>,
    TicketToken(token): TicketToken,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<TicketThread>, ApiError> {
    let thread = state.tickets.customer_thread(id, &token).await?;
    state.audit.record(Entry::new(ActorKind::Customer, "ticket.viewed").with_ticket(id)).await;
    Ok(Json(thread))
}

pub async fn reply(
    State(state): State<AppState>,
    TicketToken(token): TicketToken,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(req): ApiJson<CustomerReplyRequest>,
) -> Result<(StatusCode, Json<Message>), ApiError> {
    let message = state.tickets.customer_reply(id, &token, &req.body).await?;
    Ok((StatusCode::CREATED, Json(message)))
}
