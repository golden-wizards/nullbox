use anyhow::Context;
use nullbox_api::{AppState, Config, db, http};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=debug".into());
    // LOG_FORMAT=json: one JSON object per line; the span list carries trace_id/span_id.
    if std::env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json")) {
        tracing_subscriber::fmt()
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(true)
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
    http::observability::init();
    std::panic::set_hook(Box::new(|info| {
        tracing::error!(target: "EXISTENTIAL", %info, "the server has stared into the void, and the void panicked back");
    }));

    let config = Config::from_env().context("reading config")?;
    let pool = db::connect(&config.database_url).await.context("opening the database")?;
    let bind_addr = config.bind_addr;
    let app = http::router(AppState::new(pool, config));

    let listener = tokio::net::TcpListener::bind(bind_addr).await.with_context(|| format!("binding {bind_addr}"))?;
    tracing::info!(%bind_addr, "Nullbox is listening. The void awaits.");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("shutting down, tickets will have to wait");
        })
        .await?;
    Ok(())
}
