//! Nullbox API: the HTTP layer, services and storage around `nullbox-domain`.
//!
//! Extension seams, each in one place:
//! - auth: [`http::extract::Actor`]
//! - authorization: [`policy::can`]
//! - audit log: [`audit::AuditLog`] (also an [`events::EventSink`])
//! - webhooks/metrics: [`events::EventSink`]
//! - middleware (tracing, CORS, rate limits...): [`http::middleware`]
//! - storage: [`repo::TicketRepository`]
//! - settings: [`config::Config`]

pub mod audit;
pub mod auth;
pub mod config;
pub mod db;
pub mod dto;
pub mod error;
pub mod events;
pub mod http;
pub mod otlp;
pub mod policy;
pub mod repo;
pub mod service;
pub mod token;

use std::sync::Arc;

pub use config::Config;
pub use error::ApiError;

/// Shared state for all handlers.
#[derive(Clone)]
pub struct AppState {
    pub tickets: Arc<service::TicketService>,
    pub config: Arc<Config>,
    pub auth: Arc<auth::AuthService>,
    pub audit: Arc<audit::AuditLog>,
}

impl AppState {
    pub fn new(pool: sqlx::SqlitePool, config: Config) -> Self {
        let auth = Arc::new(auth::AuthService::from_env(pool.clone()));
        let audit = Arc::new(audit::AuditLog::new(pool.clone()));
        let repo = Arc::new(repo::SqliteTicketRepository::new(pool));
        let events = Arc::new(events::FanOut(vec![Arc::new(events::TracingEventSink), audit.clone()]));
        Self { tickets: Arc::new(service::TicketService::new(repo, events)), config: Arc::new(config), auth, audit }
    }
}
