use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgExecutor, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use super::dto::{ExternalRecordView, IdentifierView, ProductDetail, ProductListItem};
use crate::{
    error::{AppError, AppResult},
    modules::product_identification::normalize_gtin,
    util::contains_pattern,
};

/// Columns and joins shared by every product read. Keep in sync with `ProductRow`.
pub const PRODUCT_COLUMNS: &str = "p.id, p.name, p.description, p.brand, p.manufacturer, p.sku, p.unit,
    p.category_id, c.name AS category_name, p.purchase_price_idr, p.selling_price_idr, p.stock_quantity,
    COALESCE(p.low_stock_threshold, s.default_low_stock_threshold) AS low_stock_threshold,
    p.image_url, p.bpom_registration_number, p.registration_status, p.registration_valid_until,
    p.registration_verified_at, p.data_source, p.is_active, p.created_at, p.updated_at,
    (SELECT i.raw_value FROM product_identifiers i WHERE i.product_id = p.id AND i.is_primary LIMIT 1)
        AS primary_barcode";

pub const PRODUCT_FROM: &str = "FROM products p
    JOIN stores s ON s.id = p.store_id
    LEFT JOIN categories c ON c.id = p.category_id";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub brand: Option<String>,
    pub manufacturer: Option<String>,
    pub sku: Option<String>,
    pub unit: String,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub purchase_price_idr: i64,
    pub selling_price_idr: i64,
    pub stock_quantity: i32,
    pub low_stock_threshold: i32,
    pub image_url: Option<String>,
    pub bpom_registration_number: Option<String>,
    pub registration_status: String,
    pub registration_valid_until: Option<NaiveDate>,
    pub registration_verified_at: Option<DateTime<Utc>>,
    pub data_source: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub primary_barcode: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ProductPageRow {
    #[sqlx(flatten)]
    product: ProductRow,
    total_count: i64,
}

pub struct ProductFilter<'a> {
    pub search: Option<&'a str>,
    pub category_id: Option<Uuid>,
    pub active: Option<bool>,
    pub low_stock_only: bool,
    pub limit: i64,
    pub offset: i64,
}

/// Lists products for a store. Returns the page and the total number of matches.
pub async fn list(
    pool: &PgPool,
    store_id: Uuid,
    filter: &ProductFilter<'_>,
) -> AppResult<(Vec<ProductRow>, i64)> {
    let mut qb = QueryBuilder::<Postgres>::new("SELECT ");
    qb.push(PRODUCT_COLUMNS)
        .push(", COUNT(*) OVER () AS total_count ")
        .push(PRODUCT_FROM);
    qb.push(" WHERE p.store_id = ").push_bind(store_id);

    if let Some(active) = filter.active {
        qb.push(" AND p.is_active = ").push_bind(active);
    }
    if let Some(category_id) = filter.category_id {
        qb.push(" AND p.category_id = ").push_bind(category_id);
    }
    if filter.low_stock_only {
        qb.push(" AND p.stock_quantity <= COALESCE(p.low_stock_threshold, s.default_low_stock_threshold)");
    }
    if let Some(term) = filter.search {
        let pattern = contains_pattern(term);
        qb.push(" AND (p.name ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR p.brand ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR p.sku ILIKE ")
            .push_bind(pattern);
        // A numeric search term also matches its GTIN, so typing or scanning a barcode finds the product.
        let digits: String = term.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
        if let Ok(gtin) = normalize_gtin(&digits) {
            qb.push(" OR EXISTS (SELECT 1 FROM product_identifiers pi WHERE pi.product_id = p.id AND pi.scheme = 'gtin' AND pi.value = ")
                .push_bind(gtin.canonical)
                .push(")");
        }
        qb.push(")");
    }

    qb.push(" ORDER BY p.name ASC, p.id ASC LIMIT ")
        .push_bind(filter.limit)
        .push(" OFFSET ")
        .push_bind(filter.offset);

    let rows: Vec<ProductPageRow> = qb.build_query_as::<ProductPageRow>().fetch_all(pool).await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    Ok((rows.into_iter().map(|r| r.product).collect(), total))
}

pub async fn find<'e, E>(exec: E, store_id: Uuid, product_id: Uuid) -> AppResult<Option<ProductRow>>
where
    E: PgExecutor<'e>,
{
    let sql = format!(
        "SELECT {PRODUCT_COLUMNS} {PRODUCT_FROM} WHERE p.id = $1 AND p.store_id = $2"
    );
    Ok(sqlx::query_as::<_, ProductRow>(&sql)
        .bind(product_id)
        .bind(store_id)
        .fetch_optional(exec)
        .await?)
}

pub async fn identifiers<'e, E>(exec: E, product_id: Uuid) -> AppResult<Vec<IdentifierView>>
where
    E: PgExecutor<'e>,
{
    Ok(sqlx::query_as::<_, IdentifierView>(
        "SELECT id, scheme, kind, raw_value, value, is_primary
         FROM product_identifiers
         WHERE product_id = $1
         ORDER BY is_primary DESC, created_at, id",
    )
    .bind(product_id)
    .fetch_all(exec)
    .await?)
}

pub async fn external_records<'e, E>(exec: E, product_id: Uuid) -> AppResult<Vec<ExternalRecordView>>
where
    E: PgExecutor<'e>,
{
    Ok(sqlx::query_as::<_, ExternalRecordView>(
        "SELECT source_code, confidence, payload, fetched_at
         FROM product_external_records
         WHERE product_id = $1
         ORDER BY fetched_at DESC",
    )
    .bind(product_id)
    .fetch_all(exec)
    .await?)
}

/// Full product view for one store. Returns 404 for products of other stores.
pub async fn detail(pool: &PgPool, store_id: Uuid, product_id: Uuid) -> AppResult<ProductDetail> {
    let row = find(pool, store_id, product_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Product not found.".into()))?;
    build_detail(pool, &row).await
}

pub async fn build_detail<'e, E>(exec: E, row: &ProductRow) -> AppResult<ProductDetail>
where
    E: PgExecutor<'e> + Copy,
{
    Ok(ProductDetail {
        summary: ProductListItem::from(row),
        description: row.description.clone(),
        manufacturer: row.manufacturer.clone(),
        image_url: row.image_url.clone(),
        bpom_registration_number: row.bpom_registration_number.clone(),
        registration_valid_until: row.registration_valid_until,
        registration_verified_at: row.registration_verified_at,
        created_at: row.created_at,
        identifiers: identifiers(exec, row.id).await?,
        external_records: external_records(exec, row.id).await?,
    })
}
