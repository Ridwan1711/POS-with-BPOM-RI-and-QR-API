//! Read models for sales and receipts.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    util::contains_pattern,
};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SaleSummary {
    pub id: Uuid,
    pub receipt_number: String,
    pub total_idr: i64,
    pub paid_idr: i64,
    pub change_idr: i64,
    pub item_quantity: i64,
    pub payment_methods: Vec<String>,
    pub cashier_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct SalePageRow {
    #[sqlx(flatten)]
    sale: SaleSummary,
    total_count: i64,
    filtered_revenue_idr: i64,
}

pub struct SaleFilter<'a> {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub receipt: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

pub struct SaleListResult {
    pub items: Vec<SaleSummary>,
    pub total: i64,
    pub revenue_idr: i64,
}

/// Lists sales. Date filters are interpreted in the store's time zone.
pub async fn list_sales(pool: &PgPool, store_id: Uuid, filter: &SaleFilter<'_>) -> AppResult<SaleListResult> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "SELECT s.id, s.receipt_number, s.total_idr, s.paid_idr, s.change_idr, s.created_at,
            (SELECT COALESCE(SUM(si.quantity), 0)::BIGINT FROM sale_items si WHERE si.sale_id = s.id) AS item_quantity,
            ARRAY(SELECT pm.method FROM payments pm WHERE pm.sale_id = s.id ORDER BY pm.line_no) AS payment_methods,
            u.full_name AS cashier_name,
            COUNT(*) OVER () AS total_count,
            COALESCE(SUM(s.total_idr) OVER (), 0)::BIGINT AS filtered_revenue_idr
         FROM sales s
         JOIN stores st ON st.id = s.store_id
         LEFT JOIN users u ON u.id = s.cashier_id
         WHERE s.store_id = ",
    );
    qb.push_bind(store_id);

    if let Some(from) = filter.from {
        qb.push(" AND s.created_at >= (")
            .push_bind(from)
            .push("::date::timestamp AT TIME ZONE st.timezone)");
    }
    if let Some(to) = filter.to {
        qb.push(" AND s.created_at < ((")
            .push_bind(to)
            .push("::date + 1)::timestamp AT TIME ZONE st.timezone)");
    }
    if let Some(receipt) = filter.receipt {
        qb.push(" AND s.receipt_number ILIKE ").push_bind(contains_pattern(receipt));
    }

    qb.push(" ORDER BY s.created_at DESC, s.id DESC LIMIT ")
        .push_bind(filter.limit)
        .push(" OFFSET ")
        .push_bind(filter.offset);

    let rows: Vec<SalePageRow> = qb.build_query_as::<SalePageRow>().fetch_all(pool).await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    let revenue_idr = rows.first().map(|r| r.filtered_revenue_idr).unwrap_or(0);
    Ok(SaleListResult {
        items: rows.into_iter().map(|r| r.sale).collect(),
        total,
        revenue_idr,
    })
}

#[derive(Debug, FromRow)]
struct SaleHeaderRow {
    id: Uuid,
    receipt_number: String,
    cashier_name: Option<String>,
    customer_id: Option<Uuid>,
    subtotal_idr: i64,
    item_discount_idr: i64,
    sale_discount_idr: i64,
    tax_rate_bps: i32,
    tax_idr: i64,
    total_idr: i64,
    paid_idr: i64,
    change_idr: i64,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SaleItemView {
    pub product_id: Option<Uuid>,
    pub product_name: String,
    pub sku: Option<String>,
    pub unit: String,
    pub unit_price_idr: i64,
    pub quantity: i32,
    pub discount_idr: i64,
    pub line_total_idr: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PaymentView {
    pub method: String,
    pub amount_idr: i64,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ReceiptStore {
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub receipt_footer: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleView {
    pub id: Uuid,
    pub receipt_number: String,
    pub cashier_name: Option<String>,
    pub customer_id: Option<Uuid>,
    pub subtotal_idr: i64,
    pub item_discount_idr: i64,
    pub sale_discount_idr: i64,
    pub tax_rate_bps: i32,
    pub tax_idr: i64,
    pub total_idr: i64,
    pub paid_idr: i64,
    pub change_idr: i64,
    pub created_at: DateTime<Utc>,
    pub items: Vec<SaleItemView>,
    pub payments: Vec<PaymentView>,
    pub store: ReceiptStore,
}

/// Full receipt for one sale in the given store. Other stores' sales return 404.
pub async fn get_sale(pool: &PgPool, store_id: Uuid, sale_id: Uuid) -> AppResult<SaleView> {
    let header = sqlx::query_as::<_, SaleHeaderRow>(
        "SELECT s.id, s.receipt_number, u.full_name AS cashier_name, s.customer_id,
                s.subtotal_idr, s.item_discount_idr, s.sale_discount_idr, s.tax_rate_bps,
                s.tax_idr, s.total_idr, s.paid_idr, s.change_idr, s.created_at
         FROM sales s
         LEFT JOIN users u ON u.id = s.cashier_id
         WHERE s.id = $1 AND s.store_id = $2",
    )
    .bind(sale_id)
    .bind(store_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Sale not found.".into()))?;

    let items = sqlx::query_as::<_, SaleItemView>(
        "SELECT product_id, product_name, sku, unit, unit_price_idr, quantity, discount_idr, line_total_idr
         FROM sale_items WHERE sale_id = $1 ORDER BY line_no",
    )
    .bind(sale_id)
    .fetch_all(pool)
    .await?;

    let payments = sqlx::query_as::<_, PaymentView>(
        "SELECT method, amount_idr, reference FROM payments WHERE sale_id = $1 ORDER BY line_no",
    )
    .bind(sale_id)
    .fetch_all(pool)
    .await?;

    let store = sqlx::query_as::<_, ReceiptStore>(
        "SELECT name, address, phone, receipt_footer FROM stores WHERE id = $1",
    )
    .bind(store_id)
    .fetch_one(pool)
    .await?;

    Ok(SaleView {
        id: header.id,
        receipt_number: header.receipt_number,
        cashier_name: header.cashier_name,
        customer_id: header.customer_id,
        subtotal_idr: header.subtotal_idr,
        item_discount_idr: header.item_discount_idr,
        sale_discount_idr: header.sale_discount_idr,
        tax_rate_bps: header.tax_rate_bps,
        tax_idr: header.tax_idr,
        total_idr: header.total_idr,
        paid_idr: header.paid_idr,
        change_idr: header.change_idr,
        created_at: header.created_at,
        items,
        payments,
        store,
    })
}
