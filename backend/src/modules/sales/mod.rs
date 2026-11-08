//! Point of sale: checkout and transaction history.
//!
//! The backend is the source of truth for prices, discounts, tax, totals and stock.
//! The client sends product IDs, quantities and payments. It never sends prices.

pub mod checkout;
pub mod query;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    app::AppState,
    auth::StoreContext,
    error::{field_error, validate, AppError, AppResult},
    pagination::{normalize_page, Paged},
};
use query::{SaleFilter, SaleSummary, SaleView};

pub const PAYMENT_METHODS: &[&str] = &["cash", "qris", "debit_card", "credit_card", "bank_transfer", "ewallet"];
const MAX_HISTORY_RANGE_DAYS: i64 = 366;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSaleRequest {
    /// Idempotency key. Retrying with the same key returns the original sale (200) instead of charging twice.
    pub client_request_id: Option<Uuid>,
    #[validate(length(min = 1, max = 100))]
    #[validate(nested)]
    pub items: Vec<SaleItemInput>,
    /// Sale-level discount in IDR, applied after item discounts and before tax.
    #[serde(default)]
    #[validate(range(min = 0, max = 2_000_000_000))]
    pub discount_idr: i64,
    #[validate(length(min = 1, max = 5))]
    #[validate(nested)]
    pub payments: Vec<PaymentInput>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct SaleItemInput {
    pub product_id: Uuid,
    #[validate(range(min = 1, max = 10000))]
    pub quantity: i32,
    #[serde(default)]
    #[validate(range(min = 0, max = 2_000_000_000))]
    pub discount_idr: i64,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct PaymentInput {
    #[validate(length(min = 1, max = 30))]
    pub method: String,
    #[validate(range(min = 1, max = 2_000_000_000))]
    pub amount_idr: i64,
    #[validate(length(max = 100))]
    pub reference: Option<String>,
}

/// `POST /api/v1/sales`
pub async fn create(
    ctx: StoreContext,
    State(state): State<AppState>,
    Json(body): Json<CreateSaleRequest>,
) -> AppResult<(StatusCode, Json<SaleView>)> {
    validate(&body)?;

    let mut seen = std::collections::HashSet::new();
    for item in &body.items {
        if !seen.insert(item.product_id) {
            return Err(AppError::unprocessable(
                "duplicate_product_line",
                "Each product can appear only once per sale. Combine the quantities.",
                serde_json::json!({ "product_id": item.product_id }),
            ));
        }
    }
    for payment in &body.payments {
        if !PAYMENT_METHODS.contains(&payment.method.as_str()) {
            return Err(AppError::unprocessable(
                "invalid_payment_method",
                format!("Unknown payment method: {}", payment.method),
                serde_json::json!({ "allowed": PAYMENT_METHODS }),
            ));
        }
    }

    let (view, replayed) = checkout::create_sale(&state, &ctx, body).await?;
    let status = if replayed { StatusCode::OK } else { StatusCode::CREATED };
    Ok((status, Json(view)))
}

#[derive(Debug, Deserialize)]
pub struct SalesQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    /// Receipt number search, e.g. `20261108-000042`.
    pub q: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct SalesSummary {
    pub transaction_count: i64,
    pub revenue_idr: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct SalesPage {
    pub data: Vec<SaleSummary>,
    pub pagination: crate::pagination::PageMeta,
    pub summary: SalesSummary,
}

/// `GET /api/v1/sales`: dates are interpreted in the store's time zone.
pub async fn list(
    ctx: StoreContext,
    State(state): State<AppState>,
    Query(query): Query<SalesQuery>,
) -> AppResult<Json<SalesPage>> {
    let (limit, offset) = normalize_page(query.limit, query.offset);
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if to < from {
            return Err(field_error("to", "must be on or after 'from'"));
        }
        if (to - from).num_days() > MAX_HISTORY_RANGE_DAYS {
            return Err(field_error("to", "must be within 366 days of 'from'"));
        }
    }
    let receipt = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    let filter = SaleFilter {
        from: query.from,
        to: query.to,
        receipt,
        limit,
        offset,
    };
    let result = query::list_sales(&state.pool, ctx.store_id, &filter).await?;
    Ok(Json(SalesPage {
        data: result.items,
        pagination: crate::pagination::PageMeta {
            limit,
            offset,
            total: result.total,
        },
        summary: SalesSummary {
            transaction_count: result.total,
            revenue_idr: result.revenue_idr,
        },
    }))
}

/// `GET /api/v1/sales/{id}`: full receipt.
pub async fn get(
    ctx: StoreContext,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SaleView>> {
    Ok(Json(query::get_sale(&state.pool, ctx.store_id, id).await?))
}

#[allow(dead_code)]
type PagedSales = Paged<SaleSummary>;
