//! Product categories, scoped to a store.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use crate::{
    app::AppState,
    audit,
    auth::{Role, StoreContext},
    error::{field_error, validate, AppError, AppResult},
    pagination::DataResponse,
    util::is_unique_violation,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CategoryView {
    pub id: Uuid,
    pub name: String,
    pub is_active: bool,
    pub product_count: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCategoryRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryRequest {
    pub name: Option<String>,
    pub is_active: Option<bool>,
}

const CATEGORY_SELECT: &str = "SELECT c.id, c.name, c.is_active, c.updated_at,
    (SELECT COUNT(*)::BIGINT FROM products p WHERE p.category_id = c.id AND p.is_active) AS product_count
    FROM categories c";

async fn fetch_one(pool: &sqlx::PgPool, store_id: Uuid, id: Uuid) -> AppResult<CategoryView> {
    sqlx::query_as::<_, CategoryView>(&format!("{CATEGORY_SELECT} WHERE c.id = $1 AND c.store_id = $2"))
        .bind(id)
        .bind(store_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Category not found.".into()))
}

fn duplicate_error(err: sqlx::Error) -> AppError {
    if is_unique_violation(&err, "categories_store_name_key") {
        AppError::conflict("duplicate_category", "A category with this name already exists.")
    } else {
        AppError::Database(err)
    }
}

/// `GET /api/v1/categories`
pub async fn list(
    ctx: StoreContext,
    State(state): State<AppState>,
) -> AppResult<Json<DataResponse<CategoryView>>> {
    let data = sqlx::query_as::<_, CategoryView>(&format!(
        "{CATEGORY_SELECT} WHERE c.store_id = $1 ORDER BY c.is_active DESC, lower(c.name)"
    ))
    .bind(ctx.store_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(DataResponse { data }))
}

/// `POST /api/v1/categories`
pub async fn create(
    ctx: StoreContext,
    State(state): State<AppState>,
    Json(body): Json<CreateCategoryRequest>,
) -> AppResult<(StatusCode, Json<CategoryView>)> {
    ctx.require(Role::Manager)?;
    validate(&body)?;
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(field_error("name", "must not be blank"));
    }

    let mut tx = state.pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO categories (store_id, name) VALUES ($1, $2) RETURNING id",
    )
    .bind(ctx.store_id)
    .bind(&name)
    .fetch_one(&mut *tx)
    .await
    .map_err(duplicate_error)?;
    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "category.create",
        "category",
        Some(id.to_string()),
        json!({ "name": name }),
    )
    .await?;
    tx.commit().await?;

    let view = fetch_one(&state.pool, ctx.store_id, id).await?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// `PATCH /api/v1/categories/{id}`
pub async fn update(
    ctx: StoreContext,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateCategoryRequest>,
) -> AppResult<Json<CategoryView>> {
    ctx.require(Role::Manager)?;
    let mut tx = state.pool.begin().await?;
    let (current_name, current_active): (String, bool) = sqlx::query_as(
        "SELECT name, is_active FROM categories WHERE id = $1 AND store_id = $2 FOR UPDATE",
    )
    .bind(id)
    .bind(ctx.store_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Category not found.".into()))?;

    let name = match body.name {
        Some(n) => {
            let trimmed = n.trim().to_string();
            if trimmed.is_empty() || trimmed.chars().count() > 100 {
                return Err(field_error("name", "must be between 1 and 100 characters"));
            }
            trimmed
        }
        None => current_name.clone(),
    };
    let is_active = body.is_active.unwrap_or(current_active);

    sqlx::query("UPDATE categories SET name = $3, is_active = $4, updated_at = now() WHERE id = $1 AND store_id = $2")
        .bind(id)
        .bind(ctx.store_id)
        .bind(&name)
        .bind(is_active)
        .execute(&mut *tx)
        .await
        .map_err(duplicate_error)?;
    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "category.update",
        "category",
        Some(id.to_string()),
        json!({ "name": { "from": current_name, "to": name }, "is_active": is_active }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(fetch_one(&state.pool, ctx.store_id, id).await?))
}
