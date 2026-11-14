//! Audit log (#13): what HOT checks (a ticket with a code in its subject, the worker API with a
//! made-up token), plus the rest of the trail and the append-only table.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nullbox_api::{AppState, Config, db, http};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower::ServiceExt;

async fn app() -> (Router, SqlitePool) {
    let pool = db::connect_in_memory().await.expect("in-memory db");
    let config = Config {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        database_url: "sqlite::memory:".into(),
        cors_allowed_origins: vec!["http://localhost:5173".into()],
        public_web_url: "http://localhost:5173".into(),
        admin_token: Some("test-admin-token".into()),
        mcp_token: Some("test-mcp-token".into()),
    };
    (http::router(AppState::new(pool.clone(), config)), pool)
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let body = match body {
        Some(json) => {
            req = req.header("content-type", "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Signs up a worker; returns the `Cookie` header value of their session.
async fn sign_up(app: &Router, username: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/signup")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "username": username, "password": "correct horse battery" }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    res.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned()
}

async fn audit(app: &Router, cookie: &str, query: &str) -> Vec<Value> {
    let (status, body) = send(app, "GET", &format!("/api/audit{query}"), &[("cookie", cookie)], None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array().unwrap().clone()
}

fn find<'a>(entries: &'a [Value], action: &str, outcome: &str) -> Option<&'a Value> {
    entries.iter().find(|e| e["action"] == action && e["outcome"] == outcome)
}

#[tokio::test]
async fn records_what_hot_checks() {
    let (app, _) = app().await;
    let cookie = sign_up(&app, "auditor").await;

    // HOT opens a ticket with a code in its subject...
    let (status, created) = send(
        &app,
        "POST",
        "/api/tickets",
        &[("x-forwarded-for", "203.0.113.7")],
        Some(json!({ "subject": "HOT trail 7f3a-91c2", "body": "hello", "severity": "p3" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["ticket"]["id"].as_str().unwrap().to_owned();

    // ...and tries the worker API with a made-up token.
    let (status, _) =
        send(&app, "GET", "/api/tickets", &[("authorization", "Bearer made-up-token"), ("x-actor-name", "hot")], None)
            .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let entries = audit(&app, &cookie, "").await;

    let created = find(&entries, "ticket.created", "ok").expect("ticket.created");
    assert_eq!(created["detail"]["subject"], "HOT trail 7f3a-91c2");
    assert_eq!(created["ticket_id"], id.as_str());
    assert_eq!(created["actor_kind"], "customer");
    assert_eq!(created["ip"], "203.0.113.7");
    assert_eq!(created["method"], "POST");
    assert_eq!(created["path"], "/api/tickets");

    let refused = find(&entries, "auth.refused", "denied").expect("auth.refused");
    assert_eq!(refused["detail"]["reason"], "unknown or expired session");
    assert_eq!(refused["detail"]["claimed_actor"], "hot");
    assert_eq!(refused["path"], "/api/tickets");
    // The made-up token itself is never stored.
    assert!(!serde_json::to_string(&entries).unwrap().contains("made-up-token"));
}

#[tokio::test]
async fn records_who_read_and_changed_a_ticket() {
    let (app, _) = app().await;
    let cookie = sign_up(&app, "kenji").await;
    let worker = ("cookie", cookie.as_str());

    let (_, created) = send(
        &app,
        "POST",
        "/api/tickets",
        &[],
        Some(json!({ "subject": "Login broken", "body": "x", "severity": "p2" })),
    )
    .await;
    let id = created["ticket"]["id"].as_str().unwrap().to_owned();
    let token = created["access_token"].as_str().unwrap().to_owned();

    send(&app, "GET", &format!("/api/tickets/{id}"), &[worker], None).await;
    send(&app, "POST", &format!("/api/tickets/{id}/messages"), &[worker], Some(json!({ "body": "On it" }))).await;
    send(&app, "PATCH", &format!("/api/tickets/{id}"), &[worker], Some(json!({ "severity": "p1" }))).await;
    send(&app, "POST", &format!("/api/tickets/{id}/yeet"), &[worker], None).await;
    send(&app, "GET", &format!("/api/public/tickets/{id}"), &[("x-ticket-token", token.as_str())], None).await;

    let entries = audit(&app, &cookie, &format!("?ticket_id={id}")).await;
    let actions: Vec<&str> = entries.iter().rev().map(|e| e["action"].as_str().unwrap()).collect();
    assert_eq!(
        actions,
        [
            "ticket.created",
            "ticket.viewed",
            "message.added",
            "ticket.updated",
            "ticket.status_changed",
            "ticket.yeeted",
            "ticket.viewed"
        ]
    );
    let viewed = find(&entries, "ticket.viewed", "ok").unwrap();
    assert_eq!(viewed["actor_kind"], "customer"); // newest first: the customer's read
    let worker_read = entries.iter().rev().find(|e| e["action"] == "ticket.viewed").unwrap();
    assert_eq!(worker_read["actor"], "kenji");
    let updated = find(&entries, "ticket.updated", "ok").unwrap();
    assert_eq!(updated["detail"]["changes"][0]["kind"], "severity_changed");
    // Message bodies stay out of the log.
    assert!(!serde_json::to_string(&entries).unwrap().contains("On it"));
}

#[tokio::test]
async fn records_sign_ins_and_the_admin_export() {
    let (app, _) = app().await;
    let cookie = sign_up(&app, "mika").await;

    let bad = json!({ "username": "mika", "password": "wrong password!" });
    assert_eq!(send(&app, "POST", "/api/auth/login", &[], Some(bad)).await.0, StatusCode::UNAUTHORIZED);
    let good = json!({ "username": "mika", "password": "correct horse battery" });
    assert_eq!(send(&app, "POST", "/api/auth/login", &[], Some(good)).await.0, StatusCode::OK);
    send(&app, "GET", "/api/admin/export", &[("x-admin-token", "leaked")], None).await;
    send(&app, "GET", "/api/admin/export", &[("x-admin-token", "test-admin-token")], None).await;
    send(&app, "POST", "/mcp", &[("authorization", "Bearer nope")], Some(json!({}))).await;

    let entries = audit(&app, &cookie, "").await;
    assert_eq!(find(&entries, "auth.signup", "ok").unwrap()["actor"], "mika");
    assert_eq!(find(&entries, "auth.login", "denied").unwrap()["detail"]["username"], "mika");
    assert_eq!(find(&entries, "auth.login", "ok").unwrap()["actor"], "mika");
    assert_eq!(find(&entries, "admin.export", "denied").unwrap()["detail"]["reason"], "wrong admin token");
    assert_eq!(find(&entries, "admin.export", "ok").unwrap()["actor_kind"], "admin");
    assert_eq!(find(&entries, "auth.refused", "denied").unwrap()["actor_kind"], "mcp");
    assert!(!serde_json::to_string(&entries).unwrap().contains("wrong password!"));

    // Filters and paging.
    let logins = audit(&app, &cookie, "?action=auth.login&limit=1").await;
    assert_eq!(logins.len(), 1);
    assert_eq!(logins[0]["outcome"], "ok");
    let older = audit(&app, &cookie, &format!("?action=auth.login&before={}", logins[0]["id"].as_str().unwrap())).await;
    assert_eq!(older.len(), 1);
    assert_eq!(older[0]["outcome"], "denied");
}

#[tokio::test]
async fn audit_log_needs_a_login() {
    let (app, _) = app().await;
    let (status, _) = send(&app, "GET", "/api/audit", &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(&app, "GET", "/api/audit", &[("authorization", "Bearer made-up")], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn audit_log_is_append_only() {
    let (app, pool) = app().await;
    sign_up(&app, "nobody").await;
    let updated = sqlx::query("UPDATE audit_log SET actor = 'someone else'").execute(&pool).await;
    assert!(updated.unwrap_err().to_string().contains("append-only"));
    let deleted = sqlx::query("DELETE FROM audit_log").execute(&pool).await;
    assert!(deleted.unwrap_err().to_string().contains("append-only"));
}
