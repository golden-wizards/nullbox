use std::sync::Arc;

use async_trait::async_trait;
use nullbox_domain::DomainEvent;

/// Where domain events go: logs and the audit log (#13) today; email, webhooks and metrics
/// are more implementations of this trait, combined with [`FanOut`].
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn emit(&self, event: DomainEvent);
}

pub struct TracingEventSink;

#[async_trait]
impl EventSink for TracingEventSink {
    async fn emit(&self, event: DomainEvent) {
        let payload = serde_json::to_string(&event).unwrap_or_default();
        tracing::info!(target: "nullbox::events", %payload, "domain event");
    }
}

/// Sends every event to each sink, in order.
pub struct FanOut(pub Vec<Arc<dyn EventSink>>);

#[async_trait]
impl EventSink for FanOut {
    async fn emit(&self, event: DomainEvent) {
        for sink in &self.0 {
            sink.emit(event.clone()).await;
        }
    }
}
