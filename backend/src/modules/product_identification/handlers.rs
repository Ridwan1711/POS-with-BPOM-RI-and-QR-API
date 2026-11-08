//! HTTP endpoints for product identification.

use std::time::Duration;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

use super::{
    parse_scan_input,
    repository::{self as repo, SourceView},
    scan::{ScanInput, ScanKind},
    service::{self, IdentifyResponse, InputSummary},
};
use crate::{
    app::AppState,
    auth::{Role, StoreContext},
    error::{validate, AppError, AppResult},
    modules::products::{dto::{ProductDetail, ProductListItem}, repository as products},
};

const IDENTIFY_LIMIT_PER_MINUTE: u32 = 60;

#[derive(Debug, Deserialize, Validate)]
pub struct CodeRequest {
    #[validate(length(min = 1, max = 2048))]
    pub code: String,
}

/// `POST /api/v1/products/identify`
///
/// Full identification: local products, cache, then external providers. Manager or higher,
/// because the result is used to create products.
pub async fn identify(
    ctx: StoreContext,
    State(state): State<AppState>,
    Json(body): Json<CodeRequest>,
) -> AppResult<Json<IdentifyResponse>> {
    ctx.require(Role::Manager)?;
    validate(&body)?;
    state
        .limiter
        .check(
            &format!("identify:{}", ctx.user.id),
            IDENTIFY_LIMIT_PER_MINUTE,
            Duration::from_secs(60),
        )
        .map_err(|retry| AppError::RateLimited {
            retry_after_secs: retry.as_secs().max(1),
        })?;

    let response = service::identify(&state, &ctx, &body.code).await?;
    Ok(Json(response))
}

#[derive(Debug, Serialize)]
pub struct ScanResponse {
    /// `found`, `not_found`, `invalid_code` or `unsupported_format`.
    pub status: &'static str,
    pub message: String,
    pub input: InputSummary,
    pub product: Option<ProductListItem>,
}

/// `POST /api/v1/products/scan`
///
/// Fast local-only lookup for the POS. Never calls external providers.
pub async fn scan(
    ctx: StoreContext,
    State(state): State<AppState>,
    Json(body): Json<CodeRequest>,
) -> AppResult<Json<ScanResponse>> {
    validate(&body)?;
    let scan = parse_scan_input(&body.code);
    let input = InputSummary::from_scan(&scan);

    let response = match scan.kind {
        ScanKind::Invalid { reason } => ScanResponse {
            status: "invalid_code",
            message: reason,
            input,
            product: None,
        },
        ScanKind::Payload { .. } => ScanResponse {
            status: "unsupported_format",
            message: "This code cannot identify a product.".into(),
            input,
            product: None,
        },
        _ => match find_local(&state, &ctx, &scan).await? {
            Some(product) => ScanResponse {
                status: "found",
                message: "Product found.".into(),
                input,
                product: Some(product),
            },
            None => ScanResponse {
                status: "not_found",
                message: "No product in this store uses this code.".into(),
                input,
                product: None,
            },
        },
    };
    Ok(Json(response))
}

/// `GET /api/v1/products/identifiers/{identifier}`
pub async fn get_by_identifier(
    ctx: StoreContext,
    State(state): State<AppState>,
    Path(identifier): Path<String>,
) -> AppResult<Json<ProductDetail>> {
    let scan = parse_scan_input(&identifier);
    let product_id = repo::find_local_product_id(&state.pool, ctx.store_id, &scan.local_keys())
        .await?
        .ok_or_else(|| AppError::NotFound("No product uses this identifier.".into()))?;
    let detail = products::detail(&state.pool, ctx.store_id, product_id).await?;
    Ok(Json(detail))
}

/// `GET /api/v1/external-sources`
pub async fn list_sources(
    ctx: StoreContext,
    State(state): State<AppState>,
) -> AppResult<Json<crate::pagination::DataResponse<SourceView>>> {
    ctx.require(Role::Manager)?;
    let data = repo::list_sources(&state.pool).await?;
    Ok(Json(crate::pagination::DataResponse { data }))
}

async fn find_local(
    state: &AppState,
    ctx: &StoreContext,
    scan: &ScanInput,
) -> AppResult<Option<ProductListItem>> {
    match repo::find_local_product_id(&state.pool, ctx.store_id, &scan.local_keys()).await? {
        Some(id) => repo::local_product_summary(&state.pool, ctx.store_id, id).await,
        None => Ok(None),
    }
}
