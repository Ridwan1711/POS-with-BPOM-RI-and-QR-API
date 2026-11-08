//! Demo data and test fixtures. Used by `cargo run --bin seed` and by integration tests.
//! Idempotent: running it again on the same database does nothing.

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::{password::hash_password, Role},
    error::AppResult,
};

pub const DEMO_STORE_NAME: &str = "Toko Demo Makmur";

/// Creates a user with a hashed password. Returns the user ID.
pub async fn create_user(pool: &PgPool, email: &str, full_name: &str, password: &str) -> AppResult<Uuid> {
    let hash = hash_password(password)?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, full_name, password_hash) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO UPDATE SET full_name = EXCLUDED.full_name
         RETURNING id",
    )
    .bind(email.to_lowercase())
    .bind(full_name)
    .bind(hash)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn create_store(pool: &PgPool, name: &str) -> AppResult<Uuid> {
    Ok(sqlx::query_scalar("INSERT INTO stores (name) VALUES ($1) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await?)
}

pub async fn add_member(pool: &PgPool, store_id: Uuid, user_id: Uuid, role: Role) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO store_memberships (store_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (store_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(store_id)
    .bind(user_id)
    .bind(role.as_str())
    .execute(pool)
    .await?;
    Ok(())
}

/// Enables the development mock provider. Never call this in production.
pub async fn enable_mock_provider(pool: &PgPool) -> AppResult<()> {
    sqlx::query("UPDATE external_product_sources SET enabled = TRUE, updated_at = now() WHERE code = 'mock'")
        .execute(pool)
        .await?;
    Ok(())
}

pub struct SeedReport {
    pub store_id: Option<Uuid>,
    pub created: bool,
}

/// Seeds one demo store with three users, categories, and products. Returns early if it exists.
pub async fn seed_demo(pool: &PgPool, password: &str) -> AppResult<SeedReport> {
    let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM stores WHERE name = $1 LIMIT 1")
        .bind(DEMO_STORE_NAME)
        .fetch_optional(pool)
        .await?;
    if let Some(store_id) = existing {
        return Ok(SeedReport {
            store_id: Some(store_id),
            created: false,
        });
    }

    let owner = create_user(pool, "owner@example.com", "Pemilik Demo", password).await?;
    let manager = create_user(pool, "manager@example.com", "Manajer Demo", password).await?;
    let cashier = create_user(pool, "kasir@example.com", "Kasir Demo", password).await?;

    let mut tx = pool.begin().await?;
    let store_id: Uuid = sqlx::query_scalar(
        "INSERT INTO stores (name, address, phone, tax_rate_bps, default_low_stock_threshold, receipt_footer)
         VALUES ($1, 'Jl. Contoh No. 1, Jakarta Selatan', '021-555-0100', 0, 5,
                 'Terima kasih sudah berbelanja. Barang yang sudah dibeli tidak dapat ditukar.')
         RETURNING id",
    )
    .bind(DEMO_STORE_NAME)
    .fetch_one(&mut *tx)
    .await?;

    for (user_id, role) in [(owner, "owner"), (manager, "manager"), (cashier, "cashier")] {
        sqlx::query("INSERT INTO store_memberships (store_id, user_id, role) VALUES ($1, $2, $3)")
            .bind(store_id)
            .bind(user_id)
            .bind(role)
            .execute(&mut *tx)
            .await?;
    }

    let mut category_ids = std::collections::HashMap::new();
    for name in ["Makanan", "Minuman", "Kebersihan", "Lainnya"] {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO categories (store_id, name) VALUES ($1, $2) RETURNING id",
        )
        .bind(store_id)
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
        category_ids.insert(name, id);
    }

    // (name, category, brand, purchase, selling, stock, threshold, gtin or internal code, unit)
    let products: &[(&str, &str, &str, i64, i64, i32, Option<i32>, &str, &str)] = &[
        ("Gula Pasir 1 kg", "Makanan", "Gulaku Demo", 13_000, 16_500, 24, None, "8996001600016", "pcs"),
        ("Minyak Goreng 1 L", "Makanan", "Minyakku Demo", 14_000, 17_500, 3, None, "8996001600023", "botol"),
        ("Teh Celup 25 pcs", "Minuman", "Tehku Demo", 6_000, 9_000, 40, None, "8996001600030", "box"),
        ("Sabun Cuci Piring 800 ml", "Kebersihan", "Bersihku Demo", 9_000, 12_500, 2, None, "8996001600047", "botol"),
        ("Roti Tawar Kecil", "Makanan", "Roti Demo", 9_000, 12_000, 0, Some(2), "ROTI-001", "pcs"),
        ("Es Kopi Kemasan 200 ml", "Minuman", "Kopiku Demo", 4_000, 6_000, 60, None, "8996001600054", "cup"),
    ];

    for &(name, category, brand, purchase, selling, stock, threshold, code, unit) in products {
        let product_id: Uuid = sqlx::query_scalar(
            "INSERT INTO products (store_id, category_id, name, brand, unit, purchase_price_idr,
                 selling_price_idr, stock_quantity, low_stock_threshold, data_source, created_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'manual', $10)
             RETURNING id",
        )
        .bind(store_id)
        .bind(category_ids[category])
        .bind(name)
        .bind(brand)
        .bind(unit)
        .bind(purchase)
        .bind(selling)
        .bind(stock)
        .bind(threshold)
        .bind(owner)
        .fetch_one(&mut *tx)
        .await?;

        let (scheme, kind, value) = if code.chars().all(|c| c.is_ascii_digit()) {
            let gtin = crate::modules::product_identification::normalize_gtin(code)
                .expect("seed GTINs have valid check digits");
            ("gtin", gtin.kind.as_str(), gtin.canonical)
        } else {
            ("internal", "internal", code.to_string())
        };
        sqlx::query(
            "INSERT INTO product_identifiers (store_id, product_id, scheme, kind, raw_value, value, is_primary)
             VALUES ($1, $2, $3, $4, $5, $6, TRUE)",
        )
        .bind(store_id)
        .bind(product_id)
        .bind(scheme)
        .bind(kind)
        .bind(code)
        .bind(value)
        .execute(&mut *tx)
        .await?;

        if stock > 0 {
            sqlx::query(
                "INSERT INTO inventory_movements (store_id, product_id, quantity_delta, quantity_after, reason, created_by)
                 VALUES ($1, $2, $3, $3, 'initial_stock', $4)",
            )
            .bind(store_id)
            .bind(product_id)
            .bind(stock)
            .bind(owner)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    // SKU for the internal-code product so the POS can find it by SKU too.
    sqlx::query("UPDATE products SET sku = 'ROTI-001' WHERE store_id = $1 AND name = 'Roti Tawar Kecil'")
        .bind(store_id)
        .execute(pool)
        .await?;

    Ok(SeedReport {
        store_id: Some(store_id),
        created: true,
    })
}
