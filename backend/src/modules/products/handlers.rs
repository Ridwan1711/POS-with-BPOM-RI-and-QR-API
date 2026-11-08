//! Product HTTP endpoints.

use std::collections::HashSet;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{
    dto::{CreateProductRequest, IdentifierInput, IdentifierView, ProductDetail, ProductListItem, UpdateProductRequest},
    repository::{self as repo, ProductFilter, ProductRow},
};
use crate::{
    app::AppState,
    audit,
    auth::{Role, StoreContext},
    error::{field_error, validate, AppError, AppResult},
    modules::product_identification::{self as identification, repository as ident_repo},
    pagination::{normalize_page, Paged},
    util::{clean_optional, is_unique_violation},
};

const DEFAULT_UNIT: &str = "pcs";
const REGISTRATION_STATUSES_MANUAL: &[&str] = &["unknown", "reported"];
const PRODUCT_STATUSES: &[&str] = &["active", "inactive", "all"];

#[derive(Debug, Deserialize)]
pub struct ProductListQuery {
    pub q: Option<String>,
    pub category_id: Option<Uuid>,
    /// `active` (default), `inactive`, or `all`.
    pub status: Option<String>,
    pub low_stock: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// `GET /api/v1/products`
pub async fn list(
    ctx: StoreContext,
    State(state): State<AppState>,
    Query(query): Query<ProductListQuery>,
) -> AppResult<Json<Paged<ProductListItem>>> {
    let (limit, offset) = normalize_page(query.limit, query.offset);
    let status = query.status.as_deref().unwrap_or("active");
    if !PRODUCT_STATUSES.contains(&status) {
        return Err(field_error("status", "must be active, inactive or all"));
    }
    let active = match status {
        "active" => Some(true),
        "inactive" => Some(false),
        _ => None,
    };
    let search = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.chars().count() <= 200);

    let filter = ProductFilter {
        search,
        category_id: query.category_id,
        active,
        low_stock_only: query.low_stock.unwrap_or(false),
        limit,
        offset,
    };
    let (rows, total) = repo::list(&state.pool, ctx.store_id, &filter).await?;
    let data = rows.iter().map(ProductListItem::from).collect();
    Ok(Json(Paged::new(data, limit, offset, total)))
}

