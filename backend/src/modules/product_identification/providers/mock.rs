//! Deterministic development catalog. Every value is labelled `[MOCK]` so mock data can
//! never be mistaken for real product information. Disabled by default; enable it in
//! `external_product_sources` for local development and tests only.

use async_trait::async_trait;
use chrono::NaiveDate;

use crate::modules::product_identification::provider::{
    Confidence, ExternalProductData, LookupOutcome, ProductDataProvider, RegistrationInfo,
    RegistrationStatus,
};

/// Simulates a provider outage. Lets tests and demos exercise the "provider unavailable" path.
pub const MOCK_OUTAGE_GTIN: &str = "08990000000006";

pub struct MockProvider;

fn mock_registration(number: &str, valid_until: Option<NaiveDate>) -> RegistrationInfo {
    RegistrationInfo {
        number: Some(number.to_string()),
        status: RegistrationStatus::Reported,
        valid_until,
        verified_at: None,
    }
}

fn catalog_entry(canonical_gtin: &str) -> Option<ExternalProductData> {
    let data = match canonical_gtin {
        // EAN-13
        "08991002101029" => ExternalProductData {
            name: Some("[MOCK] Mi Instan Goreng 85 g".into()),
            brand: Some("Mock Brand Mie".into()),
            manufacturer: Some("PT Mock Mie Indonesia".into()),
            unit_hint: Some("pcs".into()),
            registration: Some(mock_registration("MOCK-REG-0001", None)),
            ..Default::default()
        },
        "08992761111014" => ExternalProductData {
            name: Some("[MOCK] Air Mineral 600 ml".into()),
            brand: Some("Mock Brand Air".into()),
            unit_hint: Some("botol".into()),
            ..Default::default()
        },
        "08999909001114" => ExternalProductData {
            name: Some("[MOCK] Kopi Susu Sachet 25 g".into()),
            brand: Some("Mock Brand Kopi".into()),
            manufacturer: Some("PT Mock Kopi Nusantara".into()),
            registration: Some(mock_registration(
                "MOCK-REG-0003",
                NaiveDate::from_ymd_opt(2030, 12, 31),
            )),
            ..Default::default()
        },
        // Partial: the provider knows the brand but not the product name.
        "08993175000017" => ExternalProductData {
            brand: Some("Mock Brand Partial".into()),
            ..Default::default()
        },
        // EAN-8, expanded to GTIN-14
        "00000096385074" => ExternalProductData {
            name: Some("[MOCK] Permen Mint 10 pcs".into()),
            brand: Some("Mock Brand Permen".into()),
            unit_hint: Some("pcs".into()),
            ..Default::default()
        },
        _ => return None,
    };
    Some(data)
}

#[async_trait]
impl ProductDataProvider for MockProvider {
    fn code(&self) -> &'static str {
        "mock"
    }

    fn name(&self) -> &'static str {
        "Development mock catalog"
    }

    fn confidence(&self) -> Confidence {
        Confidence::Low
    }

    async fn lookup_gtin(&self, canonical_gtin: &str) -> LookupOutcome {
        if canonical_gtin == MOCK_OUTAGE_GTIN {
            return LookupOutcome::Unavailable("Simulated mock provider outage.".into());
        }
        match catalog_entry(canonical_gtin) {
            Some(data) => LookupOutcome::Found(data),
            None => LookupOutcome::NotFound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn known_gtin_is_found_and_labelled_mock() {
        match MockProvider.lookup_gtin("08991002101029").await {
            LookupOutcome::Found(data) => {
                assert!(data.name.unwrap().starts_with("[MOCK]"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn unknown_gtin_is_not_found() {
        assert!(matches!(
            MockProvider.lookup_gtin("00400638133393").await,
            LookupOutcome::NotFound
        ));
    }

    #[tokio::test]
    async fn outage_gtin_is_unavailable_not_not_found() {
        assert!(matches!(
            MockProvider.lookup_gtin(MOCK_OUTAGE_GTIN).await,
            LookupOutcome::Unavailable(_)
        ));
    }
}
