//! Inventory: stock levels, manual adjustments, and the movement ledger.
//!
//! Every stock change writes exactly one `inventory_movements` row in the same transaction,
//! with a reason. Product rows are locked with FOR UPDATE, so concurrent adjustments and
//! sales serialize per product and stock cannot drift.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, Postgres, QueryBuilder};
use uuid::Uuid;
use validator::Validate;

use crate::{
    app::AppState,
    audit,
    auth::{Role, StoreContext},
    error::{field_error, validate, AppError, AppResult},
    modules::products::{dto::ProductListItem, repository as products},
    pagination::{normalize_page, Paged},
    settings::load_store,
};

/// Reasons a person may record. `initial_stock` and `sale` are written by product creation and checkout.
const ADJUSTMENT_REASONS: &[&str] = &[
    "restock",
    "return",
    "damage",
    "loss",
    "correction",
    "count_adjustment",
    "other",
];

#[derive(Debug, Deserialize)]
pub struct InventoryQuery {
    pub q: Option<String>,
    pub low_stock: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// `GET /api/v1/inventory`: active products with stock levels.
pub async fn list(
    ctx: StoreContext,
    State(state): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> AppResult<Json<Paged<ProductListItem>>> {
    let (limit, offset) = normalize_page(query.limit, query.offset);
    let search = query.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let filter = products::ProductFilter {
        search,
        category_id: None,
        active: Some(true),
        low_stock_only: query.low_stock.unwrap_or(false),
        limit,
        offset,
    };
    let (rows, total) = products::list(&state.pool, ctx.store_id, &filter).await?;
    Ok(Json(Paged::new(
        rows.iter().map(ProductListItem::from).collect(),
        limit,
        offset,
        total,
    )))
}

#[derive(Debug, Deserialize, Validate)]
pub struct AdjustmentRequest {
    pub product_id: Uuid,
    /// Positive to add stock, negative to remove. Zero is rejected.
    #[validate(range(min = -1_000_000, max = 1_000_000))]
    pub quantity_delta: i32,
    pub reason: String,
    #[validate(length(max = 500))]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdjustmentResponse {
    pub movement: MovementView,
    pub product_id: Uuid,
    pub stock_quantity: i32,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MovementView {
    pub id: Uuid,
    pub product_id: Uuid,
    pub product_name: String,
    pub product_sku: Option<String>,
    pub quantity_delta: i32,
    pub quantity_after: i32,
    pub reason: String,
    pub note: Option<String>,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// `POST /api/v1/inventory/adjustments` (manager or higher)
pub async fn adjust(
    ctx: StoreContext,
    State(state): State<AppState>,
    Json(body): Json<AdjustmentRequest>,
) -> AppResult<(StatusCode, Json<AdjustmentResponse>)> {
    ctx.require(Role::Manager)?;
    validate(&body)?;
    if body.quantity_delta == 0 {
        return Err(field_error("quantity_delta", "must not be zero"));
    }
    if !ADJUSTMENT_REASONS.contains(&body.reason.as_str()) {
        return Err(AppError::unprocessable(
            "invalid_reason",
            "Choose a valid reason for this stock change.",
            json!({ "allowed": ADJUSTMENT_REASONS }),
        ));
    }
    let note = body
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);

    let mut tx = state.pool.begin().await?;
    let (current_stock, is_active): (i32, bool) = sqlx::query_as(
        "SELECT stock_quantity, is_active FROM products WHERE id = $1 AND store_id = $2 FOR UPDATE",
    )
    .bind(body.product_id)
    .bind(ctx.store_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Product not found.".into()))?;
    if !is_active {
        return Err(AppError::unprocessable(
            "product_inactive",
            "This product is inactive. Reactivate it before adjusting stock.",
            json!({ "product_id": body.product_id }),
        ));
    }

    let store = load_store(&mut *tx, ctx.store_id).await?;
    let new_stock = i64::from(current_stock) + i64::from(body.quantity_delta);
    if new_stock < 0 && !store.allow_negative_stock {
        return Err(AppError::Conflict {
            code: "insufficient_stock",
            message: format!("Only {current_stock} in stock. This change would make stock negative."),
            details: json!({ "product_id": body.product_id, "available": current_stock, "requested": -body.quantity_delta }),
        });
    }
    let new_stock = i32::try_from(new_stock)
        .map_err(|_| field_error("quantity_delta", "would exceed the stock limit"))?;

    sqlx::query("UPDATE products SET stock_quantity = $2, updated_at = now() WHERE id = $1")
        .bind(body.product_id)
        .bind(new_stock)
        .execute(&mut *tx)
        .await?;

    let movement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO inventory_movements
            (store_id, product_id, quantity_delta, quantity_after, reason, note, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(ctx.store_id)
    .bind(body.product_id)
    .bind(body.quantity_delta)
    .bind(new_stock)
    .bind(&body.reason)
    .bind(&note)
    .bind(ctx.user.id)
    .fetch_one(&mut *tx)
    .await?;

    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "inventory.adjust",
        "product",
        Some(body.product_id.to_string()),
        json!({
            "quantity_delta": body.quantity_delta,
            "reason": body.reason,
            "stock_before": current_stock,
            "stock_after": new_stock,
        }),
    )
    .await?;
    tx.commit().await?;

    let movement = fetch_movement(&state.pool, ctx.store_id, movement_id).await?;
    Ok((
        StatusCode::CREATED,
        Json(AdjustmentResponse {
            movement,
            product_id: body.product_id,
            stock_quantity: new_stock,
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct MovementQuery {
    pub product_id: Option<Uuid>,
    pub reason: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, FromRow)]
struct MovementPageRow {
    #[sqlx(flatten)]
    movement: MovementView,
    total_count: i64,
}

const MOVEMENT_SELECT: &str = "SELECT m.id, m.product_id, p.name AS product_name, p.sku AS product_sku,
    m.quantity_delta, m.quantity_after, m.reason, m.note, u.full_name AS created_by_name, m.created_at
    FROM inventory_movements m
    JOIN products p ON p.id = m.product_id
    LEFT JOIN users u ON u.id = m.created_by";

async fn fetch_movement(pool: &sqlx::PgPool, store_id: Uuid, id: Uuid) -> AppResult<MovementView> {
    sqlx::query_as::<_, MovementView>(&format!("{MOVEMENT_SELECT} WHERE m.id = $1 AND m.store_id = $2"))
        .bind(id)
        .bind(store_id)
        .fetch_one(pool)
        .await
        .map_err(AppError::from)
}

/// `GET /api/v1/inventory/movements`
pub async fn movements(
    ctx: StoreContext,
    State(state): State<AppState>,
    Query(query): Query<MovementQuery>,
) -> AppResult<Json<Paged<MovementView>>> {
    let (limit, offset) = normalize_page(query.limit, query.offset);
    let mut qb = QueryBuilder::<Postgres>::new(format!("SELECT m.id, m.product_id, p.name AS product_name, p.sku AS product_sku, m.quantity_delta, m.quantity_after, m.reason, m.note, u.full_name AS created_by_name, m.created_at, COUNT(*) OVER () AS total_count FROM inventory_movements m JOIN products p ON p.id = m.product_id LEFT JOIN users u ON u.id = m.created_by WHERE m.store_id = "));
    qb.push_bind(ctx.store_id);
    if let Some(product_id) = query.product_id {
        qb.push(" AND m.product_id = ").push_bind(product_id);
    }
    if let Some(reason) = query.reason.as_deref().filter(|r| !r.is_empty()) {
        qb.push(" AND m.reason = ").push_bind(reason.to_string());
    }
    qb.push(" ORDER BY m.created_at DESC, m.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<MovementPageRow> = qb.build_query_as::<MovementPageRow>().fetch_all(&state.pool).await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    let data = rows.into_iter().map(|r| r.movement).collect();
    Ok(Json(Paged::new(data, limit, offset, total)))
}
