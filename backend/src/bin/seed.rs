//! Seeds demo data for local development.
//!
//! Usage (after migrations have run, or let this binary run them):
//!   DATABASE_URL=postgres://... SEED_PASSWORD='ChangeMe123!' cargo run --bin seed
//!
//! Enables the mock product provider. Do not run against production.

use std::error::Error;

use pos_backend::{db, seed};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    let database_url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required")?;
    let password = std::env::var("SEED_PASSWORD").unwrap_or_else(|_| "ChangeMe123!".to_string());
    if password.len() < 8 {
        return Err("SEED_PASSWORD must be at least 8 characters".into());
    }

    let pool = db::connect(&database_url, 4).await?;
    db::run_migrations(&pool).await?;

    let report = seed::seed_demo(&pool, &password).await?;
    seed::enable_mock_provider(&pool).await?;
    // Open Food Facts is enabled only when a contact User-Agent is configured.
    if std::env::var("OFF_USER_AGENT").map(|v| !v.trim().is_empty()).unwrap_or(false) {
        seed::enable_source(&pool, "open_food_facts").await?;
        println!("Enabled Open Food Facts (OFF_USER_AGENT is set).");
    }

    if report.created {
        println!("Seeded demo store '{}'.", seed::DEMO_STORE_NAME);
    } else {
        println!("Demo store already exists; nothing to seed.");
    }
    println!("Accounts (password from SEED_PASSWORD):");
    println!("  owner@example.com    owner");
    println!("  manager@example.com  manager");
    println!("  kasir@example.com    cashier");
    Ok(())
}
