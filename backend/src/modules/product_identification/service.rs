//! Identification workflow:
//!
//! 1. Classify the input (invalid / QR payload / internal code / GTIN).
//! 2. Check the store's own products. An existing product is returned, never duplicated.
//! 3. For GTINs, check the lookup cache.
//! 4. Otherwise run every enabled provider in parallel, each with a rate limit and a timeout.
//! 5. Decide the status. A provider failure never counts as "not found".
//! 6. Cache complete answers only, then record the lookup in history.

use std::{sync::Arc, time::Instant};

use chrono::{DateTime, Duration, Utc};
use futures::future::join_all;
use serde::Serialize;
use tokio::time::timeout;

use super::{
    provider::{
        Confidence, ExternalProductData, LookupOutcome, ProductDataProvider, ProviderOutcome,
        ProviderRecord,
    },
    repository::{self as repo, SourceRow},
    sanitize::sanitize,
    scan::{parse_scan_input, ScanInput, ScanKind},
};
use crate::{
    app::AppState,
    auth::StoreContext,
    error::AppResult,
    modules::products::dto::ProductListItem,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationStatus {
    /// The store already has this product. Show it instead of creating a duplicate.
    ExistingProduct,
    /// Full product name supplied by an external source. Merchant must review it.
    Identified,
    /// Some external data, but no product name. Merchant completes the rest.
    PartiallyIdentified,
    /// Providers answered and none has this GTIN. Enter the product manually.
    NotFound,
    /// Providers failed, were rate-limited, or none is enabled. Unknown, not absent.
    ProviderUnavailable,
    /// The barcode failed validation (checksum or length).
    InvalidCode,
    /// A QR payload that cannot be used for identification.
    UnsupportedFormat,
}

impl IdentificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExistingProduct => "existing_product",
            Self::Identified => "identified",
            Self::PartiallyIdentified => "partially_identified",
            Self::NotFound => "not_found",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::InvalidCode => "invalid_code",
            Self::UnsupportedFormat => "unsupported_format",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InputSummary {
    pub raw: String,
    pub scheme: Option<&'static str>,
    pub kind: Option<&'static str>,
    pub canonical: Option<String>,
}

impl InputSummary {
    pub fn from_scan(scan: &ScanInput) -> Self {
        let (scheme, kind, canonical) = match &scan.kind {
            ScanKind::Gtin { gtin, via_qr } => (
                Some(if *via_qr { "qr_gtin" } else { "gtin" }),
                Some(gtin.kind.as_str()),
                Some(gtin.canonical.clone()),
            ),
            ScanKind::InternalCode => (Some("internal"), Some("internal"), None),
            ScanKind::Payload { is_url: true } => (Some("qr"), Some("url"), None),
            ScanKind::Payload { is_url: false } => (Some("qr"), Some("payload"), None),
            ScanKind::Invalid { .. } => (None, None, None),
        };
        Self {
            raw: scan.raw.clone(),
            scheme,
            kind,
            canonical,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRef {
    pub code: String,
    pub name: String,
    pub confidence: Confidence,
}

/// A suggestion for the product form. Only the fields listed in `fields` were supplied.
#[derive(Debug, Clone, Serialize)]
pub struct ProductSuggestion {
    #[serde(flatten)]
    pub data: ExternalProductData,
    pub fields: Vec<&'static str>,
    pub sources: Vec<SourceRef>,
    pub primary_source: String,
    pub confidence: Confidence,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdentifyResponse {
    pub status: IdentificationStatus,
    pub message: String,
    pub input: InputSummary,
    pub existing_product: Option<ProductListItem>,
    pub suggestion: Option<ProductSuggestion>,
    pub providers: Vec<ProviderRecord>,
    pub from_cache: bool,
    pub looked_up_at: DateTime<Utc>,
}

impl IdentifyResponse {
    fn new(input: InputSummary) -> Self {
        Self {
            status: IdentificationStatus::NotFound,
            message: String::new(),
            input,
            existing_product: None,
            suggestion: None,
            providers: Vec::new(),
            from_cache: false,
            looked_up_at: Utc::now(),
        }
    }
}

pub struct Assessment {
    pub status: IdentificationStatus,
    pub message: String,
    pub merged: Option<ExternalProductData>,
}

/// Decides the status from provider answers. Errors never turn into "not found".
pub fn assess(providers: &[ProviderRecord]) -> Assessment {
    if providers.is_empty() {
        return Assessment {
            status: IdentificationStatus::ProviderUnavailable,
            message: "No external product data source is enabled. Enter the product details manually."
                .into(),
            merged: None,
        };
    }

    let found: Vec<&ExternalProductData> = providers
        .iter()
        .filter(|p| p.outcome == ProviderOutcome::Found)
        .filter_map(|p| p.data.as_ref())
        .collect();
    let merged = merge(&found);
    let degraded = providers
        .iter()
        .any(|p| matches!(p.outcome, ProviderOutcome::Unavailable | ProviderOutcome::NotConfigured));

    let (status, message) = match &merged {
        Some(data) if data.name.is_some() => (
            IdentificationStatus::Identified,
            "Product found in external data. Review every field before saving.",
        ),
        Some(_) => (
            IdentificationStatus::PartiallyIdentified,
            "Partial data found. Complete the product name and details manually.",
        ),
        None if degraded => (
            IdentificationStatus::ProviderUnavailable,
            "A product data source could not be reached. This does not mean the product does not exist. Try again later or enter the details manually.",
        ),
        None => (
            IdentificationStatus::NotFound,
            "No external record exists for this barcode. Enter the product details manually.",
        ),
    };

    Assessment {
        status,
        message: message.to_string(),
        merged,
    }
}

/// Field-by-field merge in provider priority order: the first provider to supply a field wins.
fn merge(items: &[&ExternalProductData]) -> Option<ExternalProductData> {
    let mut out = ExternalProductData::default();
    for item in items {
        out.name = out.name.or_else(|| item.name.clone());
        out.brand = out.brand.or_else(|| item.brand.clone());
        out.manufacturer = out.manufacturer.or_else(|| item.manufacturer.clone());
        out.description = out.description.or_else(|| item.description.clone());
        out.unit_hint = out.unit_hint.or_else(|| item.unit_hint.clone());
        out.image_url = out.image_url.or_else(|| item.image_url.clone());
        out.registration = out.registration.or_else(|| item.registration.clone());
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn build_suggestion(
    data: ExternalProductData,
    providers: &[ProviderRecord],
    fetched_at: DateTime<Utc>,
) -> ProductSuggestion {
    let mut fields = Vec::new();
    if data.name.is_some() {
        fields.push("name");
    }
    if data.brand.is_some() {
        fields.push("brand");
    }
    if data.manufacturer.is_some() {
        fields.push("manufacturer");
    }
    if data.description.is_some() {
        fields.push("description");
    }
    if data.unit_hint.is_some() {
        fields.push("unit_hint");
    }
    if data.image_url.is_some() {
        fields.push("image_url");
    }
    if data.registration.is_some() {
        fields.push("registration");
    }

    let sources: Vec<SourceRef> = providers
        .iter()
        .filter(|p| p.outcome == ProviderOutcome::Found)
        .map(|p| SourceRef {
            code: p.code.clone(),
            name: p.name.clone(),
            confidence: p.confidence,
        })
        .collect();
    let confidence = sources
        .iter()
        .map(|s| s.confidence)
        .max()
        .unwrap_or(Confidence::Low);
    let primary_source = sources.first().map(|s| s.code.clone()).unwrap_or_default();

    ProductSuggestion {
        data,
        fields,
        sources,
        primary_source,
        confidence,
        fetched_at,
    }
}

/// Runs one provider with its rate limit and timeout. Every failure mode becomes a record.
async fn run_provider(
    state: &AppState,
    source: &SourceRow,
    provider: Arc<dyn ProductDataProvider>,
    canonical_gtin: &str,
) -> ProviderRecord {
    let started = Instant::now();
    let record = |outcome: ProviderOutcome, message: Option<String>, data: Option<ExternalProductData>| {
        ProviderRecord {
            code: source.code.clone(),
            name: source.name.clone(),
            confidence: provider.confidence(),
            outcome,
            message,
            data,
            latency_ms: started.elapsed().as_millis() as u64,
        }
    };

    let limit_key = format!("provider:{}", source.code);
    let limit = source.rate_limit_per_minute.max(1) as u32;
    if let Err(retry) = state
        .limiter
        .check(&limit_key, limit, std::time::Duration::from_secs(60))
    {
        return record(
            ProviderOutcome::Unavailable,
            Some(format!(
                "Rate limit reached for this source. Retry in {} seconds.",
                retry.as_secs().max(1)
            )),
            None,
        );
    }

    let budget = std::time::Duration::from_millis(state.config.provider_timeout_ms);
    let outcome = match timeout(budget, provider.lookup_gtin(canonical_gtin)).await {
        Ok(outcome) => outcome,
        Err(_) => LookupOutcome::Unavailable("The source did not answer in time.".into()),
    };

    match outcome {
        LookupOutcome::Found(data) => {
            let clean = sanitize(data);
            if clean.is_empty() {
                record(ProviderOutcome::NotFound, None, None)
            } else {
                record(ProviderOutcome::Found, None, Some(clean))
            }
        }
        LookupOutcome::NotFound => record(ProviderOutcome::NotFound, None, None),
        LookupOutcome::Unavailable(message) => {
            record(ProviderOutcome::Unavailable, Some(message), None)
        }
        LookupOutcome::NotConfigured(message) => {
            record(ProviderOutcome::NotConfigured, Some(message), None)
        }
    }
}

async fn run_providers(state: &AppState, canonical_gtin: &str) -> AppResult<Vec<ProviderRecord>> {
    let sources = repo::enabled_sources(&state.pool).await?;
    let mut runs = Vec::new();
    for source in &sources {
        if let Some(provider) = state.providers.get(&source.code) {
            runs.push(run_provider(state, source, provider, canonical_gtin));
        }
    }
    Ok(join_all(runs).await)
}

/// Only complete answers are cached: every provider either found data or confirmed absence.
fn is_cacheable(providers: &[ProviderRecord]) -> bool {
    !providers.is_empty()
        && providers
            .iter()
            .all(|p| matches!(p.outcome, ProviderOutcome::Found | ProviderOutcome::NotFound))
}

async fn external_lookup(
    state: &AppState,
    canonical_gtin: &str,
    response: &mut IdentifyResponse,
) -> AppResult<()> {
    let (providers, fetched_at, from_cache) = match repo::cache_get(&state.pool, canonical_gtin).await? {
        Some(cached) => (cached.providers, cached.fetched_at, true),
        None => {
            let providers = run_providers(state, canonical_gtin).await?;
            let now = Utc::now();
            if is_cacheable(&providers) {
                let assessment = assess(&providers);
                let (outcome, ttl) = match assessment.status {
                    IdentificationStatus::NotFound => (
                        "not_found",
                        Duration::minutes(state.config.lookup_cache_miss_minutes),
                    ),
                    IdentificationStatus::PartiallyIdentified => (
                        "partially_identified",
                        Duration::hours(state.config.lookup_cache_hit_hours),
                    ),
                    _ => (
                        "identified",
                        Duration::hours(state.config.lookup_cache_hit_hours),
                    ),
                };
                if let Err(err) = repo::cache_put(&state.pool, canonical_gtin, outcome, &providers, ttl).await {
                    // A cache write failure must not fail the scan.
                    tracing::warn!(error = %err, "could not write lookup cache");
                }
            }
            (providers, now, false)
        }
    };

    let assessment = assess(&providers);
    response.status = assessment.status;
    response.message = assessment.message;
    response.suggestion = assessment
        .merged
        .map(|data| build_suggestion(data, &providers, fetched_at));
    response.providers = providers;
    response.from_cache = from_cache;
    response.looked_up_at = fetched_at;
    Ok(())
}

/// Full identification workflow for a scanned or typed code.
pub async fn identify(state: &AppState, ctx: &StoreContext, raw: &str) -> AppResult<IdentifyResponse> {
    let scan = parse_scan_input(raw);
    let mut response = IdentifyResponse::new(InputSummary::from_scan(&scan));

    if let ScanKind::Invalid { reason } = &scan.kind {
        response.status = IdentificationStatus::InvalidCode;
        response.message = reason.clone();
    } else if let Some(product_id) =
        repo::find_local_product_id(&state.pool, ctx.store_id, &scan.local_keys()).await?
    {
        response.existing_product = repo::local_product_summary(&state.pool, ctx.store_id, product_id).await?;
        response.status = IdentificationStatus::ExistingProduct;
        response.message = "This code already belongs to a product in your store.".into();
    } else {
        match &scan.kind {
            ScanKind::Payload { is_url: true } => {
                response.status = IdentificationStatus::UnsupportedFormat;
                response.message = "This QR code holds a web link. Links are never opened. Enter the product manually.".into();
            }
            ScanKind::Payload { is_url: false } => {
                response.status = IdentificationStatus::UnsupportedFormat;
                response.message = "This QR format cannot identify a product. Enter the product manually.".into();
            }
            ScanKind::InternalCode => {
                response.status = IdentificationStatus::NotFound;
                response.message = "No product uses this code yet. Create the product and save this code as its identifier.".into();
            }
            ScanKind::Gtin { gtin, .. } => {
                external_lookup(state, &gtin.canonical, &mut response).await?;
            }
            ScanKind::Invalid { .. } => {}
        }
    }

    repo::record_history(&state.pool, ctx.store_id, ctx.user.id, &scan, &response).await?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(code: &str, outcome: ProviderOutcome, data: Option<ExternalProductData>) -> ProviderRecord {
        ProviderRecord {
            code: code.into(),
            name: code.into(),
            confidence: Confidence::Low,
            outcome,
            message: None,
            data,
            latency_ms: 0,
        }
    }

    fn named(name: &str) -> ExternalProductData {
        ExternalProductData {
            name: Some(name.into()),
            ..Default::default()
        }
    }

    #[test]
    fn no_enabled_sources_is_unavailable_not_not_found() {
        assert_eq!(assess(&[]).status, IdentificationStatus::ProviderUnavailable);
    }

    #[test]
    fn all_not_found_is_not_found() {
        let providers = [record("a", ProviderOutcome::NotFound, None)];
        assert_eq!(assess(&providers).status, IdentificationStatus::NotFound);
    }

    #[test]
    fn failure_plus_miss_is_unavailable_not_not_found() {
        let providers = [
            record("a", ProviderOutcome::NotFound, None),
            record("b", ProviderOutcome::Unavailable, None),
        ];
        assert_eq!(assess(&providers).status, IdentificationStatus::ProviderUnavailable);
    }

    #[test]
    fn name_from_any_provider_means_identified() {
        let providers = [
            record("a", ProviderOutcome::Unavailable, None),
            record("b", ProviderOutcome::Found, Some(named("Teh Botol"))),
        ];
        let result = assess(&providers);
        assert_eq!(result.status, IdentificationStatus::Identified);
        assert_eq!(result.merged.unwrap().name.as_deref(), Some("Teh Botol"));
    }

    #[test]
    fn data_without_name_is_partial() {
        let brand_only = ExternalProductData {
            brand: Some("Acme".into()),
            ..Default::default()
        };
        let providers = [record("a", ProviderOutcome::Found, Some(brand_only))];
        assert_eq!(assess(&providers).status, IdentificationStatus::PartiallyIdentified);
    }

    #[test]
    fn earlier_provider_wins_per_field() {
        let first = ExternalProductData {
            name: Some("First".into()),
            ..Default::default()
        };
        let second = ExternalProductData {
            name: Some("Second".into()),
            brand: Some("Brand".into()),
            ..Default::default()
        };
        let providers = [
            record("a", ProviderOutcome::Found, Some(first)),
            record("b", ProviderOutcome::Found, Some(second)),
        ];
        let merged = assess(&providers).merged.unwrap();
        assert_eq!(merged.name.as_deref(), Some("First"));
        assert_eq!(merged.brand.as_deref(), Some("Brand"));
    }

    #[test]
    fn cache_only_stores_complete_answers() {
        assert!(is_cacheable(&[record("a", ProviderOutcome::NotFound, None)]));
        assert!(!is_cacheable(&[
            record("a", ProviderOutcome::NotFound, None),
            record("b", ProviderOutcome::Unavailable, None),
        ]));
        assert!(!is_cacheable(&[]));
    }

    #[test]
    fn suggestion_lists_only_supplied_fields() {
        let data = ExternalProductData {
            name: Some("Teh".into()),
            brand: Some("Acme".into()),
            ..Default::default()
        };
        let providers = [record("mock", ProviderOutcome::Found, Some(data.clone()))];
        let suggestion = build_suggestion(data, &providers, Utc::now());
        assert_eq!(suggestion.fields, vec!["name", "brand"]);
        assert_eq!(suggestion.primary_source, "mock");
    }
}
