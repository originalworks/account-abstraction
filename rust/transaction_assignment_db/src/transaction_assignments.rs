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
    transaction_sequence_id: i64,
    operator_wallet_id: String,
    outcome: TxAssignmentOutcome,
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

    // pub async fn insert_ignore_conflict(
    //     &self,
    //     input: &InsertTransactionInput,
    // ) -> Result<bool, sqlx::Error> {
    //     let result = sqlx::query!(
    //         r#"
    //         INSERT INTO transactions (
    //             tx_id,
    //             requester_id,
    //             tx_status,
    //             tx_type,
    //             blob_file_path,
    //             calldata,
    //             to_address,
    //             chain_id,
    //             signature
    //         )
    //         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
    //         ON CONFLICT (tx_id) DO NOTHING
    //         "#,
    //         input.tx_id,
    //         input.requester_id,
    //         input.tx_status.clone() as TxStatus,
    //         input.tx_type.clone() as TxType,
    //         input.blob_file_path,
    //         input.calldata,
    //         input.to_address,
    //         input.chain_id,
    //         input.signature
    //     )
    //     .execute(self.pool)
    //     .await?;

    //     Ok(result.rows_affected() == 1)
    // }

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
