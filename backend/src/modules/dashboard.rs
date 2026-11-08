//! Dashboard summary for one store day (store time zone).

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::StoreContext,
    error::AppResult,
    modules::{
        products::{dto::ProductListItem, repository as products},
        sales::query::{self as sales_query, SaleFilter, SaleSummary},
    },
};

#[derive(Debug, Deserialize)]
pub struct SummaryQuery {
    /// Store-local date. Defaults to today in the store's time zone.
    pub date: Option<NaiveDate>,
}

#[derive(Debug, Serialize)]
pub struct DaySummary {
    pub transaction_count: i64,
    pub revenue_idr: i64,
    pub average_ticket_idr: i64,
    pub items_sold: i64,
}

#[derive(Debug, Serialize)]
pub struct DailyPoint {
    pub date: NaiveDate,
    pub transaction_count: i64,
    pub revenue_idr: i64,
}

#[derive(Debug, Serialize)]
pub struct SummaryResponse {
    pub timezone: String,
    pub date: NaiveDate,
    pub today: DaySummary,
    pub last_7_days: Vec<DailyPoint>,
    pub low_stock_count: i64,
    pub low_stock_items: Vec<ProductListItem>,
    pub recent_sales: Vec<SaleSummary>,
}

#[derive(sqlx::FromRow)]
struct DayRow {
    transaction_count: i64,
    revenue_idr: i64,
    average_ticket_idr: i64,
    items_sold: i64,
}

#[derive(sqlx::FromRow)]
struct DailyRow {
    day: NaiveDate,
    transaction_count: i64,
    revenue_idr: i64,
}

/// `GET /api/v1/dashboard/summary`
pub async fn summary(
    ctx: StoreContext,
    State(state): State<AppState>,
    Query(query): Query<SummaryQuery>,
) -> AppResult<Json<SummaryResponse>> {
    let (timezone, today): (String, NaiveDate) = sqlx::query_as(
        "SELECT timezone, (now() AT TIME ZONE timezone)::date FROM stores WHERE id = $1",
    )
    .bind(ctx.store_id)
    .fetch_one(&state.pool)
    .await?;
    let date = query.date.unwrap_or(today);

    let day = day_summary(&state.pool, ctx.store_id, date).await?;
    let last_7_days = daily_series(&state.pool, ctx.store_id, date).await?;

    let (low_rows, low_total) = products::list(
        &state.pool,
        ctx.store_id,
        &products::ProductFilter {
            search: None,
            category_id: None,
            active: Some(true),
            low_stock_only: true,
            limit: 10,
            offset: 0,
        },
    )
    .await?;

    let recent = sales_query::list_sales(
        &state.pool,
        ctx.store_id,
        &SaleFilter {
            from: None,
            to: None,
            receipt: None,
            limit: 10,
            offset: 0,
        },
    )
    .await?;

    Ok(Json(SummaryResponse {
        timezone,
        date,
        today: day,
        last_7_days,
        low_stock_count: low_total,
        low_stock_items: low_rows.iter().map(ProductListItem::from).collect(),
        recent_sales: recent.items,
    }))
}

async fn day_summary(pool: &sqlx::PgPool, store_id: Uuid, date: NaiveDate) -> AppResult<DaySummary> {
    let row = sqlx::query_as::<_, DayRow>(
        "WITH bounds AS (
            SELECT ($2::date::timestamp AT TIME ZONE st.timezone) AS start_at,
                   (($2::date + 1)::timestamp AT TIME ZONE st.timezone) AS end_at
            FROM stores st WHERE st.id = $1
         ),
         day_sales AS (
            SELECT s.id, s.total_idr
            FROM sales s, bounds b
            WHERE s.store_id = $1 AND s.created_at >= b.start_at AND s.created_at < b.end_at
         )
         SELECT
            (SELECT COUNT(*)::BIGINT FROM day_sales) AS transaction_count,
            (SELECT COALESCE(SUM(total_idr), 0)::BIGINT FROM day_sales) AS revenue_idr,
            (SELECT COALESCE(ROUND(AVG(total_idr)), 0)::BIGINT FROM day_sales) AS average_ticket_idr,
            (SELECT COALESCE(SUM(si.quantity), 0)::BIGINT FROM sale_items si JOIN day_sales d ON d.id = si.sale_id)
                AS items_sold",
    )
    .bind(store_id)
    .bind(date)
    .fetch_one(pool)
    .await?;
    Ok(DaySummary {
        transaction_count: row.transaction_count,
        revenue_idr: row.revenue_idr,
        average_ticket_idr: row.average_ticket_idr,
        items_sold: row.items_sold,
    })
}

/// Seven store-local days ending on `date`, with zero-filled gaps.
async fn daily_series(pool: &sqlx::PgPool, store_id: Uuid, date: NaiveDate) -> AppResult<Vec<DailyPoint>> {
    let start = date - Duration::days(6);
    let rows = sqlx::query_as::<_, DailyRow>(
        "SELECT (s.created_at AT TIME ZONE st.timezone)::date AS day,
                COUNT(*)::BIGINT AS transaction_count,
                COALESCE(SUM(s.total_idr), 0)::BIGINT AS revenue_idr
         FROM sales s
         JOIN stores st ON st.id = s.store_id
         WHERE s.store_id = $1
           AND s.created_at >= ($2::date::timestamp AT TIME ZONE st.timezone)
           AND s.created_at < (($3::date + 1)::timestamp AT TIME ZONE st.timezone)
         GROUP BY day
         ORDER BY day",
    )
    .bind(store_id)
    .bind(start)
    .bind(date)
    .fetch_all(pool)
    .await?;

    Ok((0..7)
        .map(|offset| {
            let day = start + Duration::days(offset);
            match rows.iter().find(|r| r.day == day) {
                Some(r) => DailyPoint {
                    date: day,
                    transaction_count: r.transaction_count,
                    revenue_idr: r.revenue_idr,
                },
                None => DailyPoint {
                    date: day,
                    transaction_count: 0,
                    revenue_idr: 0,
                },
            }
        })
        .collect())
}
