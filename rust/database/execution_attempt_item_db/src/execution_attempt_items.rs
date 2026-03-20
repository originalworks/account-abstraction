use serde::{Deserialize, Serialize};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct ExecutionAttemptItem {
    pub id: Uuid,
    pub execution_attempt_id: Uuid,
    pub tx_id: String,
    pub created_at: OffsetDateTime,
}

pub struct ExecutionAttemptItemRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> ExecutionAttemptItemRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, id: Uuid) -> anyhow::Result<ExecutionAttemptItem> {
        let attempt_item = sqlx::query_as!(
            ExecutionAttemptItem,
            r#"
            SELECT
                id,
                execution_attempt_id,
                tx_id,
                created_at
            FROM
                execution_attempt_items
            WHERE
                id = $1"#,
            id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(attempt_item)
    }
}
