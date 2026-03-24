use serde::{Deserialize, Serialize};
use sqlx::Type;
use sqlx::{PgPool, types::time::OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "text")]
pub enum TxAssignmentOutcome {
    SUCCEED,
    FAILED,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WalletAssignment {
    pub id: Uuid,
    pub tx_id: String,
    pub operator_wallet_id: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct WalletAssignmentRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> WalletAssignmentRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, tx_assignment_id: Uuid) -> anyhow::Result<WalletAssignment> {
        let transaction_assignment = sqlx::query_as!(
            WalletAssignment,
            r#"
            SELECT 
                id,
                tx_id, 
                operator_wallet_id, 
                created_at,
                updated_at
            FROM 
                wallet_assignments
            WHERE
                id = $1"#,
            tx_assignment_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(transaction_assignment)
    }

    pub async fn new_assignment(
        &self,
        tx_id: String,
        operator_wallet_id: Uuid,
    ) -> anyhow::Result<Uuid> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO wallet_assignments (
                tx_id,
                operator_wallet_id
            )
            VALUES ($1, $2)
            RETURNING id
            "#,
            tx_id,
            operator_wallet_id,
        )
        .fetch_one(self.pool)
        .await?;

        Ok(id)
    }
}
