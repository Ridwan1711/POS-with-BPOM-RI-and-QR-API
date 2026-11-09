//! Provider implementations and the registry that resolves them by code.

pub mod bpom;
pub mod mock;
pub mod open_food_facts;

use std::sync::Arc;

use super::provider::ProductDataProvider;
use crate::config::Config;

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn ProductDataProvider>>,
}

impl ProviderRegistry {
    /// Every provider that has code. Whether each one runs is decided by the
    /// `external_product_sources` table (`enabled`), not by this list.
    pub fn from_config(config: &Config) -> Self {
        Self {
            providers: vec![
                Arc::new(mock::MockProvider),
                Arc::new(bpom::BpomRegistryAdapter::new(config.bpom_api_base_url.clone())),
                Arc::new(open_food_facts::OpenFoodFactsProvider::new(
                    config.off_api_base_url.clone(),
                    config.off_user_agent.clone(),
                )),
            ],
        }
    }

    pub fn get(&self, code: &str) -> Option<Arc<dyn ProductDataProvider>> {
        self.providers.iter().find(|p| p.code() == code).cloned()
    }
}
