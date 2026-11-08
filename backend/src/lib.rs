//! POS backend: HTTP API, domain modules and persistence.
//!
//! Layout:
//! - `app`        router construction and shared application state
//! - `auth`       password hashing, server-side sessions, request extractors
//! - `modules`    domain modules (products, product identification, inventory, sales, ...)
//! - `settings`   store configuration
//! - `db`         pool creation and embedded migrations
//! - `error`      the single error type that maps to the public API error format

pub mod app;
pub mod audit;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod modules;
pub mod money;
pub mod pagination;
pub mod rate_limit;
pub mod seed;
pub mod settings;
pub mod util;

pub use app::{build_router, AppState};
pub use config::Config;
pub use error::{AppError, AppResult};
