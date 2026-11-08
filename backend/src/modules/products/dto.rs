use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use super::repository::ProductRow;
use crate::util::double_option;

#[derive(Debug, Clone, Serialize)]
pub struct CategoryRef {
    pub id: Uuid,
    pub name: String,
}

/// Compact product view used in lists, POS search, and identification results.
#[derive(Debug, Clone, Serialize)]
pub struct ProductListItem {
    pub id: Uuid,
    pub name: String,
    pub brand: Option<String>,
    pub sku: Option<String>,
    pub unit: String,
    pub category: Option<CategoryRef>,
    pub primary_barcode: Option<String>,
    pub purchase_price_idr: i64,
    pub selling_price_idr: i64,
    pub stock_quantity: i32,
    pub low_stock_threshold: i32,
    pub is_low_stock: bool,
    pub registration_status: String,
    pub data_source: String,
    pub is_active: bool,
    pub updated_at: DateTime<Utc>,
}

impl From<&ProductRow> for ProductListItem {
    fn from(row: &ProductRow) -> Self {
        Self {
            id: row.id,
            name: row.name.clone(),
            brand: row.brand.clone(),
            sku: row.sku.clone(),
            unit: row.unit.clone(),
            category: match (row.category_id, &row.category_name) {
                (Some(id), Some(name)) => Some(CategoryRef { id, name: name.clone() }),
                _ => None,
            },
            primary_barcode: row.primary_barcode.clone(),
            purchase_price_idr: row.purchase_price_idr,
            selling_price_idr: row.selling_price_idr,
            stock_quantity: row.stock_quantity,
            low_stock_threshold: row.low_stock_threshold,
            is_low_stock: row.stock_quantity <= row.low_stock_threshold,
            registration_status: row.registration_status.clone(),
            data_source: row.data_source.clone(),
            is_active: row.is_active,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct IdentifierView {
    pub id: Uuid,
    pub scheme: String,
    pub kind: String,
    pub raw_value: String,
    pub value: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ExternalRecordView {
    pub source_code: String,
    pub confidence: String,
    pub payload: Value,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProductDetail {
    #[serde(flatten)]
    pub summary: ProductListItem,
    pub description: Option<String>,
    pub manufacturer: Option<String>,
    pub image_url: Option<String>,
    pub bpom_registration_number: Option<String>,
    pub registration_valid_until: Option<NaiveDate>,
    pub registration_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub identifiers: Vec<IdentifierView>,
    /// Provenance of external data this product was confirmed from.
    pub external_records: Vec<ExternalRecordView>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct IdentifierInput {
    /// Any code: GTIN barcode, QR payload, or merchant internal code. Classified server-side.
    #[validate(length(min = 1, max = 2048))]
    pub code: String,
    #[serde(default)]
    pub is_primary: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProductRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    #[validate(length(max = 120))]
    pub brand: Option<String>,
    #[validate(length(max = 120))]
    pub manufacturer: Option<String>,
    pub category_id: Option<Uuid>,
    #[validate(length(max = 64))]
    pub sku: Option<String>,
    #[validate(length(max = 20))]
    pub unit: Option<String>,
    #[serde(default)]
    #[validate(range(min = 0, max = 2_000_000_000))]
    pub purchase_price_idr: i64,
    #[validate(range(min = 1, max = 2_000_000_000))]
    pub selling_price_idr: i64,
    #[serde(default)]
    #[validate(range(min = 0, max = 10_000_000))]
    pub initial_stock: i32,
    #[validate(range(min = 0, max = 10_000_000))]
    pub low_stock_threshold: Option<i32>,
    #[validate(length(max = 2048))]
    pub image_url: Option<String>,
    #[validate(length(max = 100))]
    pub registration_number: Option<String>,
    /// Only `unknown` or `reported`. `verified` comes from an official source, never from a client.
    pub registration_status: Option<String>,
    pub registration_valid_until: Option<NaiveDate>,
    #[serde(default)]
    #[validate(length(max = 10))]
    #[validate(nested)]
    pub identifiers: Vec<IdentifierInput>,
    /// Provider the merchant confirmed the details from, if any. Provenance is taken from the server cache.
    #[validate(length(max = 50))]
    pub external_source_code: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProductRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub brand: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub manufacturer: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub sku: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub category_id: Option<Option<Uuid>>,
    #[validate(length(min = 1, max = 20))]
    pub unit: Option<String>,
    #[validate(range(min = 0, max = 2_000_000_000))]
    pub purchase_price_idr: Option<i64>,
    #[validate(range(min = 1, max = 2_000_000_000))]
    pub selling_price_idr: Option<i64>,
    #[serde(default, deserialize_with = "double_option")]
    pub low_stock_threshold: Option<Option<i32>>,
    #[serde(default, deserialize_with = "double_option")]
    pub image_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub bpom_registration_number: Option<Option<String>>,
    pub registration_status: Option<String>,
    pub is_active: Option<bool>,
}
