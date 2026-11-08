//! Adapter for BPOM RI product registration data.
//!
//! STATUS: not connected. No public, documented BPOM endpoint has been verified for this
//! project, so this adapter does not call any URL. It reports `NotConfigured` or
//! `Unavailable`. It never reports "not found", because that would be false reassurance.
//!
//! To connect a legitimate source, see `docs/integrations/bpom.md`.

use async_trait::async_trait;

use crate::modules::product_identification::provider::{
    Confidence, LookupOutcome, ProductDataProvider,
};

pub struct BpomRegistryAdapter {
    base_url: Option<String>,
}

impl BpomRegistryAdapter {
    pub fn new(base_url: Option<String>) -> Self {
        Self { base_url }
    }
}

#[async_trait]
impl ProductDataProvider for BpomRegistryAdapter {
    fn code(&self) -> &'static str {
        "bpom_registry"
    }

    fn name(&self) -> &'static str {
        "BPOM RI registry (adapter)"
    }

    fn confidence(&self) -> Confidence {
        Confidence::High
    }

    async fn lookup_gtin(&self, _canonical_gtin: &str) -> LookupOutcome {
        match &self.base_url {
            None => LookupOutcome::NotConfigured(
                "No approved BPOM RI endpoint is configured (BPOM_API_BASE_URL is unset).".into(),
            ),
            Some(_) => LookupOutcome::Unavailable(
                "The BPOM RI adapter is configured but not implemented yet.".into(),
            ),
        }
    }
}
