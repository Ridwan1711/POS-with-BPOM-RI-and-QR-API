//! Integration tests against a real PostgreSQL database.
//!
//! Each test gets its own database, created and migrated by `#[sqlx::test]`.
//! Requires `DATABASE_URL` pointing at a role that can create databases.

use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
    Router,
};
use pos_backend::{build_router, seed, AppState, Config};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PASSWORD: &str = "Test-password-123";

fn test_config() -> Config {
    Config {
        database_url: String::new(),
        bind_addr: "127.0.0.1:0".into(),
        db_max_connections: 5,
        session_ttl_hours: 1,
        cookie_secure: false,
        cors_allowed_origins: Vec::new(),
        login_max_attempts: 1000,
        login_window_secs: 60,
        provider_timeout_ms: 2000,
        lookup_cache_hit_hours: 1,
        lookup_cache_miss_minutes: 1,
        bpom_api_base_url: None,
    }
}

fn app(pool: PgPool) -> Router {
    build_router(AppState::new(pool, test_config()))
}

struct Session {
    cookie: String,
}

async fn send(
    router: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    store: Option<Uuid>,
    body: Option<Value>,
) -> (StatusCode, Option<String>, Value) {
    let mut builder = Request::builder().method(method).uri(format!("/api/v1{path}"));
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    if let Some(store) = store {
        builder = builder.header("x-store-id", store.to_string());
    }
    let request = match body {
        Some(json_body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json_body.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = router.clone().oneshot(request).await.expect("router is infallible");
    let status = response.status();
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, set_cookie, value)
}

async fn login(router: &Router, pool: &PgPool, email: &str, _store_id: Uuid) -> Session {
    let (status, set_cookie, _) = send(
        router,
        Method::POST,
        "/auth/login",
        None,
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login for {email}");
    let cookie = set_cookie
        .expect("login sets a cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let _ = pool;
    Session { cookie }
}

/// Store with one owner, one cashier, and one product with `stock` units at `price` IDR.
async fn fixture(pool: &PgPool, email: &str, stock: i32, price: i64) -> (Uuid, Uuid, Uuid) {
    let store_id = seed::create_store(pool, "Toko Uji").await.unwrap();
    let owner = seed::create_user(pool, email, "Owner", PASSWORD).await.unwrap();
    seed::add_member(pool, store_id, owner, pos_backend::auth::Role::Owner).await.unwrap();
    let product_id: Uuid = sqlx::query_scalar(
        "INSERT INTO products (store_id, name, selling_price_idr, stock_quantity, created_by)
         VALUES ($1, 'Uji Produk', $2, $3, $4) RETURNING id",
    )
    .bind(store_id)
    .bind(price)
    .bind(stock)
    .bind(owner)
    .fetch_one(pool)
    .await
    .unwrap();
    (store_id, owner, product_id)
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_checkouts_never_oversell(pool: PgPool) {
    let (store_id, _owner, product_id) = fixture(&pool, "owner@test.local", 10, 1_000).await;
    let router = app(pool.clone());
    let session = login(&router, &pool, "owner@test.local", store_id).await;

    let mut tasks = Vec::new();
    for _ in 0..25 {
        let router = router.clone();
        let cookie = session.cookie.clone();
        tasks.push(tokio::spawn(async move {
            let body = json!({
                "client_request_id": Uuid::new_v4(),
                "items": [{ "product_id": product_id, "quantity": 1 }],
                "payments": [{ "method": "cash", "amount_idr": 1_000 }],
            });
            send(&router, Method::POST, "/sales", Some(&cookie), Some(store_id), Some(body)).await.0
        }));
    }

    let mut created = 0;
    let mut rejected = 0;
    for task in tasks {
        match task.await.unwrap() {
            StatusCode::CREATED => created += 1,
            StatusCode::CONFLICT => rejected += 1,
            other => panic!("unexpected status {other}"),
        }
    }

    assert_eq!(created, 10, "exactly the available stock must sell");
    assert_eq!(rejected, 15);
    let stock: i32 = sqlx::query_scalar("SELECT stock_quantity FROM products WHERE id = $1")
        .bind(product_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stock, 0);
    let movements: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_movements WHERE product_id = $1 AND reason = 'sale'",
    )
    .bind(product_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(movements, 10, "every sold unit has exactly one ledger row");
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_duplicate_barcodes_create_one_product(pool: PgPool) {
    let (store_id, _owner, _product) = fixture(&pool, "owner@test.local", 0, 1_000).await;
    let router = app(pool.clone());
    let session = login(&router, &pool, "owner@test.local", store_id).await;

    let mut tasks = Vec::new();
    for i in 0..5 {
        let router = router.clone();
        let cookie = session.cookie.clone();
        tasks.push(tokio::spawn(async move {
            let body = json!({
                "name": format!("Duplikat {i}"),
                "selling_price_idr": 5_000,
                "identifiers": [{ "code": "8991002101029" }],
            });
            send(&router, Method::POST, "/products", Some(&cookie), Some(store_id), Some(body)).await.0
        }));
    }

    let mut created = 0;
    for task in tasks {
        if task.await.unwrap() == StatusCode::CREATED {
            created += 1;
        }
    }
    assert_eq!(created, 1, "the unique constraint must admit exactly one product per barcode");
    let identifiers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM product_identifiers WHERE store_id = $1 AND value = '08991002101029'",
    )
    .bind(store_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(identifiers, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn products_are_isolated_between_stores(pool: PgPool) {
    let (store_a, _owner_a, _pa) = fixture(&pool, "owner-a@test.local", 5, 1_000).await;
    let (store_b, _owner_b, product_b) = fixture(&pool, "owner-b@test.local", 5, 1_000).await;
    // The owner of store A is not a member of store B.
    let router = app(pool.clone());
    let session_a = login(&router, &pool, "owner-a@test.local", store_a).await;

    // Asking for store B explicitly is refused outright.
    let (status, _, _) = send(
        &router,
        Method::GET,
        &format!("/products/{product_b}"),
        Some(&session_a.cookie),
        Some(store_b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Presenting store A but a product ID from store B returns 404, not the product.
    let (status, _, _) = send(
        &router,
        Method::GET,
        &format!("/products/{product_b}"),
        Some(&session_a.cookie),
        Some(store_a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Checkout with a product from another store is refused and sells nothing.
    let (status, _, body) = send(
        &router,
        Method::POST,
        "/sales",
        Some(&session_a.cookie),
        Some(store_a),
        Some(json!({
            "items": [{ "product_id": product_b, "quantity": 1 }],
            "payments": [{ "method": "cash", "amount_idr": 2_000 }],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let stock_b: i32 = sqlx::query_scalar("SELECT stock_quantity FROM products WHERE id = $1")
        .bind(product_b)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stock_b, 5, "another store's stock must not move");
}

#[sqlx::test(migrations = "./migrations")]
async fn logout_revokes_the_session(pool: PgPool) {
    let (store_id, _owner, _product) = fixture(&pool, "owner@test.local", 0, 1_000).await;
    let router = app(pool.clone());
    let session = login(&router, &pool, "owner@test.local", store_id).await;

    let (status, _, _) = send(&router, Method::GET, "/auth/me", Some(&session.cookie), None, None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, set_cookie, _) = send(&router, Method::POST, "/auth/logout", Some(&session.cookie), None, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(set_cookie.unwrap().contains("Max-Age=0"));

    let (status, _, _) = send(&router, Method::GET, "/auth/me", Some(&session.cookie), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a logged-out token must stop working");
}

#[sqlx::test(migrations = "./migrations")]
async fn unknown_email_and_wrong_password_look_identical(pool: PgPool) {
    let (_store, _owner, _product) = fixture(&pool, "owner@test.local", 0, 1_000).await;
    let router = app(pool.clone());
    let (s1, _, b1) = send(&router, Method::POST, "/auth/login", None, None,
        Some(json!({ "email": "owner@test.local", "password": "wrong-password" }))).await;
    let (s2, _, b2) = send(&router, Method::POST, "/auth/login", None, None,
        Some(json!({ "email": "nobody@test.local", "password": "wrong-password" }))).await;
    assert_eq!(s1, StatusCode::UNAUTHORIZED);
    assert_eq!(s2, StatusCode::UNAUTHORIZED);
    assert_eq!(b1["error"]["message"], b2["error"]["message"]);
}
