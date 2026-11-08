//! Products: store-owned catalog items with pricing, stock snapshot, and identifiers.
//!
//! Ownership rules:
//! - Merchant fields (prices, stock, SKU, category) are never overwritten by provider data.
//! - Stock changes only through `modules::inventory` and `modules::sales`, which write the ledger.

pub mod dto;
pub mod handlers;
pub mod repository;
