use axum::http::HeaderMap;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

pub const SESSION_COOKIE: &str = "pos_session";

/// 256-bit random session token, hex encoded.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Tokens are stored only as SHA-256 digests, so a database leak does not expose live sessions.
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Reads the session token from the `Cookie` header.
pub fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .map(str::trim)
        .find_map(|pair| pair.strip_prefix(SESSION_COOKIE)?.strip_prefix('='))
        .filter(|token| !token.is_empty() && token.len() <= 256)
        .map(str::to_string)
}

/// `Set-Cookie` value for a new session. HttpOnly so scripts cannot read it.
pub fn session_cookie(token: &str, max_age_secs: i64, secure: bool) -> String {
    let mut cookie = format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age_secs}"
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// `Set-Cookie` value that removes the session cookie.
pub fn clear_session_cookie(secure: bool) -> String {
    let mut cookie = format!("{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn extracts_session_cookie_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            HeaderValue::from_static("theme=dark; pos_session=abc123; other=1"),
        );
        assert_eq!(token_from_headers(&headers).as_deref(), Some("abc123"));
    }

    #[test]
    fn ignores_missing_or_empty_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(axum::http::header::COOKIE, HeaderValue::from_static("pos_session="));
        assert_eq!(token_from_headers(&headers), None);
        assert_eq!(token_from_headers(&HeaderMap::new()), None);
    }

    #[test]
    fn tokens_are_random_and_hash_deterministically() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert_eq!(hash_token(&a), hash_token(&a));
        assert_ne!(hash_token(&a), a);
    }
}
