use std::{env, fmt::Display, str::FromStr};

/// Runtime configuration, read from environment variables. See `backend/.env.example`.
#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub db_max_connections: u32,
    pub session_ttl_hours: i64,
    /// Set `COOKIE_SECURE=true` when served over HTTPS.
    pub cookie_secure: bool,
    pub cors_allowed_origins: Vec<String>,
    pub login_max_attempts: u32,
    pub login_window_secs: u64,
    pub provider_timeout_ms: u64,
    pub lookup_cache_hit_hours: i64,
    pub lookup_cache_miss_minutes: i64,
    /// Base URL of an approved BPOM RI integration. Unset means the adapter reports "not configured".
    pub bpom_api_base_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("configuration error: {0}")]
pub struct ConfigError(pub String);

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = env::var("DATABASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| ConfigError("DATABASE_URL is required".into()))?;

        Ok(Self {
            database_url,
            bind_addr: env_or("BIND_ADDR", "0.0.0.0:8080"),
            db_max_connections: parse_or("DB_MAX_CONNECTIONS", 10)?,
            session_ttl_hours: parse_or("SESSION_TTL_HOURS", 12)?,
            cookie_secure: parse_or("COOKIE_SECURE", false)?,
            cors_allowed_origins: list_or("CORS_ALLOWED_ORIGINS", "http://localhost:3000"),
            login_max_attempts: parse_or("LOGIN_MAX_ATTEMPTS", 10)?,
            login_window_secs: parse_or("LOGIN_WINDOW_SECS", 900)?,
            provider_timeout_ms: parse_or("PROVIDER_TIMEOUT_MS", 4000)?,
            lookup_cache_hit_hours: parse_or("LOOKUP_CACHE_HIT_HOURS", 168)?,
            lookup_cache_miss_minutes: parse_or("LOOKUP_CACHE_MISS_MINUTES", 60)?,
            bpom_api_base_url: env::var("BPOM_API_BASE_URL")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
        })
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn list_or(key: &str, default: &str) -> Vec<String> {
    env_or(key, default)
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_or<T>(key: &str, default: T) -> Result<T, ConfigError>
where
    T: FromStr,
    T::Err: Display,
{
    match env::var(key) {
        Ok(raw) if !raw.trim().is_empty() => raw
            .trim()
            .parse::<T>()
            .map_err(|e| ConfigError(format!("{key} is invalid: {e}"))),
        _ => Ok(default),
    }
}
