use std::error::Error;

use pos_backend::{build_router, db, AppState, Config};
use tokio::{net::TcpListener, signal};
use tracing_subscriber::{prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let config = Config::from_env()?;
    let bind_addr = config.bind_addr.clone();

    let pool = db::connect(&config.database_url, config.db_max_connections).await?;
    db::run_migrations(&pool).await?;
    tracing::info!("database migrations are up to date");

    let app = build_router(AppState::new(pool, config));
    let listener = TcpListener::bind(&bind_addr).await?;
    tracing::info!(address = %bind_addr, "POS API listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("POS API stopped");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into());
    let json = std::env::var("LOG_JSON").map(|v| v == "true" || v == "1").unwrap_or(false);
    let registry = tracing_subscriber::registry().with(filter);
    if json {
        registry.with(tracing_subscriber::fmt::layer().json()).init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
