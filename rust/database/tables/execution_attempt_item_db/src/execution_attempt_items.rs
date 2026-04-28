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

    pub async fn insert_many(
        &self,
        execution_attempt_id: Uuid,
        tx_ids: &Vec<String>,
    ) -> anyhow::Result<()> {
        let ids: Vec<Uuid> = tx_ids.iter().map(|_| Uuid::new_v4()).collect();

        sqlx::query!(
            r#"
            INSERT INTO execution_attempt_items (id, execution_attempt_id, tx_id)
            SELECT id, $1, tx_id
            FROM UNNEST($2::uuid[], $3::text[]) AS t(id, tx_id)
            "#,
            execution_attempt_id,
            &ids,
            tx_ids
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }
}
