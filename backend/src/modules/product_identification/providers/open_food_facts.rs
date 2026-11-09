//! Open Food Facts (OFF): crowdsourced food product database, licensed under ODbL.
//!
//! Terms and etiquette that shape this adapter (see docs/integrations/open-food-facts.md):
//! - Read limit is 15 requests per minute per IP. This server's IP is shared by all merchants,
//!   so the limit is enforced here as well as in the service layer.
//! - Every request must carry a User-Agent of the form `AppName/Version (ContactEmail)`.
//!   Without `OFF_USER_AGENT` the provider reports `NotConfigured` and sends nothing.
//! - Only network errors, timeouts, HTTP 429 and 5xx are retried, with exponential backoff and jitter.
//!   Other 4xx responses are not retried.
//! - OFF data is crowdsourced. It is returned as untrusted input and sanitized by the service layer.

use std::time::Duration;

use async_trait::async_trait;
use rand::Rng;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    modules::product_identification::provider::{
        Confidence, ExternalProductData, LookupOutcome, ProductDataProvider,
    },
    rate_limit::RateLimiter,
};

pub const PROVIDER_CODE: &str = "open_food_facts";
const PER_MINUTE_LIMIT: u32 = 15;
const MAX_ATTEMPTS: u32 = 3;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const FIELDS: &str = "code,product_name,brands,quantity,ingredients_text,image_front_small_url";

#[derive(Debug, Deserialize)]
struct V3Response {
    product: Option<OffProduct>,
}

#[derive(Debug, Default, Deserialize)]
pub struct OffProduct {
    pub product_name: Option<String>,
    pub brands: Option<String>,
    pub quantity: Option<String>,
    pub ingredients_text: Option<String>,
    pub image_front_small_url: Option<String>,
}

pub struct OpenFoodFactsProvider {
    /// `None` when the User-Agent is missing or invalid. Lookups then report `NotConfigured`.
    client: Option<reqwest::Client>,
    base_url: String,
    limiter: RateLimiter,
}

enum Attempt {
    Done(LookupOutcome),
    Retry(String),
}

impl OpenFoodFactsProvider {
    pub fn new(base_url: String, user_agent: Option<String>) -> Self {
        let client = user_agent.and_then(|ua| {
            reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .user_agent(ua)
                .build()
                .ok()
        });
        Self {
            client,
            base_url,
            limiter: RateLimiter::new(),
        }
    }

    async fn fetch_once(&self, client: &reqwest::Client, code: &str) -> Attempt {
        let url = format!("{}/api/v3/product/{}", self.base_url.trim_end_matches('/'), code);
        let response = match client.get(&url).query(&[("fields", FIELDS)]).send().await {
            Ok(response) => response,
            Err(err) => return Attempt::Retry(format!("network error ({err})")),
        };

        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Attempt::Done(LookupOutcome::NotFound);
        }
        if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            return Attempt::Retry(format!("HTTP {status}"));
        }
        if !status.is_success() {
            return Attempt::Done(LookupOutcome::Unavailable(format!(
                "Open Food Facts rejected the request (HTTP {status})."
            )));
        }
        match response.text().await {
            Ok(body) => Attempt::Done(outcome_from_body(&body)),
            Err(err) => Attempt::Retry(format!("could not read the response ({err})")),
        }
    }
}

/// Turns a successful v3 response body into an outcome. Pure, so it can be unit tested.
pub fn outcome_from_body(body: &str) -> LookupOutcome {
    match serde_json::from_str::<V3Response>(body) {
        Ok(V3Response { product: Some(product) }) => LookupOutcome::Found(map_product(&product)),
        Ok(V3Response { product: None }) => LookupOutcome::NotFound,
        Err(_) => LookupOutcome::Unavailable("Open Food Facts returned an unreadable response.".into()),
    }
}

