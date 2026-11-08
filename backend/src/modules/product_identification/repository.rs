//! Persistence for identification: local product matching, external source registry,
//! lookup cache, and lookup history.

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    provider::{Confidence, ExternalProductData, ProviderOutcome, ProviderRecord},
    scan::{ScanInput, ScanKind},
    service::IdentifyResponse,
};
use crate::{
    error::AppResult,
    modules::products::{dto::ProductListItem, repository as products},
};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SourceRow {
    pub code: String,
    pub name: String,
    pub priority: i32,
    pub rate_limit_per_minute: i32,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct SourceView {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub priority: i32,
    pub requires_credentials: bool,
    pub rate_limit_per_minute: i32,
}

pub async fn list_sources(pool: &PgPool) -> AppResult<Vec<SourceView>> {
    Ok(sqlx::query_as::<_, SourceView>(
        "SELECT code, name, description, enabled, priority, requires_credentials, rate_limit_per_minute
         FROM external_product_sources ORDER BY priority, code",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn enabled_sources(pool: &PgPool) -> AppResult<Vec<SourceRow>> {
    Ok(sqlx::query_as::<_, SourceRow>(
        "SELECT code, name, priority, rate_limit_per_minute
         FROM external_product_sources WHERE enabled ORDER BY priority, code",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn source_is_enabled(pool: &PgPool, code: &str) -> AppResult<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM external_product_sources WHERE code = $1 AND enabled)",
    )
    .bind(code)
    .fetch_one(pool)
    .await?)
}

/// Finds an existing product in the store matching any of the `(scheme, value)` keys.
/// Internal codes also match the product SKU (case-insensitive).
pub async fn find_local_product_id(
    pool: &PgPool,
    store_id: Uuid,
    keys: &[(&'static str, String)],
) -> AppResult<Option<Uuid>> {
    if keys.is_empty() {
        return Ok(None);
    }
    let schemes: Vec<String> = keys.iter().map(|(scheme, _)| (*scheme).to_string()).collect();
    let values: Vec<String> = keys.iter().map(|(_, value)| value.clone()).collect();

    let by_identifier: Option<Uuid> = sqlx::query_scalar(
        "SELECT i.product_id
         FROM product_identifiers i
         JOIN UNNEST($2::text[], $3::text[]) AS k(scheme, value)
           ON i.scheme = k.scheme AND i.value = k.value
         WHERE i.store_id = $1
         LIMIT 1",
    )
    .bind(store_id)
    .bind(&schemes)
    .bind(&values)
    .fetch_optional(pool)
    .await?;
    if by_identifier.is_some() {
        return Ok(by_identifier);
    }

    let internal = keys.iter().find(|(scheme, _)| *scheme == "internal");
    if let Some((_, code)) = internal {
        let by_sku: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM products WHERE store_id = $1 AND sku = $2 LIMIT 1",
        )
        .bind(store_id)
        .bind(code.to_uppercase())
        .fetch_optional(pool)
        .await?;
        return Ok(by_sku);
    }
    Ok(None)
}

/// Loads a product summary for an identification result.
pub async fn local_product_summary(
    pool: &PgPool,
    store_id: Uuid,
    product_id: Uuid,
) -> AppResult<Option<ProductListItem>> {
    Ok(products::find(pool, store_id, product_id)
        .await?
        .as_ref()
        .map(ProductListItem::from))
}

#[derive(Debug, sqlx::FromRow)]
struct CacheRow {
    providers: Value,
    fetched_at: DateTime<Utc>,
}

pub struct CachedLookup {
    pub providers: Vec<ProviderRecord>,
    pub fetched_at: DateTime<Utc>,
}

/// Returns an unexpired cache entry. A corrupt entry is treated as a miss.
pub async fn cache_get(pool: &PgPool, canonical_gtin: &str) -> AppResult<Option<CachedLookup>> {
    let row = sqlx::query_as::<_, CacheRow>(
        "SELECT providers, fetched_at FROM product_lookup_cache
         WHERE canonical_gtin = $1 AND expires_at > now()",
    )
    .bind(canonical_gtin)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|row| match serde_json::from_value::<Vec<ProviderRecord>>(row.providers) {
        Ok(providers) => Some(CachedLookup {
            providers,
            fetched_at: row.fetched_at,
        }),
        Err(err) => {
            tracing::warn!(error = %err, "ignoring unreadable lookup cache entry");
            None
        }
    }))
}

pub async fn cache_put(
    pool: &PgPool,
    canonical_gtin: &str,
    outcome: &str,
    providers: &[ProviderRecord],
    ttl: Duration,
) -> AppResult<()> {
    let providers_json = serde_json::to_value(providers)
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    sqlx::query(
        "INSERT INTO product_lookup_cache (canonical_gtin, outcome, providers, fetched_at, expires_at)
         VALUES ($1, $2, $3, now(), $4)
         ON CONFLICT (canonical_gtin) DO UPDATE SET
            outcome = EXCLUDED.outcome,
            providers = EXCLUDED.providers,
            fetched_at = EXCLUDED.fetched_at,
            expires_at = EXCLUDED.expires_at",
    )
    .bind(canonical_gtin)
    .bind(outcome)
    .bind(providers_json)
    .bind(Utc::now() + ttl)
    .execute(pool)
    .await?;
    Ok(())
}

/// Provenance for a product being saved: the data one specific provider returned for this GTIN,
/// taken from the server-side cache. The client's copy of the data is never trusted for provenance.
pub struct CachedProvider {
    pub code: String,
    pub data: ExternalProductData,
    pub confidence: Confidence,
    pub fetched_at: DateTime<Utc>,
}

pub async fn cached_provider(
    pool: &PgPool,
    canonical_gtin: &str,
    source_code: &str,
) -> AppResult<Option<CachedProvider>> {
    let Some(cached) = cache_get(pool, canonical_gtin).await? else {
        return Ok(None);
    };
    Ok(cached
        .providers
        .into_iter()
        .find(|p| p.code == source_code && p.outcome == ProviderOutcome::Found)
        .and_then(|p| {
            p.data.map(|data| CachedProvider {
                code: p.code,
                data,
                confidence: p.confidence,
                fetched_at: cached.fetched_at,
            })
        }))
}

pub async fn record_history(
    pool: &PgPool,
    store_id: Uuid,
    user_id: Uuid,
    scan: &ScanInput,
    response: &IdentifyResponse,
) -> AppResult<()> {
    let (scheme, canonical): (Option<&str>, Option<String>) = match &scan.kind {
        ScanKind::Gtin { gtin, .. } => (Some("gtin"), Some(gtin.canonical.clone())),
        ScanKind::InternalCode => (Some("internal"), Some(scan.raw.clone())),
        ScanKind::Payload { .. } => (Some("qr"), None),
        ScanKind::Invalid { .. } => (None, None),
    };
    sqlx::query(
        "INSERT INTO product_lookup_history
            (store_id, user_id, raw_input, scheme, canonical_value, status, product_id, from_cache)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(store_id)
    .bind(user_id)
    .bind(&scan.raw)
    .bind(scheme)
    .bind(canonical)
    .bind(response.status.as_str())
    .bind(response.existing_product.as_ref().map(|p| p.id))
    .bind(response.from_cache)
    .execute(pool)
    .await?;
    Ok(())
}