/// `GET /api/v1/products/{id}`
pub async fn get(
    ctx: StoreContext,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ProductDetail>> {
    Ok(Json(repo::detail(&state.pool, ctx.store_id, id).await?))
}

/// An identifier after classification, ready to store.
#[derive(Debug, Clone)]
pub struct PlannedIdentifier {
    pub scheme: &'static str,
    pub kind: &'static str,
    pub raw_value: String,
    pub value: String,
    pub is_primary: bool,
}

/// Classifies identifiers, rejects duplicates within the request, and ensures one primary.
pub fn plan_identifiers(inputs: &[IdentifierInput]) -> AppResult<Vec<PlannedIdentifier>> {
    let mut planned: Vec<PlannedIdentifier> = Vec::with_capacity(inputs.len());
    let mut seen: HashSet<(&'static str, String)> = HashSet::new();

    for input in inputs {
        let classified = identification::classify_code(&input.code).map_err(|reason| {
            AppError::unprocessable(
                "invalid_identifier",
                format!("{} {reason}", input.code.trim()),
                json!({ "code": input.code.trim() }),
            )
        })?;
        if !seen.insert((classified.scheme, classified.value.clone())) {
            return Err(AppError::unprocessable(
                "duplicate_identifier_in_request",
                format!("The code {} was listed more than once.", classified.raw_value),
                json!({ "code": classified.raw_value }),
            ));
        }
        planned.push(PlannedIdentifier {
            scheme: classified.scheme,
            kind: classified.kind,
            raw_value: classified.raw_value,
            value: classified.value,
            is_primary: input.is_primary,
        });
    }

    let primaries = planned.iter().filter(|p| p.is_primary).count();
    if primaries > 1 {
        return Err(AppError::unprocessable(
            "multiple_primary_identifiers",
            "Only one identifier can be primary.",
            Value::Null,
        ));
    }
    if primaries == 0 {
        if let Some(first) = planned.first_mut() {
            first.is_primary = true;
        }
    }
    Ok(planned)
}

fn map_write_error(err: sqlx::Error) -> AppError {
    if is_unique_violation(&err, "product_identifiers_store_value_key") {
        return AppError::conflict(
            "duplicate_identifier",
            "This barcode or code already belongs to a product in this store.",
        );
    }
    if is_unique_violation(&err, "products_store_sku_key") {
        return AppError::conflict("duplicate_sku", "Another product in this store already uses this SKU.");
    }
    AppError::Database(err)
}

/// Fails with 409 if any identifier is already used by another product in the store.
async fn ensure_identifiers_available(
    conn: &mut PgConnection,
    store_id: Uuid,
    identifiers: &[PlannedIdentifier],
    except_product: Option<Uuid>,
) -> AppResult<()> {
    for identifier in identifiers {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "SELECT product_id FROM product_identifiers
             WHERE store_id = $1 AND scheme = $2 AND value = $3",
        )
        .bind(store_id)
        .bind(identifier.scheme)
        .bind(&identifier.value)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(existing) = owner {
            if Some(existing) != except_product {
                return Err(AppError::Conflict {
                    code: "duplicate_identifier",
                    message: format!(
                        "{} already belongs to another product in this store.",
                        identifier.raw_value
                    ),
                    details: json!({ "existing_product_id": existing }),
                });
            }
        }
    }
    Ok(())
}

async fn ensure_sku_available(
    conn: &mut PgConnection,
    store_id: Uuid,
    sku: &str,
    except_product: Option<Uuid>,
) -> AppResult<()> {
    let owner: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM products WHERE store_id = $1 AND sku = $2")
            .bind(store_id)
            .bind(sku)
            .fetch_optional(&mut *conn)
            .await?;
    match owner {
        Some(existing) if Some(existing) != except_product => Err(AppError::Conflict {
            code: "duplicate_sku",
            message: format!("SKU {sku} is already used by another product in this store."),
            details: json!({ "existing_product_id": existing }),
        }),
        _ => Ok(()),
    }
}

async fn ensure_category(conn: &mut PgConnection, store_id: Uuid, category_id: Option<Uuid>) -> AppResult<()> {
    let Some(category_id) = category_id else {
        return Ok(());
    };
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM categories WHERE id = $1 AND store_id = $2 AND is_active)",
    )
    .bind(category_id)
    .bind(store_id)
    .fetch_one(&mut *conn)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(field_error("category_id", "does not exist in this store"))
    }
}

/// External provenance, resolved from the server-side cache for the product's GTIN.
pub struct Provenance {
    pub code: String,
    pub payload: Value,
    pub confidence: &'static str,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

/// Resolves provenance for a confirmed external source. If the source is disabled, or its
/// cache entry has expired, the product is saved as manual entry. The save is not blocked.
async fn resolve_provenance(
    state: &AppState,
    source_code: Option<&str>,
    identifiers: &[PlannedIdentifier],
) -> AppResult<Option<Provenance>> {
    let Some(code) = source_code.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok(None);
    };
    let Some(gtin) = identifiers.iter().find(|i| i.scheme == "gtin") else {
        return Ok(None);
    };
    if !ident_repo::source_is_enabled(&state.pool, code).await? {
        return Ok(None);
    }
    let Some(cached) = ident_repo::cached_provider(&state.pool, &gtin.value, code).await? else {
        return Ok(None);
    };
    let confidence = match cached.confidence {
        identification::provider::Confidence::Low => "low",
        identification::provider::Confidence::Medium => "medium",
        identification::provider::Confidence::High => "high",
    };
    Ok(Some(Provenance {
        code: cached.code,
        payload: serde_json::to_value(&cached.data).unwrap_or(Value::Null),
        confidence,
        fetched_at: cached.fetched_at,
    }))
}

