//! Request extractors.
//!
//! - `CurrentUser` resolves the session cookie to an active user.
//! - `StoreContext` additionally resolves the store (from `X-Store-Id`, or the user's
//!   first membership) and the user's role in it. Every business handler takes this,
//!   so tenant isolation is enforced in one place.

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, HeaderName},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::session;
use crate::{
    app::AppState,
    error::{AppError, AppResult},
};

pub static STORE_HEADER: HeaderName = HeaderName::from_static("x-store-id");

/// Ordered from least to most privileged: `cashier < manager < owner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Cashier,
    Manager,
    Owner,
}

impl Role {
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "cashier" => Some(Self::Cashier),
            "manager" => Some(Self::Manager),
            "owner" => Some(Self::Owner),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cashier => "cashier",
            Self::Manager => "manager",
            Self::Owner => "owner",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
}

#[derive(sqlx::FromRow)]
struct SessionUserRow {
    id: Uuid,
    email: String,
    full_name: String,
}

impl<S> FromRequestParts<S> for CurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app = AppState::from_ref(state);
        let token = session::token_from_headers(&parts.headers)
            .ok_or_else(|| AppError::Unauthorized("Sign in to continue.".into()))?;

        let row = sqlx::query_as::<_, SessionUserRow>(
            "SELECT u.id, u.email, u.full_name
             FROM auth_sessions s
             JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = $1
               AND s.revoked_at IS NULL
               AND s.expires_at > now()
               AND u.is_active",
        )
        .bind(session::hash_token(&token))
        .fetch_optional(&app.pool)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Your session has expired. Sign in again.".into()))?;

        Ok(CurrentUser {
            id: row.id,
            email: row.email,
            full_name: row.full_name,
        })
    }
}

#[derive(Debug, Clone)]
pub struct StoreContext {
    pub user: CurrentUser,
    pub store_id: Uuid,
    pub role: Role,
}

impl StoreContext {
    /// Fails with 403 unless the caller's role is at least `min`.
    pub fn require(&self, min: Role) -> AppResult<()> {
        if self.role >= min {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "This action requires the {} role or higher.",
                min.as_str()
            )))
        }
    }
}

impl<S> FromRequestParts<S> for StoreContext
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app = AppState::from_ref(state);
        let user = CurrentUser::from_request_parts(parts, state).await?;

        let requested: Option<Uuid> = match parts.headers.get(&STORE_HEADER) {
            None => None,
            Some(value) => Some(
                value
                    .to_str()
                    .ok()
                    .and_then(|s| Uuid::parse_str(s.trim()).ok())
                    .ok_or_else(|| AppError::BadRequest("X-Store-Id must be a valid store UUID.".into()))?,
            ),
        };

        let membership = match requested {
            Some(store_id) => {
                sqlx::query_as::<_, (Uuid, String)>(
                    "SELECT store_id, role FROM store_memberships WHERE user_id = $1 AND store_id = $2",
                )
                .bind(user.id)
                .bind(store_id)
                .fetch_optional(&app.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, (Uuid, String)>(
                    "SELECT store_id, role FROM store_memberships
                     WHERE user_id = $1
                     ORDER BY created_at, store_id
                     LIMIT 1",
                )
                .bind(user.id)
                .fetch_optional(&app.pool)
                .await?
            }
        };

        // Same error for "store does not exist" and "not a member": no tenant enumeration.
        let (store_id, role_text) = membership
            .ok_or_else(|| AppError::Forbidden("You do not have access to this store.".into()))?;
        let role = Role::from_db(&role_text)
            .ok_or_else(|| AppError::Internal(format!("unknown role in database: {role_text}")))?;

        Ok(StoreContext { user, store_id, role })
    }
}

#[cfg(test)]
mod tests {
    use super::Role;

    #[test]
    fn roles_are_ordered_by_privilege() {
        assert!(Role::Cashier < Role::Manager);
        assert!(Role::Manager < Role::Owner);
        assert_eq!(Role::from_db("manager"), Some(Role::Manager));
        assert_eq!(Role::from_db("root"), None);
    }
}
