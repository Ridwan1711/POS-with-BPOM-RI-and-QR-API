//! Authentication: argon2id password hashing, opaque server-side sessions in an
//! HttpOnly cookie, and request extractors that enforce store membership and roles.

pub mod context;
pub mod handlers;
pub mod password;
pub mod session;

pub use context::{CurrentUser, Role, StoreContext, STORE_HEADER};
