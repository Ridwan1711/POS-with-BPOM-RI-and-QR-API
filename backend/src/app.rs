//! Router construction and shared application state.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderName, HeaderValue, Method},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

use crate::{
    auth::handlers as auth,
    config::Config,
    error::AppResult,
    modules::{categories, dashboard, inventory, product_identification::handlers as identification, products, sales},
    rate_limit::RateLimiter,
    settings,
};
use crate::modules::product_identification::providers::ProviderRegistry;

/// Maximum accepted request body. Product and sale payloads are small.
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub providers: Arc<ProviderRegistry>,
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let providers = ProviderRegistry::from_config(&config);
        Self {
            pool,
            config: Arc::new(config),
            providers: Arc::new(providers),
            limiter: Arc::new(RateLimiter::new()),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    let cors = cors_layer(&state.config.cors_allowed_origins);

    let api = Router::new()
        // Authentication
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        // Store settings
        .route("/settings", get(settings::get_settings).patch(settings::update_settings))
        // Categories
        .route("/categories", get(categories::list).post(categories::create))
        .route("/categories/{id}", patch(categories::update))
        // Products
        .route("/products", get(products::handlers::list).post(products::handlers::create))
        .route("/products/identify", post(identification::identify))
        .route("/products/scan", post(identification::scan))
        .route("/products/identifiers/{identifier}", get(identification::get_by_identifier))
        .route(
            "/products/{id}",
            get(products::handlers::get)
                .patch(products::handlers::update)
                .delete(products::handlers::deactivate),
        )
        .route("/products/{id}/identifiers", post(products::handlers::add_identifier))
        .route(
            "/products/{id}/identifiers/{identifier_id}",
            delete(products::handlers::remove_identifier),
        )
        .route("/external-sources", get(identification::list_sources))
        // Inventory
        .route("/inventory", get(inventory::list))
        .route("/inventory/adjustments", post(inventory::adjust))
        .route("/inventory/movements", get(inventory::movements))
        // Sales / POS
        .route("/sales", get(sales::list).post(sales::create))
        .route("/sales/{id}", get(sales::get))
        // Dashboard
        .route("/dashboard/summary", get(dashboard::summary));

    let mut app = Router::new()
        .route("/health", get(health_live))
        .route("/health/ready", get(health_ready))
        .nest("/api/v1", api)
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(TraceLayer::new_for_http());

    if let Some(cors) = cors {
        app = app.layer(cors);
    }

    app.with_state(state)
}

fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    let allowed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect();
    if allowed.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(allowed))
            .allow_credentials(true)
            .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
            .allow_headers([
                header::CONTENT_TYPE,
                header::ACCEPT,
                HeaderName::from_static("x-store-id"),
            ]),
    )
}

async fn health_live() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn health_ready(State(state): State<AppState>) -> AppResult<Json<Value>> {
    sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.pool).await?;
    Ok(Json(json!({ "status": "ready", "database": "ok" })))
}
