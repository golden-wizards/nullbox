//! Login (#14): sign-up, login, sessions, and the members-only gate.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nullbox_api::{AppState, Config, db, http};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn app() -> Router {
    let pool = db::connect_in_memory().await.expect("in-memory db");
    let config = Config {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        database_url: "sqlite::memory:".into(),
        cors_allowed_origins: vec!["http://localhost:5173".into()],
        public_web_url: "http://localhost:5173".into(),
        admin_token: "test-admin-token".into(),
        // Any settings added later keep their env/defaults.
        ..Config::from_env().expect("config")
    };
    http::router(AppState::new(pool, config))
}

struct Res {
    status: StatusCode,
    set_cookie: Option<String>,
    location: Option<String>,
    body: Value,
}

async fn send(app: &Router, method: &str, uri: &str, headers: &[(&str, &str)], body: Option<Value>) -> Res {
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
    let header = |name: &str| res.headers().get(name).map(|v| v.to_str().unwrap().to_owned());
    let set_cookie = header("set-cookie");
    let location = header("location");
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    Res { status, set_cookie, location, body: serde_json::from_slice(&bytes).unwrap_or(Value::Null) }
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn members_only_endpoints_need_a_session() {
    let app = app().await;
    for (method, uri) in [
        ("GET", "/api/tickets"),
        ("GET", "/api/stats/backlog"),
        ("GET", "/api/tickets/0190a0a0-0000-7000-8000-000000000000"),
        ("POST", "/api/tickets/0190a0a0-0000-7000-8000-000000000000/yeet"),
        ("GET", "/api/auth/me"),
    ] {
        let res = send(&app, method, uri, &[], None).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
    // The old trusted name header no longer gets anyone in.
    let res = send(&app, "GET", "/api/tickets", &[("x-actor-name", "admin")], None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    // Nor does a made-up session.
    let res = send(&app, "GET", "/api/tickets", &[("cookie", "nullbox_session=forged")], None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sign_up_log_in_log_out() {
    let app = app().await;
    let creds = json!({ "username": "kenji", "password": "hunter2hunter2" });

    let res = send(&app, "POST", "/api/auth/signup", &[], Some(creds.clone())).await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.body["username"], "kenji");
    let set_cookie = res.set_cookie.unwrap();
    for attr in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"] {
        assert!(set_cookie.contains(attr), "{set_cookie} lacks {attr}");
    }

    // Same name again (any case) is taken.
    let res =
        send(&app, "POST", "/api/auth/signup", &[], Some(json!({ "username": "KENJI", "password": "whatever123" })))
            .await;
    assert_eq!((res.status, &res.body["code"]), (StatusCode::CONFLICT, &json!("username_taken")));

    // Weak input is refused.
    let res = send(&app, "POST", "/api/auth/signup", &[], Some(json!({ "username": "a", "password": "short" }))).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Wrong password and unknown user look the same.
    let res =
        send(&app, "POST", "/api/auth/login", &[], Some(json!({ "username": "kenji", "password": "nope-nope" }))).await;
    assert_eq!((res.status, &res.body["code"]), (StatusCode::UNAUTHORIZED, &json!("invalid_credentials")));
    assert!(res.set_cookie.is_none());
    let res =
        send(&app, "POST", "/api/auth/login", &[], Some(json!({ "username": "ghost", "password": "nope-nope" }))).await;
    assert_eq!((res.status, &res.body["code"]), (StatusCode::UNAUTHORIZED, &json!("invalid_credentials")));

    // The right password opens the inbox.
    let res = send(&app, "POST", "/api/auth/login", &[], Some(creds)).await;
    assert_eq!(res.status, StatusCode::OK);
    let cookie = cookie_pair(&res.set_cookie.unwrap());
    let session = [("cookie", cookie.as_str())];
    let res = send(&app, "GET", "/api/auth/me", &session, None).await;
    assert_eq!((res.status, &res.body["username"]), (StatusCode::OK, &json!("kenji")));
    assert_eq!(send(&app, "GET", "/api/tickets", &session, None).await.status, StatusCode::OK);

    // A bearer token works too (API clients).
    let token = cookie.split_once('=').unwrap().1.to_owned();
    let bearer = format!("Bearer {token}");
    assert_eq!(send(&app, "GET", "/api/tickets", &[("authorization", &bearer)], None).await.status, StatusCode::OK);

    // Logging out kills the session server-side.
    let res = send(&app, "POST", "/api/auth/logout", &session, None).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert!(res.set_cookie.unwrap().contains("Max-Age=0"));
    assert_eq!(send(&app, "GET", "/api/tickets", &session, None).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn gate_redirects_to_login_without_a_session() {
    let app = app().await;
    let res = send(&app, "GET", "/api/auth/gate", &[("x-forwarded-uri", "/inbox/abc")], None).await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.location.as_deref(), Some("/login?next=%2Finbox%2Fabc"));

    // No open redirects.
    let res = send(&app, "GET", "/api/auth/gate", &[("x-forwarded-uri", "//evil.example")], None).await;
    assert_eq!(res.location.as_deref(), Some("/login?next=%2Finbox"));

    let res =
        send(&app, "POST", "/api/auth/signup", &[], Some(json!({ "username": "山田", "password": "パスワード12345" })))
            .await;
    let cookie = cookie_pair(&res.set_cookie.unwrap());
    let res = send(&app, "GET", "/api/auth/gate", &[("cookie", &cookie)], None).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn customer_links_still_work_without_login() {
    let app = app().await;
    let res = send(
        &app,
        "POST",
        "/api/tickets",
        &[],
        Some(json!({ "subject": "Printer on fire", "body": "Again.", "severity": "p1" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let id = res.body["ticket"]["id"].as_str().unwrap().to_owned();
    let token = res.body["access_token"].as_str().unwrap().to_owned();
    let res = send(&app, "GET", &format!("/api/public/tickets/{id}"), &[("x-ticket-token", &token)], None).await;
    assert_eq!(res.status, StatusCode::OK);
    let res = send(
        &app,
        "POST",
        &format!("/api/public/tickets/{id}/messages"),
        &[("x-ticket-token", &token)],
        Some(json!({ "body": "still burning" })),
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
}
