//! Worker accounts and server-side sessions (#14).
//!
//! Passwords are hashed with argon2id (PHC string, per-user salt). A session is a random
//! token handed to the browser in an `HttpOnly; Secure; SameSite=Lax` cookie; the database
//! only stores its SHA-256, like customer access tokens (see [`crate::token`]).

use std::sync::LazyLock;

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rand::TryRng;
use rand::rngs::SysRng;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{ApiError, token};

/// Name of the session cookie.
pub const SESSION_COOKIE: &str = "nullbox_session";
/// How long a session lives.
pub const SESSION_TTL_DAYS: i64 = 7;

/// A signed-in worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub username: String,
}

pub struct AuthService {
    pool: SqlitePool,
    /// `SIGNUP_INVITE_CODE`: when set, sign-up requires it. Unset means open sign-up.
    invite_code: Option<String>,
}

/// Hash compared against when the username doesn't exist, so a wrong username takes as
/// long as a wrong password.
/// Its password is random and thrown away, so nothing matches it.
static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| token::generate().ok().and_then(|random| hash_password(&random).ok()).unwrap_or_default());

fn hash_password(password: &str) -> Result<String, ApiError> {
    let mut salt = [0u8; 16];
    SysRng.try_fill_bytes(&mut salt).map_err(|e| ApiError::WorksOnMyMachine(e.to_string()))?;
    let salt = SaltString::encode_b64(&salt).map_err(|e| ApiError::WorksOnMyMachine(e.to_string()))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| ApiError::WorksOnMyMachine(e.to_string()))
}

fn verify_password(password: &str, phc: &str) -> bool {
    PasswordHash::new(phc).is_ok_and(|hash| Argon2::default().verify_password(password.as_bytes(), &hash).is_ok())
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f).await.map_err(|e| ApiError::WorksOnMyMachine(e.to_string()))
}

fn db_err(e: sqlx::Error) -> ApiError {
    ApiError::from(crate::repo::RepoError::Database(e))
}

/// Usernames: 2–40 letters (any script), digits, `.`, `_` or `-`.
pub fn validate_username(username: &str) -> Result<&str, ApiError> {
    let username = username.trim();
    let len = username.chars().count();
    if !(2..=40).contains(&len) || !username.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        return Err(ApiError::Validation("username must be 2-40 letters, digits, '.', '_' or '-'".into()));
    }
    Ok(username)
}

pub fn validate_password(password: &str) -> Result<(), ApiError> {
    let len = password.chars().count();
    if !(8..=200).contains(&len) {
        return Err(ApiError::Validation("password must be 8-200 characters".into()));
    }
    Ok(())
}

impl AuthService {
    pub fn new(pool: SqlitePool, invite_code: Option<String>) -> Self {
        Self { pool, invite_code: invite_code.filter(|code| !code.trim().is_empty()) }
    }

    /// Reads `SIGNUP_INVITE_CODE` from the environment (unset: open sign-up).
    pub fn from_env(pool: SqlitePool) -> Self {
        Self::new(pool, std::env::var("SIGNUP_INVITE_CODE").ok())
    }

    /// Creates an account and signs it in. Returns the user and a fresh session token.
    pub async fn sign_up(
        &self,
        username: &str,
        password: &str,
        invite_code: Option<&str>,
    ) -> Result<(User, String), ApiError> {
        if let Some(expected) = &self.invite_code {
            let given = invite_code.unwrap_or_default();
            if token::hash(given) != token::hash(expected) {
                return Err(ApiError::Forbidden);
            }
        }
        let username = validate_username(username)?.to_owned();
        validate_password(password)?;
        let password = password.to_owned();
        let hash = blocking(move || hash_password(&password)).await??;

        let user = User { id: Uuid::now_v7().to_string(), username };
        let inserted = sqlx::query("INSERT INTO users (id, username, password_hash, created_at) VALUES (?, ?, ?, ?)")
            .bind(&user.id)
            .bind(&user.username)
            .bind(&hash)
            .bind(ts(Utc::now()))
            .execute(&self.pool)
            .await;
        match inserted {
            Ok(_) => {}
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => return Err(ApiError::UsernameTaken),
            Err(e) => return Err(db_err(e)),
        }
        let session = self.create_session(&user.id).await?;
        Ok((user, session))
    }

