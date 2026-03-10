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
pub struct TransactionAssignment {
    pub id: Uuid,
    pub transaction_sequence_id: i64,
    pub operator_wallet_id: String,
    pub nonce_used: Option<i64>,
    pub gas_limit: Option<i64>,
    pub max_fee_per_gas: Option<i64>,
    pub max_priority_fee: Option<i64>,
    pub outcome: Option<TxAssignmentOutcome>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct TransactionAssignmentRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> TransactionAssignmentRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(
        &self,
        tx_assignment_id: Uuid,
    ) -> anyhow::Result<TransactionAssignment> {
        let transaction_assignment = sqlx::query_as!(
            TransactionAssignment,
            r#"
            SELECT 
                id,
                transaction_sequence_id, 
                operator_wallet_id, 
                nonce_used,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee,
                outcome as "outcome: TxAssignmentOutcome", 
                created_at,
                updated_at
            FROM 
                transaction_assignments
            WHERE
                id = $1"#,
            tx_assignment_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(transaction_assignment)
    }
}
