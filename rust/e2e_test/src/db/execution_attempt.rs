use db_types::{TxExecutionOutcome, TxType};
use execution_attempt_db::execution_attempts::{ExecutionAttempt, ExecutionAttemptRepo};
use uuid::Uuid;

#[allow(async_fn_in_trait)]
pub trait ExecutionAttemptTestExt {
    async fn find_by_tx_id(&self, tx_id: &String) -> anyhow::Result<Vec<ExecutionAttempt>>;
    async fn find_by_source_execution_attempt_id(
        &self,
        source_execution_attempt_id: &Uuid,
    ) -> anyhow::Result<Vec<ExecutionAttempt>>;
}

impl ExecutionAttemptTestExt for ExecutionAttemptRepo {
    async fn find_by_source_execution_attempt_id(
        &self,
        source_execution_attempt_id: &Uuid,
    ) -> anyhow::Result<Vec<ExecutionAttempt>> {
        let attempts = sqlx::query_as!(
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
            source_execution_attempt_id = $1
        ORDER BY
            created_at DESC
        "#,
            source_execution_attempt_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(attempts)
    }
    async fn find_by_tx_id(&self, tx_id: &String) -> anyhow::Result<Vec<ExecutionAttempt>> {
        let attempts = sqlx::query_as!(
            ExecutionAttempt,
            r#"
        SELECT
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
        FROM
            execution_attempts ea
        INNER JOIN
            execution_attempt_items eai
            ON eai.execution_attempt_id = ea.id
        WHERE
            eai.tx_id = $1
        ORDER BY
            ea.created_at DESC
        "#,
            tx_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(attempts)
    }
}