    /// Checks username and password. Returns the user and a fresh session token.
    pub async fn log_in(&self, username: &str, password: &str) -> Result<(User, String), ApiError> {
        let row: Option<(String, String, Option<String>)> =
            sqlx::query_as("SELECT id, username, password_hash FROM users WHERE username = ?")
                .bind(username.trim())
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        let (user, phc) = match row {
            Some((id, username, Some(phc))) => (Some(User { id, username }), phc),
            _ => (None, DUMMY_HASH.clone()),
        };
        let password = password.to_owned();
        let ok = blocking(move || verify_password(&password, &phc)).await?;
        match user {
            Some(user) if ok => {
                let session = self.create_session(&user.id).await?;
                Ok((user, session))
            }
            _ => Err(ApiError::InvalidCredentials),
        }
    }

    /// Signs in an external identity (SSO), creating its account on first use.
    /// `username` is only used for a new account (e.g. `octocat@github`).
    pub async fn sso_sign_in(&self, provider: &str, subject: &str, username: &str) -> Result<(User, String), ApiError> {
        let existing: Option<(String, String)> = sqlx::query_as(
            "SELECT u.id, u.username FROM oauth_identities o JOIN users u ON u.id = o.user_id
             WHERE o.provider = ? AND o.subject = ?",
        )
        .bind(provider)
        .bind(subject)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        let user = match existing {
            Some((id, username)) => User { id, username },
            None => {
                let now = ts(Utc::now());
                let mut username = username.to_owned();
                // Renamed accounts can reuse an old login: keep names unique.
                let taken: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
                    .bind(&username)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(db_err)?;
                if taken.is_some() {
                    username = format!("{username}-{subject}");
                }
                let user = User { id: Uuid::now_v7().to_string(), username };
                let mut tx = self.pool.begin().await.map_err(db_err)?;
                sqlx::query("INSERT INTO users (id, username, password_hash, created_at) VALUES (?, ?, NULL, ?)")
                    .bind(&user.id)
                    .bind(&user.username)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(db_err)?;
                sqlx::query(
                    "INSERT INTO oauth_identities (provider, subject, user_id, created_at) VALUES (?, ?, ?, ?)",
                )
                .bind(provider)
                .bind(subject)
                .bind(&user.id)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
                tx.commit().await.map_err(db_err)?;
                user
            }
        };
        let session = self.create_session(&user.id).await?;
        Ok((user, session))
    }

    async fn create_session(&self, user_id: &str) -> Result<String, ApiError> {
        let session = token::generate().map_err(|e| ApiError::WorksOnMyMachine(e.to_string()))?;
        let now = Utc::now();
        sqlx::query("INSERT INTO sessions (token_hash, user_id, created_at, expires_at) VALUES (?, ?, ?, ?)")
            .bind(token::hash(&session))
            .bind(user_id)
            .bind(ts(now))
            .bind(ts(now + Duration::days(SESSION_TTL_DAYS)))
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(session)
    }

    /// The user behind a session token, if the session exists and hasn't expired.
    pub async fn session_user(&self, session: &str) -> Result<Option<User>, ApiError> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT u.id, u.username FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = ? AND s.expires_at > ?",
        )
        .bind(token::hash(session))
        .bind(ts(Utc::now()))
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|(id, username)| User { id, username }))
    }

    /// Ends a session. Unknown tokens are fine.
    pub async fn log_out(&self, session: &str) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ? OR expires_at <= ?")
            .bind(token::hash(session))
            .bind(ts(Utc::now()))
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }
}

/// Fixed-width UTC timestamps, so SQL string comparison orders them correctly.
fn ts(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}
