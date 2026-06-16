use std::collections::HashMap;

use crate::types::{
    ExecutionAttemptWithTxInputRequestRow, ExecutionAttemptWithTxInputs, ExecutionAttemptWithTxRow,
    ExecutionAttemptWithTxs, OutcomePropagationInput,
};
use db_types::{BlobStorageType, TxExecutionOutcome};
use db_types::{TxStatus, TxType};
use serde::{Deserialize, Serialize};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};
use tx_request_db::tx_requests::{TxRequest, TxRequestWithInput};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct ExecutionAttempt {
    pub id: Uuid,
    pub chain_id: i64,
    pub operator_wallet_id: Uuid,
    pub nonce_used: Option<i64>,
    pub tx_value: i64,
    pub tx_type: TxType,
    pub tx_hash: Option<String>,
    pub gas_limit: Option<i64>,
    pub used_gas: Option<i64>,
    pub max_fee_per_gas: Option<i64>,
    pub max_priority_fee: Option<i64>,
    pub max_fee_per_blob_gas: Option<i64>,
    pub outcome: Option<TxExecutionOutcome>,
    pub error_object: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct NewExecutionAttempt {
    pub chain_id: i64,
    pub operator_wallet_id: Uuid,
    pub nonce_used: Option<i64>,
    pub tx_value: i64,
    pub tx_type: TxType,
    pub tx_hash: Option<String>,
    pub gas_limit: Option<i64>,
    pub used_gas: Option<i64>,
    pub max_fee_per_gas: Option<i64>,
    pub max_priority_fee: Option<i64>,
    pub max_fee_per_blob_gas: Option<i64>,
    pub outcome: Option<TxExecutionOutcome>,
    pub error_object: Option<String>,
    pub retryable: Option<bool>,
}

impl NewExecutionAttempt {
    pub fn default_standard(chain_id: i64, operator_wallet_id: Uuid, tx_value: i64) -> Self {
        Self {
            chain_id,
            operator_wallet_id,
            nonce_used: None,
            tx_value,
            tx_type: TxType::STANDARD,
            tx_hash: None,
            gas_limit: None,
            used_gas: None,
            max_fee_per_gas: None,
            max_priority_fee: None,
            max_fee_per_blob_gas: None,
            outcome: None,
            error_object: None,
            retryable: None,
        }
    }
}

pub struct ExecutionAttemptRepo {
    pool: PgPool,
}

impl ExecutionAttemptRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_old_unresolved(&self) -> anyhow::Result<Vec<ExecutionAttemptWithTxs>> {
        let rows = sqlx::query_as!(
            ExecutionAttemptWithTxRow,
            r#"
            SELECT
                ea.id as attempt_id,
                ea.chain_id,
                ea.operator_wallet_id,
                ea.nonce_used,
                ea.tx_value,
                ea.tx_type as "tx_type: TxType",
                ea.tx_hash,
                ea.gas_limit,
                ea.used_gas,
                ea.max_fee_per_gas,
                ea.max_priority_fee,
                ea.max_fee_per_blob_gas,
                ea.outcome as "outcome: TxExecutionOutcome",
                ea.error_object,
                ea.created_at as attempt_created_at,
                ea.updated_at as attempt_updated_at,

                tr.sequence_id,
                tr.tx_id,
                tr.requester_id,
                tr.tx_type as "request_tx_type?: TxType",
                tr.tx_status as "tx_status?: TxStatus",
                tr.chain_id as request_chain_id,
                tr.use_operator_wallet_id,
                tr.attempts,
                tr.metadata,
                tr.created_at as request_created_at,
                tr.updated_at as request_updated_at

            FROM (
                SELECT *
                FROM execution_attempts
                WHERE outcome IS NULL
                ORDER BY created_at ASC
                LIMIT 10
            ) ea

            LEFT JOIN execution_attempt_items eai
                ON eai.execution_attempt_id = ea.id

            LEFT JOIN tx_requests tr
                ON tr.tx_id = eai.tx_id

            ORDER BY ea.created_at ASC
        "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut grouped: HashMap<Uuid, ExecutionAttemptWithTxs> = HashMap::new();

        for row in rows {
            let entry = grouped
                .entry(row.attempt_id)
                .or_insert_with(|| ExecutionAttemptWithTxs {
                    execution_attempt: ExecutionAttempt {
                        id: row.attempt_id,
                        chain_id: row.chain_id,
                        operator_wallet_id: row.operator_wallet_id,
                        nonce_used: row.nonce_used,
                        tx_value: row.tx_value,
                        tx_type: row.tx_type.clone(),
                        tx_hash: row.tx_hash.clone(),
                        gas_limit: row.gas_limit,
                        used_gas: row.used_gas,
                        max_fee_per_gas: row.max_fee_per_gas,
                        max_priority_fee: row.max_priority_fee,
                        max_fee_per_blob_gas: row.max_fee_per_blob_gas,
                        outcome: row.outcome.clone(),
                        error_object: row.error_object.clone(),
                        created_at: row.attempt_created_at,
                        updated_at: row.attempt_updated_at,
                    },
                    tx_requests: Vec::new(),
                });

            if let (
                Some(sequence_id),
                Some(tx_id),
                Some(requester_id),
                Some(tx_type),
                Some(tx_status),
                Some(chain_id),
                Some(attempts),
                Some(created_at),
                Some(updated_at),
            ) = (
                row.sequence_id,
                row.tx_id,
                row.requester_id,
                row.request_tx_type,
                row.tx_status,
                row.request_chain_id,
                row.attempts,
                row.request_created_at,
                row.request_updated_at,
            ) {
                entry.tx_requests.push(TxRequest {
                    sequence_id,
                    tx_id,
                    requester_id,
                    tx_type,
                    tx_status,
                    chain_id,
                    use_operator_wallet_id: row.use_operator_wallet_id,
                    attempts: attempts.into(),
                    metadata: row.metadata,
                    created_at,
                    updated_at,
                });
            }
        }

        let mut attempts: Vec<_> = grouped.into_values().collect();

        attempts.sort_by_key(|a| a.execution_attempt.created_at);

        Ok(attempts)
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
                used_gas,
                max_fee_per_gas,
                max_priority_fee,
                max_fee_per_blob_gas,
                outcome as "outcome: TxExecutionOutcome",
                error_object,
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
    pub async fn insert(&self, input: &NewExecutionAttempt) -> anyhow::Result<ExecutionAttempt> {
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
                used_gas,
                max_fee_per_gas,
                max_priority_fee,
                max_fee_per_blob_gas,
                tx_value,
                outcome,
                error_object,
                retryable
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15
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
                used_gas,
                max_fee_per_gas,
                max_priority_fee,
                max_fee_per_blob_gas,
                outcome as "outcome: TxExecutionOutcome",
                error_object,
                created_at,
                updated_at
            "#,
            uuid::Uuid::new_v4(),
            input.chain_id,
            input.operator_wallet_id,
            input.nonce_used,
            input.tx_type.clone() as TxType,
            input.tx_hash,
            input.gas_limit,
            input.used_gas,
            input.max_fee_per_gas,
            input.max_priority_fee,
            input.max_fee_per_blob_gas,
            input.tx_value,
            input.outcome.clone() as Option<TxExecutionOutcome>,
            input.error_object,
            input.retryable
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(attempt)
    }

    pub async fn propagate_outcome(
        &self,
        propagation_input: &OutcomePropagationInput,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        let attempt = sqlx::query_as!(
            ExecutionAttempt,
            r#"
            UPDATE execution_attempts
            SET
                outcome = $2,
                retryable = $3,
                used_gas = $4
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
                used_gas,
                max_fee_per_gas,
                max_priority_fee,
                max_fee_per_blob_gas,
                outcome as "outcome: TxExecutionOutcome",
                error_object,
                created_at,
                updated_at
            "#,
            propagation_input.execution_attempt_id,
            propagation_input.outcome.clone() as TxExecutionOutcome,
            propagation_input.retryable,
            propagation_input.used_gas
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
            propagation_input.execution_attempt_id,
            propagation_input.tx_requests_status.clone() as TxStatus
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }
    pub async fn select_and_lock_for_retry(
        &self,
        execution_attempt_id: Uuid,
    ) -> anyhow::Result<Option<ExecutionAttemptWithTxInputs>> {
        let mut tx = self.pool.begin().await?;

        let execution_attempt = sqlx::query_as!(
            ExecutionAttempt,
            r#"
                WITH locked AS (
                    SELECT *
                    FROM execution_attempts
                    WHERE
                        id = $1
                        AND outcome IN ('FAILED', 'DROPPED', 'STUCK', 'REVERTED')
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
                    ea.used_gas,
                    ea.max_fee_per_gas,
                    ea.max_priority_fee,
                    ea.max_fee_per_blob_gas,
                    ea.outcome as "outcome: TxExecutionOutcome",
                    ea.error_object,
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
            ExecutionAttemptWithTxInputRequestRow,
            r#"
                SELECT
                    tr.tx_id,
                    tr.requester_id,
                    tr.tx_type as "tx_type: TxType",
                    tr.tx_status as "tx_status: TxStatus",
                    metadata,

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
                    metadata: row.metadata,
                }
            })
            .collect();

        tx.commit().await?;

        Ok(Some(ExecutionAttemptWithTxInputs {
            execution_attempt,
            tx_requests,
        }))
    }

    pub async fn select_with_txs(
        &self,
        execution_attempt_id: &Uuid,
    ) -> anyhow::Result<Option<ExecutionAttemptWithTxs>> {
        let rows = sqlx::query_as!(
            ExecutionAttemptWithTxRow,
            r#"
                SELECT
                    ea.id as attempt_id,
                    ea.chain_id,
                    ea.operator_wallet_id,
                    ea.nonce_used,
                    ea.tx_value,
                    ea.tx_type as "tx_type: TxType",
                    ea.tx_hash,
                    ea.gas_limit,
                    ea.used_gas,
                    ea.max_fee_per_gas,
                    ea.max_priority_fee,
                    ea.max_fee_per_blob_gas,
                    ea.outcome as "outcome: TxExecutionOutcome",
                    ea.error_object,
                    ea.created_at as attempt_created_at,
                    ea.updated_at as attempt_updated_at,

                    tr.sequence_id,
                    tr.tx_id,
                    tr.requester_id,
                    tr.tx_type as "request_tx_type?: TxType",
                    tr.tx_status as "tx_status?: TxStatus",
                    tr.chain_id as request_chain_id,
                    tr.use_operator_wallet_id,
                    tr.attempts,
                    tr.metadata,
                    tr.created_at as request_created_at,
                    tr.updated_at as request_updated_at

                FROM execution_attempts ea
                LEFT JOIN execution_attempt_items eai
                    ON eai.execution_attempt_id = ea.id
                LEFT JOIN tx_requests tr
                    ON tr.tx_id = eai.tx_id
                WHERE ea.id = $1
        "#,
            execution_attempt_id
        )
        .fetch_all(&self.pool)
        .await?;

        let Some(first) = rows.first() else {
            return Ok(None);
        };

        let execution_attempt = ExecutionAttempt {
            id: first.attempt_id,
            chain_id: first.chain_id,
            operator_wallet_id: first.operator_wallet_id,
            nonce_used: first.nonce_used,
            tx_value: first.tx_value,
            tx_type: first.tx_type.clone(),
            tx_hash: first.tx_hash.clone(),
            gas_limit: first.gas_limit,
            used_gas: first.used_gas,
            max_fee_per_gas: first.max_fee_per_gas,
            max_priority_fee: first.max_priority_fee,
            max_fee_per_blob_gas: first.max_fee_per_blob_gas,
            outcome: first.outcome.clone(),
            error_object: first.error_object.clone(),
            created_at: first.attempt_created_at,
            updated_at: first.attempt_updated_at,
        };

        let tx_requests = rows
            .into_iter()
            .filter_map(|row| {
                Some(TxRequest {
                    sequence_id: row.sequence_id?,
                    tx_id: row.tx_id?,
                    requester_id: row.requester_id?,
                    tx_type: row.request_tx_type?,
                    tx_status: row.tx_status?,
                    chain_id: row.request_chain_id?,
                    use_operator_wallet_id: row.use_operator_wallet_id,
                    attempts: row.attempts?.into(),
                    metadata: row.metadata,
                    created_at: row.request_created_at?,
                    updated_at: row.request_updated_at?,
                })
            })
            .collect();

        Ok(Some(ExecutionAttemptWithTxs {
            execution_attempt,
            tx_requests,
        }))
    }
}
