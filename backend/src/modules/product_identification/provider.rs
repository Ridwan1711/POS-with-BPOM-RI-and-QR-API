//! Contract every external product data provider implements.
//!
//! Providers return *untrusted* data. The service layer sanitizes it, never writes it
//! over merchant-owned fields (price, stock, SKU, category), and never presents a
//! registration as verified unless an official source said so.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// How much the application should trust a source's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationStatus {
    #[default]
    Unknown,
    /// A registration number or status was reported, but not checked against an official source.
    Reported,
    /// Confirmed against an official registry at `verified_at`.
    Verified,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RegistrationInfo {
    pub number: Option<String>,
    pub status: RegistrationStatus,
    pub valid_until: Option<NaiveDate>,
    pub verified_at: Option<DateTime<Utc>>,
}

/// Product fields a provider supplied. Every field is optional: never invent values.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExternalProductData {
    pub name: Option<String>,
    pub brand: Option<String>,
    pub manufacturer: Option<String>,
    pub description: Option<String>,
    pub unit_hint: Option<String>,
    pub image_url: Option<String>,
    pub registration: Option<RegistrationInfo>,
}

impl ExternalProductData {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.brand.is_none()
            && self.manufacturer.is_none()
            && self.description.is_none()
            && self.unit_hint.is_none()
            && self.image_url.is_none()
            && self.registration.is_none()
    }
}

#[derive(Debug, Clone)]
pub enum LookupOutcome {
    Found(ExternalProductData),
    /// The provider answered and has no record of this GTIN.
    NotFound,
    /// Network error, timeout, rate limit or upstream failure. This is never proof that a product does not exist.
    Unavailable(String),
    /// The provider exists but lacks credentials or an approved endpoint.
    NotConfigured(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOutcome {
    Found,
    NotFound,
    Unavailable,
    NotConfigured,
}

/// One provider's answer for one lookup. Returned to clients and stored in the lookup cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRecord {
    pub code: String,
    pub name: String,
    pub confidence: Confidence,
    pub outcome: ProviderOutcome,
    pub message: Option<String>,
    pub data: Option<ExternalProductData>,
    pub latency_ms: u64,
}

/// Contract for external product data sources. Lookups use the canonical GTIN-14.
#[async_trait]
pub trait ProductDataProvider: Send + Sync {
    /// Stable code. Must match a row in `external_product_sources`.
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn confidence(&self) -> Confidence;
    async fn lookup_gtin(&self, canonical_gtin: &str) -> LookupOutcome;
}
