use db_types::TxType;
use serde::{Deserialize, Serialize};
use sqlx::{
    PgPool, Type,
    types::{Uuid, time::OffsetDateTime},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "text")]
pub enum TxExecutionOutcome {
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
    pub tx_value: i64,
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

pub struct NewExecutionAttempt {
    pub chain_id: i64,
    pub operator_wallet_id: Uuid,
    pub nonce_used: i64,
    pub tx_value: i64,
    pub tx_type: TxType,
    pub tx_hash: String,
    pub gas_limit: i64,
    pub max_fee_per_gas: i64,
    pub max_priority_fee: i64,
    pub max_fee_per_blob_gas: Option<i64>,
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
                tx_value,
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
    pub async fn insert(&self, input: NewExecutionAttempt) -> anyhow::Result<ExecutionAttempt> {
        let attempt = sqlx::query_as!(
            ExecutionAttempt,
            r#"
            INSERT INTO execution_attempts (
                id,
                chain_id,
                operator_wallet_id,
                nonce_used,
                tx_type,
                tx_hash,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee,
                max_fee_per_blob_gas,
                tx_value
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
            )
            RETURNING
                id,
                chain_id,
                operator_wallet_id,
                tx_type as "tx_type: TxType",
                nonce_used,
                tx_value,
                tx_hash,
                gas_limit,
                max_fee_per_gas,
                max_priority_fee,
                max_fee_per_blob_gas,
                outcome as "outcome: TxExecutionOutcome",
                created_at,
                updated_at
            "#,
            uuid::Uuid::new_v4(),
            input.chain_id,
            input.operator_wallet_id,
            input.nonce_used,
            input.tx_type as TxType,
            input.tx_hash,
            input.gas_limit,
            input.max_fee_per_gas,
            input.max_priority_fee,
            input.max_fee_per_blob_gas,
            input.tx_value
        )
        .fetch_one(self.pool)
        .await?;

        Ok(attempt)
    }
}
