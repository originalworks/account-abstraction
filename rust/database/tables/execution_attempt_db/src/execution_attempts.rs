use crate::retry_types::{ExecutionAttemptWithTxRequestRow, RetriedExecutionAttempt};
use db_types::BlobStorageType;
use db_types::{TxStatus, TxType};
use serde::{Deserialize, Serialize};
use sqlx::{
    PgPool, Type,
    types::{Uuid, time::OffsetDateTime},
};
use tx_request_db::tx_requests::TxRequestWithInput;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "text")]
pub enum TxExecutionOutcome {
    STUCK,
    DROPPED,
    SUCCEED,
    FAILED,
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

pub struct ExecutionAttemptRepo {
    pool: PgPool,
}

impl ExecutionAttemptRepo {
    pub fn new(pool: PgPool) -> Self {
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
        .fetch_one(&self.pool)
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
        .fetch_one(&self.pool)
        .await?;

        Ok(attempt)
    }

    pub async fn propagate_outcome(
        &self,
        execution_attempt_id: Uuid,
        outcome: TxExecutionOutcome,
        tx_requests_status: TxStatus,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        let attempt = sqlx::query_as!(
            ExecutionAttempt,
            r#"
            UPDATE execution_attempts
            SET
                outcome = $2
            WHERE
                id = $1
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
            execution_attempt_id,
            outcome as TxExecutionOutcome
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
            UPDATE tx_requests tr
            SET
                tx_status = $2
            FROM execution_attempt_items eai
            JOIN execution_attempts ea
                ON ea.id = eai.execution_attempt_id
            WHERE
                tr.tx_id = eai.tx_id
                AND ea.id = $1
                AND ea.outcome IS NOT NULL
            "#,
            execution_attempt_id,
            tx_requests_status as TxStatus
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }
    pub async fn select_and_lock_for_retry(
        &self,
        execution_attempt_id: Uuid,
    ) -> anyhow::Result<Option<RetriedExecutionAttempt>> {
        let mut tx = self.pool.begin().await?;

        let execution_attempt = sqlx::query_as!(
            ExecutionAttempt,
            r#"
        WITH locked AS (
            SELECT *
            FROM execution_attempts
            WHERE
                id = $1
                AND outcome IN ('FAILED', 'DROPPED', 'STUCK')
                AND retry_lock = false
            FOR UPDATE SKIP LOCKED
        )
        UPDATE execution_attempts ea
        SET
            retry_lock = true
        FROM locked
        WHERE ea.id = locked.id
        RETURNING
            ea.id,
            ea.chain_id,
            ea.operator_wallet_id,
            ea.tx_type as "tx_type: TxType",
            ea.nonce_used,
            ea.tx_value,
            ea.tx_hash,
            ea.gas_limit,
            ea.max_fee_per_gas,
            ea.max_priority_fee,
            ea.max_fee_per_blob_gas,
            ea.outcome as "outcome: TxExecutionOutcome",
            ea.created_at,
            ea.updated_at
        "#,
            execution_attempt_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(execution_attempt) = execution_attempt else {
            tx.rollback().await?;
            return Ok(None);
        };

        let rows = sqlx::query_as!(
            ExecutionAttemptWithTxRequestRow,
            r#"
                SELECT
                    tr.tx_id,
                    tr.requester_id,
                    tr.tx_type as "tx_type: TxType",
                    tr.tx_status as "tx_status: TxStatus",

                    bti.signature as "blob_signature?",
                    bti.image_id as "image_id?",
                    bti.commitment as "commitment?",
                    bti.blob_sha2 as "blob_sha2?",
                    bti.deadline_timestamp as "blob_deadline_timestamp?",
                    bti.storage_type as "storage_type?: BlobStorageType",
                    bti.source_file_path as "source_file_path?",
                    bti.created_at as "blob_created_at?",

                    sti.signature as "standard_signature?",
                    sti.calldata as "calldata?",
                    sti.to_address as "to_address?",
                    sti.value_wei as "value_wei?",
                    sti.deadline_timestamp as "standard_deadline_timestamp?",
                    sti.pass_value_from_operator_wallet as "pass_value_from_operator_wallet?",
                    sti.created_at as "standard_created_at?"

                FROM execution_attempt_items eai
                JOIN tx_requests tr
                    ON tr.tx_id = eai.tx_id

                LEFT JOIN blob_tx_inputs bti
                    ON bti.tx_id = tr.tx_id

                LEFT JOIN standard_tx_inputs sti
                    ON sti.tx_id = tr.tx_id

                WHERE eai.execution_attempt_id = $1
        "#,
            execution_attempt.id
        )
        .fetch_all(&mut *tx)
        .await?;

        let tx_requests = rows
            .into_iter()
            .map(|row| {
                let tx_input = row.into_tx_input().expect(
                    &format!("Could not parse execution attempt row {:?}", row).to_string(),
                );

                TxRequestWithInput {
                    tx_id: row.tx_id,
                    requester_id: row.requester_id,
                    tx_type: row.tx_type,
                    tx_status: row.tx_status,
                    tx_input,
                }
            })
            .collect();

        tx.commit().await?;

        Ok(Some(RetriedExecutionAttempt {
            execution_attempt,
            tx_requests,
        }))
    }
}
