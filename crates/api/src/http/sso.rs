//! Single sign-on with GitHub OAuth (#14).
//!
//! Off unless `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` are set. Flow: `GET
//! /api/auth/github/start` sets a short-lived `state` cookie and redirects to GitHub;
//! GitHub sends the browser back to `GET /api/auth/github/callback`, which checks the
//! state, swaps the code for a token, reads the GitHub user and starts a normal session.
//! GitHub accounts become workers named `<login>@github`, so they can't collide with
//! password accounts. Optional `GITHUB_ALLOWED_USERS` (comma-separated logins) limits
//! who may sign in this way.

use std::sync::LazyLock;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::auth::session_cookie;
use crate::auth::SESSION_TTL_DAYS;
use crate::{AppState, token};

const STATE_COOKIE: &str = "nullbox_oauth_state";

#[derive(Clone)]
pub struct GithubConfig {
    pub client_id: String,
    pub client_secret: String,
    /// `GITHUB_REDIRECT_URL`, default `{PUBLIC_WEB_URL}/api/auth/github/callback`.
    pub redirect_url: Option<String>,
    /// `GITHUB_ALLOWED_USERS`: lowercase logins; empty means any GitHub account.
    pub allowed_users: Vec<String>,
}

impl GithubConfig {
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        Some(Self {
            client_id: var("GITHUB_CLIENT_ID")?,
            client_secret: var("GITHUB_CLIENT_SECRET")?,
            redirect_url: var("GITHUB_REDIRECT_URL"),
            allowed_users: var("GITHUB_ALLOWED_USERS")
                .unwrap_or_default()
                .split(',')
                .map(|u| u.trim().to_lowercase())
                .filter(|u| !u.is_empty())
                .collect(),
        })
    }
}

static GITHUB: LazyLock<Option<GithubConfig>> = LazyLock::new(GithubConfig::from_env);

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("nullbox")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Which sign-in methods the login page should offer.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct AuthProviders {
    pub password: bool,
    pub github: bool,
}

/// `GET /api/auth/providers`
pub async fn providers() -> Json<AuthProviders> {
    Json(AuthProviders { password: true, github: GITHUB.is_some() })
}

fn redirect(location: &str, cookie: Option<HeaderValue>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::LOCATION, HeaderValue::from_str(location).unwrap_or(HeaderValue::from_static("/login")));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Some(cookie) = cookie {
        headers.append(header::SET_COOKIE, cookie);
    }
    (StatusCode::FOUND, headers).into_response()
}

fn state_cookie(value: &str, max_age: i64) -> HeaderValue {
    // Lax, so the cookie comes along on GitHub's top-level redirect back to us.
    HeaderValue::from_str(&format!(
        "{STATE_COOKIE}={value}; Path=/api/auth/github; Max-Age={max_age}; HttpOnly; Secure; SameSite=Lax"
    ))
    .expect("cookie is ASCII")
}

fn callback_url(state: &AppState, gh: &GithubConfig) -> String {
    gh.redirect_url.clone().unwrap_or_else(|| format!("{}/api/auth/github/callback", state.config.public_web_url))
}

/// The GitHub authorize URL for a given state.
pub fn authorize_url(client_id: &str, redirect_uri: &str, state: &str) -> String {
    let enc = |s: &str| percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string();
    format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read%3Auser&state={}&allow_signup=false",
        enc(client_id),
        enc(redirect_uri),
        enc(state)
    )
}

/// `GET /api/auth/github/start`
pub async fn start(State(state): State<AppState>) -> Response {
    let Some(gh) = GITHUB.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(nonce) = token::generate() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    redirect(&authorize_url(&gh.client_id, &callback_url(&state, gh), &nonce), Some(state_cookie(&nonce, 600)))
}

#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
}

#[derive(Deserialize)]
struct GithubUser {
    id: i64,
    login: String,
}

fn cookie_value(parts: &Parts, name: &str) -> Option<String> {
    parts
        .headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.trim().to_owned())
}

/// `GET /api/auth/github/callback`
pub async fn callback(State(state): State<AppState>, Query(q): Query<Callback>, parts: Parts) -> Response {
    let Some(gh) = GITHUB.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let fail = |why: &str| {
        tracing::warn!(reason = why, "GitHub sign-in refused");
        redirect("/login?sso=failed", Some(state_cookie("", 0)))
    };
    let expected = cookie_value(&parts, STATE_COOKIE);
    let (Some(code), Some(given), Some(expected)) = (q.code, q.state, expected) else {
        return fail("missing code or state");
    };
    if expected.is_empty() || token::hash(&given) != token::hash(&expected) {
        return fail("state mismatch");
    }

    let token = HTTP
        .post("https://github.com/login/oauth/access_token")
        .header(header::ACCEPT, "application/json")
        .form(&[
            ("client_id", gh.client_id.as_str()),
            ("client_secret", gh.client_secret.as_str()),
            ("code", code.as_str()),
            ("redirect_uri", callback_url(&state, gh).as_str()),
        ])
        .send()
        .await;
    let access_token = match token {
        Ok(res) => res.json::<TokenResponse>().await.ok().and_then(|t| t.access_token),
        Err(_) => None,
    };
    let Some(access_token) = access_token else {
        return fail("code exchange failed");
    };
    let user = HTTP
        .get("https://api.github.com/user")
        .header(header::ACCEPT, "application/vnd.github+json")
        .bearer_auth(access_token)
        .send()
        .await;
    let user = match user {
        Ok(res) if res.status().is_success() => res.json::<GithubUser>().await.ok(),
        _ => None,
    };
    let Some(user) = user else {
        return fail("reading the GitHub user failed");
    };
    if !gh.allowed_users.is_empty() && !gh.allowed_users.contains(&user.login.to_lowercase()) {
        return fail("GitHub user not in GITHUB_ALLOWED_USERS");
    }

    match state.auth.sso_sign_in("github", &user.id.to_string(), &format!("{}@github", user.login)).await {
        Ok((user, session)) => {
            tracing::info!(username = %user.username, "worker signed in with GitHub");
            let mut res = redirect("/inbox", Some(state_cookie("", 0)));
            res.headers_mut().append(header::SET_COOKIE, session_cookie(&session, SESSION_TTL_DAYS * 24 * 60 * 60));
            res
        }
        Err(_) => fail("creating the session failed"),
    }
}
