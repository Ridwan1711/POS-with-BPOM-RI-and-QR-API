use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use super::{
    context::CurrentUser,
    password::{dummy_hash, verify_password},
    session,
};
use crate::{
    app::AppState,
    audit,
    error::{validate, AppError, AppResult},
};

use std::time::Duration;

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 3, max = 254))]
    pub email: String,
    #[validate(length(min = 1, max = 256))]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserView {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StoreMembershipView {
    pub store_id: Uuid,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub user: UserView,
    pub stores: Vec<StoreMembershipView>,
}

#[derive(sqlx::FromRow)]
struct LoginUserRow {
    id: Uuid,
    email: String,
    full_name: String,
    password_hash: String,
    is_active: bool,
}

/// `POST /api/v1/auth/login`
pub async fn login(
    State(state): State<AppState>,
    Json(mut body): Json<LoginRequest>,
) -> AppResult<Response> {
    body.email = body.email.trim().to_lowercase();
    validate(&body)?;

    let window = Duration::from_secs(state.config.login_window_secs);
    state
        .limiter
        .check(
            &format!("login:{}", body.email),
            state.config.login_max_attempts,
            window,
        )
        .map_err(|retry| AppError::RateLimited {
            retry_after_secs: retry.as_secs().max(1),
        })?;

    let user = sqlx::query_as::<_, LoginUserRow>(
        "SELECT id, email, full_name, password_hash, is_active FROM users WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&state.pool)
    .await?;

    let invalid = || AppError::Unauthorized("Invalid email or password.".into());
    let user = match user {
        Some(row) => {
            let password_ok = verify_password(&body.password, &row.password_hash);
            if !password_ok || !row.is_active {
                return Err(invalid());
            }
            row
        }
        None => {
            let _ = verify_password(&body.password, dummy_hash());
            return Err(invalid());
        }
    };

    let token = session::generate_token();
    let ttl_hours = state.config.session_ttl_hours;
    let mut tx = state.pool.begin().await?;
    sqlx::query("INSERT INTO auth_sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user.id)
        .bind(session::hash_token(&token))
        .bind(Utc::now() + ChronoDuration::hours(ttl_hours))
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        None,
        Some(user.id),
        "auth.login",
        "user",
        Some(user.id.to_string()),
        json!({}),
    )
    .await?;
    tx.commit().await?;

    let stores = load_memberships(&state.pool, user.id).await?;
    let cookie = session::session_cookie(&token, ttl_hours * 3600, state.config.cookie_secure);
    let body = SessionResponse {
        user: UserView {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
        },
        stores,
    };
    Ok(([(SET_COOKIE, cookie)], Json(body)).into_response())
}

/// `POST /api/v1/auth/logout` (idempotent)
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(token) = session::token_from_headers(&headers) {
        sqlx::query(
            "UPDATE auth_sessions SET revoked_at = now()
             WHERE token_hash = $1 AND revoked_at IS NULL",
        )
        .bind(session::hash_token(&token))
        .execute(&state.pool)
        .await?;
    }
    let cookie = session::clear_session_cookie(state.config.cookie_secure);
    Ok(([(SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response())
}

/// `GET /api/v1/auth/me`
pub async fn me(user: CurrentUser, State(state): State<AppState>) -> AppResult<Json<SessionResponse>> {
    let stores = load_memberships(&state.pool, user.id).await?;
    Ok(Json(SessionResponse {
        user: UserView {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
        },
        stores,
    }))
}

async fn load_memberships(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> AppResult<Vec<StoreMembershipView>> {
    let rows = sqlx::query_as::<_, StoreMembershipView>(
        "SELECT s.id AS store_id, s.name, m.role
         FROM store_memberships m
         JOIN stores s ON s.id = m.store_id
         WHERE m.user_id = $1
         ORDER BY m.created_at, s.name",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
