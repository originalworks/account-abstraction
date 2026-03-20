use db_types::TxType;
use serde::{Deserialize, Serialize};
use sqlx::{
    PgPool, Type,
    types::{Uuid, time::OffsetDateTime},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "text")]
enum TxExecutionOutcome {
    SUCCEED,
    FAILED,
    REVERTED,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct ExecutionAttempt {
    pub id: Uuid,
    pub chain_id: i64,
    pub operator_wallet_id: Uuid,
    pub nonce_used: i64,
    pub tx_type: TxType,
    pub tx_hash: String,
    pub gas_limit: i64,
    pub max_fee_per_gas: i64,
    pub max_priority_fee: i64,
    pub max_fee_per_blob_gas: Option<i64>,
    pub outcome: Option<TxExecutionOutcome>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct ExecutionAttemptRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> ExecutionAttemptRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, id: Uuid) -> anyhow::Result<ExecutionAttempt> {
        let attempt = sqlx::query_as!(
            ExecutionAttempt,
            r#"
            SELECT
                id,
                chain_id,
                operator_wallet_id,
                tx_type as "tx_type: TxType", 
                nonce_used,
                tx_hash,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee,
                max_fee_per_blob_gas,
                outcome as "outcome: TxExecutionOutcome",
                created_at,
                updated_at
            FROM
                execution_attempts
            WHERE
                id = $1"#,
            id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(attempt)
    }
}
