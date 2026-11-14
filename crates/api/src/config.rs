use std::net::SocketAddr;

/// Every setting, read from the environment (plus `.env` in dev). See `.env.example`.
#[derive(Debug, Clone)]
pub struct Config {
    /// `BIND_ADDR`, e.g. `0.0.0.0:3000` in a container.
    pub bind_addr: SocketAddr,
    /// `DATABASE_URL`, e.g. `sqlite://nullbox.db`.
    pub database_url: String,
    /// `CORS_ALLOWED_ORIGINS`, comma-separated. The web app runs on another origin (Vite, Tauri).
    pub cors_allowed_origins: Vec<String>,
    /// `PUBLIC_WEB_URL`: where the web app lives; used to build secret access links.
    pub public_web_url: String,
    /// `ADMIN_TOKEN`: unlocks `GET /api/admin/export`, the CEO's spreadsheet. Unset or empty:
    /// the export is off and always answers 403. There is deliberately no default.
    pub admin_token: Option<String>,
    /// `MCP_TOKEN`: bearer token for the MCP server at `/mcp`. Unset or empty: `/mcp` answers 401.
    pub mcp_token: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid config value for {name}: {reason}")]
pub struct ConfigError {
    pub name: &'static str,
    pub reason: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());

        let bind_addr = var("BIND_ADDR", "127.0.0.1:3000")
            .parse()
            .map_err(|e: std::net::AddrParseError| ConfigError { name: "BIND_ADDR", reason: e.to_string() })?;

        Ok(Self {
            bind_addr,
            database_url: var("DATABASE_URL", "sqlite://nullbox.db"),
            cors_allowed_origins: var("CORS_ALLOWED_ORIGINS", "http://localhost:5173")
                .split(',')
                .map(|origin| origin.trim().to_owned())
                .filter(|origin| !origin.is_empty())
                .collect(),
            public_web_url: var("PUBLIC_WEB_URL", "http://localhost:5173").trim_end_matches('/').to_owned(),
            admin_token: optional_secret("ADMIN_TOKEN"),
            mcp_token: optional_secret("MCP_TOKEN"),
        })
    }
}

/// A secret that has no default: unset, empty or whitespace-only all mean "not configured".
fn optional_secret(name: &str) -> Option<String> {
    non_empty(std::env::var(name).ok())
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::non_empty;

    #[test]
    fn unset_or_blank_secrets_are_not_configured() {
        assert_eq!(non_empty(None), None);
        assert_eq!(non_empty(Some(String::new())), None);
        assert_eq!(non_empty(Some("  \n".into())), None);
        assert_eq!(non_empty(Some(" s3cret \n".into())), Some("s3cret".into()));
    }
}
