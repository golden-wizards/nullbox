//! HTTP tests: the whole router against in-memory SQLite, via `oneshot`.
//! Copy a test, change the requests, done.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nullbox_api::{AppState, Config, db, http};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn app() -> Router {
    app_with(|_| {}).await
}

/// Like `app()`, but lets a test tweak the config first.
async fn app_with(tweak: impl FnOnce(&mut Config)) -> Router {
    let pool = db::connect_in_memory().await.expect("in-memory db");
    let mut config = Config {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        database_url: "sqlite::memory:".into(),
        cors_allowed_origins: vec!["http://localhost:5173".into()],
        public_web_url: "http://localhost:5173".into(),
        admin_token: Some("test-admin-token".into()),
        mcp_token: Some("test-mcp-token".into()),
    };
    tweak(&mut config);
    http::router(AppState::new(pool, config))
}

/// Send a request; returns status and JSON body (`Null` if empty).
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

/// Signs up a worker and returns the `Cookie` header value of their session.
async fn sign_up(app: &Router, username: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/signup")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "username": username, "password": "correct horse battery" }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let set_cookie = res.headers()["set-cookie"].to_str().unwrap();
    set_cookie.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn customer_and_worker_round_trip() {
    let app = app().await;
    let cookie = sign_up(&app, "山田").await;
    let worker = ("cookie", cookie.as_str());

    // Anonymous customer creates a ticket.
    let (status, created) = send(
        &app,
        "POST",
        "/api/tickets",
        &[],
        Some(json!({ "subject": "Printer on fire", "body": "Again.", "severity": "p1" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["ticket"]["id"].as_str().unwrap().to_owned();
    let token = created["access_token"].as_str().unwrap().to_owned();
    assert_eq!(created["ticket"]["number"], 1);
    assert!(created["ticket"]["anon"]["animal"].is_string());
    assert!(created["access_link"].as_str().unwrap().ends_with(&format!("/t/{id}#{token}")));

    // The secret link works; a wrong token looks like a missing ticket.
    let (status, thread) =
        send(&app, "GET", &format!("/api/public/tickets/{id}"), &[("x-ticket-token", &token)], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(thread["ticket"]["subject"], "Printer on fire");
    let (status, error) =
        send(&app, "GET", &format!("/api/public/tickets/{id}"), &[("x-ticket-token", "nope")], None).await;
    assert_eq!((status, &error["code"]), (StatusCode::NOT_FOUND, &json!("not_found")));

    // The worker sees it in the inbox, notes internally and yeets it.
    let (_, inbox) = send(&app, "GET", "/api/tickets", &[worker], None).await;
    assert_eq!(inbox.as_array().unwrap().len(), 1);
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/tickets/{id}/messages"),
        &[worker],
        Some(json!({ "body": "it's always DNS", "internal": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, yeeted) = send(&app, "POST", &format!("/api/tickets/{id}/yeet"), &[worker], None).await;
    assert_eq!((status, &yeeted["status"]), (StatusCode::OK, &json!("resolved")));

    let (_, stats) = send(&app, "GET", "/api/stats/backlog", &[worker], None).await;
    assert_eq!(stats, json!({ "open_count": 0, "beast_mood": "asleep", "oldest_open_age_secs": null }));

    // The customer never sees internal notes, and replying reopens the ticket.
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/public/tickets/{id}/messages"),
        &[("x-ticket-token", &token)],
        Some(json!({ "body": "Still on fire" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, thread) =
        send(&app, "GET", &format!("/api/public/tickets/{id}"), &[("x-ticket-token", &token)], None).await;
    assert_eq!(thread["ticket"]["status"], "open");
    let messages = thread["messages"].as_array().unwrap();
    assert!(messages.iter().all(|m| m["internal"] == false));
    assert!(
        messages
            .iter()
            .any(|m| m["system_event"] == json!({ "kind": "status_changed", "from": "resolved", "to": "open" }))
    );

    // The worker view does include the note, written by 山田.
    let (_, thread) = send(&app, "GET", &format!("/api/tickets/{id}"), &[worker], None).await;
    let note = thread["messages"].as_array().unwrap().iter().find(|m| m["internal"] == true).unwrap();
    assert_eq!(note["author_name"], "山田");
}

#[tokio::test]
async fn a_ticket_token_only_opens_its_own_ticket() {
    let app = app().await;
    let cookie = sign_up(&app, "山田").await;
    let worker = ("cookie", cookie.as_str());
    let mut tickets = Vec::new();
    for subject in ["Ticket A", "Ticket B"] {
        let (status, created) =
            send(&app, "POST", "/api/tickets", &[], Some(json!({ "subject": subject, "body": "x", "severity": "p3" })))
                .await;
        assert_eq!(status, StatusCode::CREATED);
        tickets.push((
            created["ticket"]["id"].as_str().unwrap().to_owned(),
            created["access_token"].as_str().unwrap().to_owned(),
        ));
    }
    let (a_token, (b_id, _)) = (&tickets[0].1, &tickets[1]);

    // A's token on B's thread and reply endpoints looks like a missing ticket.
    let a_auth = [("x-ticket-token", a_token.as_str())];
    let (status, error) = send(&app, "GET", &format!("/api/public/tickets/{b_id}"), &a_auth, None).await;
    assert_eq!((status, &error["code"]), (StatusCode::NOT_FOUND, &json!("not_found")));
    let (status, error) = send(
        &app,
        "POST",
        &format!("/api/public/tickets/{b_id}/messages"),
        &a_auth,
        Some(json!({ "body": "not my ticket" })),
    )
    .await;
    assert_eq!((status, &error["code"]), (StatusCode::NOT_FOUND, &json!("not_found")));

    // Nothing was written to B.
    let (_, thread) = send(&app, "GET", &format!("/api/tickets/{b_id}"), &[worker], None).await;
    assert!(thread["messages"].as_array().unwrap().iter().all(|m| m["body"] != "not my ticket"));
}

#[tokio::test]
async fn errors_share_one_shape() {
    let app = app().await;
    let cookie = sign_up(&app, "山田").await;
    let worker = ("cookie", cookie.as_str());

    let (status, error) = send(&app, "GET", "/api/tickets", &[], None).await;
    assert_eq!((status, &error["code"]), (StatusCode::UNAUTHORIZED, &json!("missing_actor")));

    let (status, error) =
        send(&app, "POST", "/api/tickets", &[], Some(json!({ "subject": " ", "body": "x", "severity": "p4" }))).await;
    assert_eq!((status, &error["code"]), (StatusCode::UNPROCESSABLE_ENTITY, &json!("validation_failed")));

    let (status, error) = send(&app, "GET", "/api/tickets/not-a-uuid", &[worker], None).await;
    assert_eq!((status, &error["code"]), (StatusCode::NOT_FOUND, &json!("not_found")));

    let (status, error) = send(&app, "GET", "/api/teapot", &[], None).await;
    assert_eq!((status, &error["code"]), (StatusCode::IM_A_TEAPOT, &json!("im_a_teapot")));
}

#[tokio::test]
async fn admin_export_needs_the_admin_token() {
    let app = app().await;
    let ticket = json!({ "subject": "Export me", "body": "x", "severity": "p3", "customer_email": "a@example.com" });
    send(&app, "POST", "/api/tickets", &[], Some(ticket)).await;

    let (status, error) = send(&app, "GET", "/api/admin/export", &[], None).await;
    assert_eq!((status, &error["code"]), (StatusCode::FORBIDDEN, &json!("forbidden")));
    let (status, _) = send(&app, "GET", "/api/admin/export", &[("x-admin-token", "nope")], None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let admin = [("x-admin-token", "test-admin-token")];
    let (status, tickets) = send(&app, "GET", "/api/admin/export", &admin, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tickets[0]["customer_email"], "a@example.com");
}

#[tokio::test]
async fn admin_export_is_off_without_an_admin_token() {
    let app = app_with(|config| config.admin_token = None).await;
    for headers in [&[][..], &[("x-admin-token", "")][..], &[("x-admin-token", "test-admin-token")][..]] {
        let (status, error) = send(&app, "GET", "/api/admin/export", headers, None).await;
        assert_eq!((status, &error["code"]), (StatusCode::FORBIDDEN, &json!("forbidden")));
    }
}

const MCP_AUTH: (&str, &str) = ("authorization", "Bearer test-mcp-token");

async fn rpc(app: &Router, method: &str, params: Value) -> Value {
    let (status, res) = send(
        app,
        "POST",
        "/mcp",
        &[MCP_AUTH],
        Some(json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["id"], 7);
    res
}

#[tokio::test]
async fn mcp_needs_the_bearer_token() {
    let app = app().await;
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let (status, _) = send(&app, "POST", "/mcp", &[], Some(ping.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(&app, "POST", "/mcp", &[("authorization", "Bearer nope")], Some(ping.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, res) = send(&app, "POST", "/mcp", &[MCP_AUTH], Some(ping)).await;
    assert_eq!((status, &res["result"]), (StatusCode::OK, &json!({})));
}

#[tokio::test]
async fn mcp_agent_works_a_ticket() {
    let app = app().await;

    let init = rpc(&app, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } })).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    let (status, _) = send(
        &app,
        "POST",
        "/mcp",
        &[MCP_AUTH],
        Some(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let list = rpc(&app, "tools/list", json!({})).await;
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for tool in ["list_tickets", "get_ticket", "reply_to_ticket", "update_ticket", "resolve_ticket"] {
        assert!(names.contains(&tool), "{tool} missing");
    }

    let (status, _) = send(
        &app,
        "POST",
        "/api/tickets",
        &[],
        Some(json!({ "subject": "VPN down", "body": "Help", "severity": "p2" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let call = |name: &str, arguments: Value| json!({ "name": name, "arguments": arguments });
    let listed = rpc(&app, "tools/call", call("list_tickets", json!({}))).await;
    assert_eq!(listed["result"]["isError"], false);
    assert_eq!(listed["result"]["structuredContent"]["tickets"][0]["subject"], "VPN down");

    let replied = rpc(&app, "tools/call", call("reply_to_ticket", json!({ "id": 1, "body": "Restarted it." }))).await;
    assert_eq!(replied["result"]["structuredContent"]["author_name"], "MCP agent");
    let resolved = rpc(&app, "tools/call", call("resolve_ticket", json!({ "id": "#1" }))).await;
    assert_eq!(resolved["result"]["structuredContent"]["status"], "resolved");

    let thread = rpc(&app, "tools/call", call("get_ticket", json!({ "id": 1 }))).await;
    assert_eq!(thread["result"]["structuredContent"]["ticket"]["status"], "resolved");

    let missing = rpc(&app, "tools/call", call("get_ticket", json!({ "id": 99 }))).await;
    assert_eq!(missing["result"]["isError"], true);
    let unknown = rpc(&app, "nope/nope", json!({})).await;
    assert_eq!(unknown["error"]["code"], -32601);
}
