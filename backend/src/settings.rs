//! Store settings: identity, tax, inventory policy and receipt footer.

use axum::{extract::State, Json};
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
    util::{clean_optional, double_option},
};

const STORE_COLUMNS: &str = "id, name, address, phone, timezone, currency_code, tax_rate_bps,
    allow_negative_stock, default_low_stock_threshold, receipt_footer, updated_at";

#[derive(Debug, sqlx::FromRow)]
pub struct StoreRow {
    pub id: Uuid,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub timezone: String,
    pub currency_code: String,
    pub tax_rate_bps: i32,
    pub allow_negative_stock: bool,
    pub default_low_stock_threshold: i32,
    pub receipt_footer: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct StoreView {
    pub id: Uuid,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub timezone: String,
    pub currency_code: String,
    pub tax_rate_bps: i32,
    pub allow_negative_stock: bool,
    pub default_low_stock_threshold: i32,
    pub receipt_footer: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl From<StoreRow> for StoreView {
    fn from(row: StoreRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            address: row.address,
            phone: row.phone,
            timezone: row.timezone,
            currency_code: row.currency_code,
            tax_rate_bps: row.tax_rate_bps,
            allow_negative_stock: row.allow_negative_stock,
            default_low_stock_threshold: row.default_low_stock_threshold,
            receipt_footer: row.receipt_footer,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSettingsRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub address: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub phone: Option<Option<String>>,
    pub timezone: Option<String>,
    /// Tax rate in basis points. 1100 = 11% (PPN). 0 disables tax.
    #[validate(range(min = 0, max = 3000))]
    pub tax_rate_bps: Option<i32>,
    pub allow_negative_stock: Option<bool>,
    #[validate(range(min = 0, max = 100000))]
    pub default_low_stock_threshold: Option<i32>,
    #[serde(default, deserialize_with = "double_option")]
    pub receipt_footer: Option<Option<String>>,
}

pub async fn load_store<'e, E>(exec: E, store_id: Uuid) -> AppResult<StoreRow>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as::<_, StoreRow>(&format!("SELECT {STORE_COLUMNS} FROM stores WHERE id = $1"))
        .bind(store_id)
        .fetch_optional(exec)
        .await?
        .ok_or_else(|| AppError::NotFound("Store not found.".into()))
}

/// `GET /api/v1/settings`
pub async fn get_settings(
    ctx: StoreContext,
    State(state): State<AppState>,
) -> AppResult<Json<StoreView>> {
    let store = load_store(&state.pool, ctx.store_id).await?;
    Ok(Json(store.into()))
}

/// `PATCH /api/v1/settings` (owner only)
pub async fn update_settings(
    ctx: StoreContext,
    State(state): State<AppState>,
    Json(body): Json<UpdateSettingsRequest>,
) -> AppResult<Json<StoreView>> {
    ctx.require(Role::Owner)?;
    validate(&body)?;

    if let Some(tz) = body.timezone.as_deref() {
        let known: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1)")
            .bind(tz)
            .fetch_one(&state.pool)
            .await?;
        if !known {
            return Err(field_error("timezone", "must be a valid IANA time zone, e.g. Asia/Jakarta"));
        }
    }

    let mut tx = state.pool.begin().await?;
    let current = sqlx::query_as::<_, StoreRow>(&format!(
        "SELECT {STORE_COLUMNS} FROM stores WHERE id = $1 FOR UPDATE"
    ))
    .bind(ctx.store_id)
    .fetch_one(&mut *tx)
    .await?;

    let name = match body.name {
        Some(n) => {
            let trimmed = n.trim().to_string();
            if trimmed.is_empty() {
                return Err(field_error("name", "must not be blank"));
            }
            trimmed
        }
        None => current.name.clone(),
    };
    let address = match body.address {
        Some(v) => clean_optional(v),
        None => current.address.clone(),
    };
    let phone = match body.phone {
        Some(v) => clean_optional(v),
        None => current.phone.clone(),
    };
    let receipt_footer = match body.receipt_footer {
        Some(v) => clean_optional(v),
        None => current.receipt_footer.clone(),
    };
    let timezone = body.timezone.unwrap_or_else(|| current.timezone.clone());
    let tax_rate_bps = body.tax_rate_bps.unwrap_or(current.tax_rate_bps);
    let allow_negative_stock = body.allow_negative_stock.unwrap_or(current.allow_negative_stock);
    let default_low_stock_threshold = body
        .default_low_stock_threshold
        .unwrap_or(current.default_low_stock_threshold);

    let updated = sqlx::query_as::<_, StoreRow>(&format!(
        "UPDATE stores SET name = $2, address = $3, phone = $4, timezone = $5, tax_rate_bps = $6,
             allow_negative_stock = $7, default_low_stock_threshold = $8, receipt_footer = $9,
             updated_at = now()
         WHERE id = $1
         RETURNING {STORE_COLUMNS}"
    ))
    .bind(ctx.store_id)
    .bind(&name)
    .bind(&address)
    .bind(&phone)
    .bind(&timezone)
    .bind(tax_rate_bps)
    .bind(allow_negative_stock)
    .bind(default_low_stock_threshold)
    .bind(&receipt_footer)
    .fetch_one(&mut *tx)
    .await?;

    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "settings.update",
        "store",
        Some(ctx.store_id.to_string()),
        json!({
            "tax_rate_bps": { "from": current.tax_rate_bps, "to": tax_rate_bps },
            "allow_negative_stock": { "from": current.allow_negative_stock, "to": allow_negative_stock },
        }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(updated.into()))
}
