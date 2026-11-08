//! Audit trail for security-relevant and business-critical actions.
//! Callers pass the transaction connection so that the audit row commits atomically with the change.

use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn record(
    conn: &mut PgConnection,
    store_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<String>,
    metadata: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_logs (store_id, user_id, action, entity_type, entity_id, metadata)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(store_id)
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(metadata)
    .execute(conn)
    .await?;
    Ok(())
}