/// Maps OFF fields onto our product data. Brands are split on commas, and only the first one is kept.
pub fn map_product(product: &OffProduct) -> ExternalProductData {
    let brand = non_empty(&product.brands).and_then(|brands| {
        brands
            .split(',')
            .map(str::trim)
            .find(|part| !part.is_empty())
            .map(str::to_string)
    });

    let mut lines = Vec::new();
    if let Some(quantity) = non_empty(&product.quantity) {
        lines.push(format!("Net weight: {quantity}"));
    }
    if let Some(ingredients) = non_empty(&product.ingredients_text) {
        lines.push(format!("Ingredients: {ingredients}"));
    }

    ExternalProductData {
        name: non_empty(&product.product_name),
        brand,
        manufacturer: None,
        description: (!lines.is_empty()).then(|| lines.join("\n")),
        unit_hint: None,
        image_url: non_empty(&product.image_front_small_url),
        registration: None,
    }
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// OFF keys products by the barcode without padding zeros. Its API normalizes the other forms.
pub fn native_code(canonical_gtin: &str) -> &str {
    let trimmed = canonical_gtin.trim_start_matches('0');
    if trimmed.is_empty() {
        canonical_gtin
    } else {
        trimmed
    }
}

fn backoff(attempt: u32) -> Duration {
    let base_ms = 500u64 * (1u64 << (attempt - 1));
    let jitter_ms = rand::thread_rng().gen_range(0..=250u64);
    Duration::from_millis(base_ms + jitter_ms)
}

#[async_trait]
impl ProductDataProvider for OpenFoodFactsProvider {
    fn code(&self) -> &'static str {
        PROVIDER_CODE
    }

    fn name(&self) -> &'static str {
        "Open Food Facts"
    }

    fn confidence(&self) -> Confidence {
        Confidence::Medium
    }

    async fn lookup_gtin(&self, canonical_gtin: &str) -> LookupOutcome {
        let Some(client) = &self.client else {
            return LookupOutcome::NotConfigured(
                "OFF_USER_AGENT is not set or is invalid. Open Food Facts requires a User-Agent with a contact address.".into(),
            );
        };
        let code = native_code(canonical_gtin);
        let mut last_reason = String::from("no response");

        for attempt in 1..=MAX_ATTEMPTS {
            if self
                .limiter
                .check(PROVIDER_CODE, PER_MINUTE_LIMIT, Duration::from_secs(60))
                .is_err()
            {
                return LookupOutcome::Unavailable(
                    "The Open Food Facts rate limit for this server was reached. Try again in a minute.".into(),
                );
            }
            match self.fetch_once(client, code).await {
                Attempt::Done(outcome) => return outcome,
                Attempt::Retry(reason) => {
                    last_reason = reason;
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(backoff(attempt)).await;
                    }
                }
            }
        }
        LookupOutcome::Unavailable(format!("Open Food Facts did not answer: {last_reason}."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOUND_NUTELLA: &str = r#"{
        "code": "3017620422003",
        "product": {
            "code": "3017620422003",
            "product_name": "Nutella",
            "brands": "Nutella, Ferrero",
            "quantity": "400 g e",
            "ingredients_text": "Sugar, palm oil, hazelnuts 13%, skimmed milk powder 8.7%, cocoa",
            "image_front_small_url": "https://images.openfoodfacts.org/images/products/301/762/042/2003/front_en.small.jpg"
        },
        "status": "success"
    }"#;

    const PARTIAL_BRAND_ONLY: &str = r#"{"product": {"product_name": "  ", "brands": "Acme"}}"#;

    #[test]
    fn strips_padding_zeros_for_the_native_barcode() {
        assert_eq!(native_code("08991002101029"), "8991002101029");
        assert_eq!(native_code("00000096385074"), "96385074");
        assert_eq!(native_code("00089686170726"), "89686170726");
        assert_eq!(native_code("00000000000000"), "00000000000000");
    }

    #[test]
    fn maps_a_found_product_and_keeps_only_the_first_brand() {
        match outcome_from_body(FOUND_NUTELLA) {
            LookupOutcome::Found(data) => {
                assert_eq!(data.name.as_deref(), Some("Nutella"));
                assert_eq!(data.brand.as_deref(), Some("Nutella"));
                assert!(data.manufacturer.is_none(), "OFF does not provide a reliable manufacturer");
                let description = data.description.expect("description");
                assert!(description.starts_with("Net weight: 400 g e"));
                assert!(description.contains("Ingredients: Sugar"));
                assert!(data.image_url.unwrap().starts_with("https://"));
                assert!(data.registration.is_none(), "OFF never provides registration status");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn missing_product_is_not_found() {
        assert!(matches!(outcome_from_body(r#"{"code": "1", "errors": []}"#), LookupOutcome::NotFound));
        assert!(matches!(outcome_from_body(r#"{"product": null}"#), LookupOutcome::NotFound));
    }

    #[test]
    fn unreadable_body_is_unavailable_not_not_found() {
        assert!(matches!(outcome_from_body("<html>oops</html>"), LookupOutcome::Unavailable(_)));
    }

    #[test]
    fn blank_fields_are_dropped_so_partial_data_is_not_invented() {
        match outcome_from_body(PARTIAL_BRAND_ONLY) {
            LookupOutcome::Found(data) => {
                assert!(data.name.is_none());
                assert_eq!(data.brand.as_deref(), Some("Acme"));
                assert!(data.description.is_none());
                assert!(data.image_url.is_none());
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn backoff_grows_exponentially_with_bounded_jitter() {
        let first = backoff(1).as_millis();
        let second = backoff(2).as_millis();
        assert!((500..=750).contains(&first), "first backoff {first}");
        assert!((1000..=1250).contains(&second), "second backoff {second}");
    }

    #[tokio::test]
    async fn without_a_user_agent_nothing_is_sent_and_the_result_is_not_configured() {
        let provider = OpenFoodFactsProvider::new("https://world.openfoodfacts.org".into(), None);
        assert!(matches!(
            provider.lookup_gtin("08991002101029").await,
            LookupOutcome::NotConfigured(_)
        ));
    }
}