/// `POST /api/v1/products`
pub async fn create(
    ctx: StoreContext,
    State(state): State<AppState>,
    Json(body): Json<CreateProductRequest>,
) -> AppResult<(StatusCode, Json<ProductDetail>)> {
    ctx.require(Role::Manager)?;
    validate(&body)?;

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(field_error("name", "must not be blank"));
    }
    let unit = clean_optional(body.unit.clone()).unwrap_or_else(|| DEFAULT_UNIT.to_string());
    let sku = clean_optional(body.sku.clone()).map(|s| s.to_uppercase());
    let identifiers = plan_identifiers(&body.identifiers)?;

    let registration_number = clean_optional(body.registration_number.clone());
    let registration_status = match body.registration_status.as_deref() {
        None if registration_number.is_some() => "reported".to_string(),
        None => "unknown".to_string(),
        Some(status) if REGISTRATION_STATUSES_MANUAL.contains(&status) => status.to_string(),
        Some(_) => {
            return Err(field_error(
                "registration_status",
                "can only be 'unknown' or 'reported'. Verified status comes from an official source.",
            ))
        }
    };

    let provenance = resolve_provenance(&state, body.external_source_code.as_deref(), &identifiers).await?;
    let data_source = provenance
        .as_ref()
        .map(|p| p.code.clone())
        .unwrap_or_else(|| "manual".to_string());

    let mut tx = state.pool.begin().await?;
    ensure_category(&mut tx, ctx.store_id, body.category_id).await?;
    ensure_identifiers_available(&mut tx, ctx.store_id, &identifiers, None).await?;
    if let Some(sku) = &sku {
        ensure_sku_available(&mut tx, ctx.store_id, sku, None).await?;
    }

    let product_id: Uuid = sqlx::query_scalar(
        "INSERT INTO products (
            store_id, category_id, name, description, brand, manufacturer, sku, unit,
            purchase_price_idr, selling_price_idr, stock_quantity, low_stock_threshold, image_url,
            bpom_registration_number, registration_status, registration_valid_until,
            data_source, created_by
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 0, $11, $12, $13, $14, $15, $16, $17)
         RETURNING id",
    )
    .bind(ctx.store_id)
    .bind(body.category_id)
    .bind(&name)
    .bind(clean_optional(body.description.clone()))
    .bind(clean_optional(body.brand.clone()))
    .bind(clean_optional(body.manufacturer.clone()))
    .bind(&sku)
    .bind(&unit)
    .bind(body.purchase_price_idr)
    .bind(body.selling_price_idr)
    .bind(body.low_stock_threshold)
    .bind(clean_optional(body.image_url.clone()))
    .bind(&registration_number)
    .bind(&registration_status)
    .bind(body.registration_valid_until)
    .bind(&data_source)
    .bind(ctx.user.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_write_error)?;

    for identifier in &identifiers {
        sqlx::query(
            "INSERT INTO product_identifiers (store_id, product_id, scheme, kind, raw_value, value, is_primary)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(ctx.store_id)
        .bind(product_id)
        .bind(identifier.scheme)
        .bind(identifier.kind)
        .bind(&identifier.raw_value)
        .bind(&identifier.value)
        .bind(identifier.is_primary)
        .execute(&mut *tx)
        .await
        .map_err(map_write_error)?;
    }

    if body.initial_stock > 0 {
        sqlx::query(
            "INSERT INTO inventory_movements
                (store_id, product_id, quantity_delta, quantity_after, reason, created_by)
             VALUES ($1, $2, $3, $3, 'initial_stock', $4)",
        )
        .bind(ctx.store_id)
        .bind(product_id)
        .bind(body.initial_stock)
        .bind(ctx.user.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE products SET stock_quantity = $2 WHERE id = $1")
            .bind(product_id)
            .bind(body.initial_stock)
            .execute(&mut *tx)
            .await?;
    }

    if let Some(p) = &provenance {
        sqlx::query(
            "INSERT INTO product_external_records (store_id, product_id, source_code, confidence, payload, fetched_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(ctx.store_id)
        .bind(product_id)
        .bind(&p.code)
        .bind(p.confidence)
        .bind(&p.payload)
        .bind(p.fetched_at)
        .execute(&mut *tx)
        .await?;
    }

    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "product.create",
        "product",
        Some(product_id.to_string()),
        json!({
            "name": name,
            "identifiers": identifiers.len(),
            "data_source": data_source,
            "initial_stock": body.initial_stock,
        }),
    )
    .await?;
    tx.commit().await?;

    let detail = repo::detail(&state.pool, ctx.store_id, product_id).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

/// `PATCH /api/v1/products/{id}`
///
/// Price, stock and stock-related fields follow the rules in the README. Stock is not editable here.
pub async fn update(
    ctx: StoreContext,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProductRequest>,
) -> AppResult<Json<ProductDetail>> {
    ctx.require(Role::Manager)?;
    validate(&body)?;

    let mut tx = state.pool.begin().await?;
    // Lock first. The joined read below cannot use FOR UPDATE because of the outer joins.
    let locked: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM products WHERE id = $1 AND store_id = $2 FOR UPDATE")
            .bind(id)
            .bind(ctx.store_id)
            .fetch_optional(&mut *tx)
            .await?;
    if locked.is_none() {
        return Err(AppError::NotFound("Product not found.".into()));
    }
    let current: ProductRow = repo::find(&mut *tx, ctx.store_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Product not found.".into()))?;

    let name = match body.name.as_deref() {
        Some(n) => {
            let trimmed = n.trim();
            if trimmed.is_empty() {
                return Err(field_error("name", "must not be blank"));
            }
            trimmed.to_string()
        }
        None => current.name.clone(),
    };
    let description = resolve_optional(body.description, current.description.clone());
    let brand = resolve_optional(body.brand, current.brand.clone());
    let manufacturer = resolve_optional(body.manufacturer, current.manufacturer.clone());
    let image_url = resolve_optional(body.image_url, current.image_url.clone());
    let registration_number = resolve_optional(body.bpom_registration_number, current.bpom_registration_number.clone());
    let category_id = resolve_optional(body.category_id, current.category_id);
    let sku = match body.sku {
        Some(value) => clean_optional(value).map(|s| s.to_uppercase()),
        None => current.sku.clone(),
    };
    let unit = body
        .unit
        .as_deref()
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| current.unit.clone());
    let purchase_price = body.purchase_price_idr.unwrap_or(current.purchase_price_idr);
    let selling_price = body.selling_price_idr.unwrap_or(current.selling_price_idr);
    // The row's `low_stock_threshold` is the effective value; PATCH needs the raw override.
    let current_override: Option<i32> =
        sqlx::query_scalar("SELECT low_stock_threshold FROM products WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let low_stock_threshold = resolve_optional(body.low_stock_threshold, current_override);
    let registration_status = match body.registration_status.as_deref() {
        None => current.registration_status.clone(),
        Some(status) if REGISTRATION_STATUSES_MANUAL.contains(&status) => status.to_string(),
        Some(_) => {
            return Err(field_error(
                "registration_status",
                "can only be 'unknown' or 'reported'. Verified status comes from an official source.",
            ))
        }
    };
    let is_active = body.is_active.unwrap_or(current.is_active);

    ensure_category(&mut tx, ctx.store_id, category_id).await?;
    if let Some(sku) = &sku {
        ensure_sku_available(&mut tx, ctx.store_id, sku, Some(id)).await?;
    }

    sqlx::query(
        "UPDATE products SET
            name = $3, description = $4, brand = $5, manufacturer = $6, sku = $7, unit = $8,
            category_id = $9, purchase_price_idr = $10, selling_price_idr = $11,
            low_stock_threshold = $12, image_url = $13, bpom_registration_number = $14,
            registration_status = $15, is_active = $16, updated_at = now()
         WHERE id = $1 AND store_id = $2",
    )
    .bind(id)
    .bind(ctx.store_id)
    .bind(&name)
    .bind(&description)
    .bind(&brand)
    .bind(&manufacturer)
    .bind(&sku)
    .bind(&unit)
    .bind(category_id)
    .bind(purchase_price)
    .bind(selling_price)
    .bind(low_stock_threshold)
    .bind(&image_url)
    .bind(&registration_number)
    .bind(&registration_status)
    .bind(is_active)
    .execute(&mut *tx)
    .await
    .map_err(map_write_error)?;

    let mut changes = serde_json::Map::new();
    if purchase_price != current.purchase_price_idr {
        changes.insert("purchase_price_idr".into(), json!({ "from": current.purchase_price_idr, "to": purchase_price }));
    }
    if selling_price != current.selling_price_idr {
        changes.insert("selling_price_idr".into(), json!({ "from": current.selling_price_idr, "to": selling_price }));
    }
    if is_active != current.is_active {
        changes.insert("is_active".into(), json!({ "from": current.is_active, "to": is_active }));
    }
    if name != current.name {
        changes.insert("name".into(), json!({ "from": current.name, "to": name }));
    }
    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "product.update",
        "product",
        Some(id.to_string()),
        Value::Object(changes),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(repo::detail(&state.pool, ctx.store_id, id).await?))
}

/// Resolves a double-option PATCH field against the current value.
fn resolve_optional<T>(patch: Option<Option<T>>, current: Option<T>) -> Option<T> {
    match patch {
        Some(value) => value,
        None => current,
    }
}

/// `DELETE /api/v1/products/{id}`
///
/// Soft delete. Past sales keep their product snapshots, and the stock ledger stays intact.
pub async fn deactivate(
    ctx: StoreContext,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    ctx.require(Role::Manager)?;
    let mut tx = state.pool.begin().await?;
    let updated: Option<Uuid> = sqlx::query_scalar(
        "UPDATE products SET is_active = FALSE, updated_at = now()
         WHERE id = $1 AND store_id = $2 AND is_active
         RETURNING id",
    )
    .bind(id)
    .bind(ctx.store_id)
    .fetch_optional(&mut *tx)
    .await?;
    if updated.is_none() {
        return Err(AppError::NotFound("Product not found or already inactive.".into()));
    }
    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "product.deactivate",
        "product",
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/products/{id}/identifiers`
pub async fn add_identifier(
    ctx: StoreContext,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<IdentifierInput>,
) -> AppResult<(StatusCode, Json<IdentifierView>)> {
    ctx.require(Role::Manager)?;
    validate(&body)?;
    let planned = plan_identifiers(std::slice::from_ref(&body))?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::BadRequest("identifier is empty".into()))?;

    let mut tx = state.pool.begin().await?;
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM products WHERE id = $1 AND store_id = $2 FOR UPDATE")
            .bind(id)
            .bind(ctx.store_id)
            .fetch_optional(&mut *tx)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound("Product not found.".into()));
    }
    ensure_identifiers_available(&mut tx, ctx.store_id, std::slice::from_ref(&planned), Some(id)).await?;

    if planned.is_primary {
        sqlx::query("UPDATE product_identifiers SET is_primary = FALSE WHERE product_id = $1 AND is_primary")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let view: IdentifierView = sqlx::query_as(
        "INSERT INTO product_identifiers (store_id, product_id, scheme, kind, raw_value, value, is_primary)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, scheme, kind, raw_value, value, is_primary",
    )
    .bind(ctx.store_id)
    .bind(id)
    .bind(planned.scheme)
    .bind(planned.kind)
    .bind(&planned.raw_value)
    .bind(&planned.value)
    .bind(planned.is_primary)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_write_error)?;

    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "product.identifier_add",
        "product",
        Some(id.to_string()),
        json!({ "scheme": planned.scheme, "value": planned.value }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// `DELETE /api/v1/products/{id}/identifiers/{identifier_id}`
pub async fn remove_identifier(
    ctx: StoreContext,
    State(state): State<AppState>,
    Path((id, identifier_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    ctx.require(Role::Manager)?;
    let mut tx = state.pool.begin().await?;
    let removed: Option<String> = sqlx::query_scalar(
        "DELETE FROM product_identifiers
         WHERE id = $1 AND product_id = $2 AND store_id = $3
         RETURNING value",
    )
    .bind(identifier_id)
    .bind(id)
    .bind(ctx.store_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(value) = removed else {
        return Err(AppError::NotFound("Identifier not found.".into()));
    };
    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "product.identifier_remove",
        "product",
        Some(id.to_string()),
        json!({ "value": value }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
