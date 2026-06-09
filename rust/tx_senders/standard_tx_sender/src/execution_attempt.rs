use crate::transaction::ExecuteBatchTxContext;
use anyhow::bail;
use db_types::{ExecutionErrorObject, TxExecutionOutcome, TxType};
use execution_attempt_db::execution_attempts::NewExecutionAttempt;
use sqs_queue::message_body::ToJsonString;
use uuid::Uuid;

pub trait NewStandardExecutionAttemptBuilder {
    fn standard_successful(
        tx_context: &ExecuteBatchTxContext,
        operator_wallet_id: Uuid,
    ) -> anyhow::Result<NewExecutionAttempt>;

    fn standard_failed(
        tx_context: &ExecuteBatchTxContext,
        operator_wallet_id: Uuid,
        error_object: ExecutionErrorObject,
        retryable: bool,
    ) -> anyhow::Result<NewExecutionAttempt>;
}

impl NewStandardExecutionAttemptBuilder for NewExecutionAttempt {
    fn standard_successful(
        tx_context: &ExecuteBatchTxContext,
        operator_wallet_id: Uuid,
    ) -> anyhow::Result<Self> {
        let Some(fees) = tx_context.fees else {
            bail!("Can't build successful tx without fees");
        };
        let nonce_used = try_option_u64_to_option_i64(tx_context.assigned_nonce)?;
        let gas_limit = try_option_u64_to_option_i64(tx_context.gas_limit)?;

        Ok(NewExecutionAttempt {
            chain_id: tx_context.chain_id,
            operator_wallet_id,
            nonce_used,
            tx_type: TxType::STANDARD,
            tx_hash: tx_context.tx_hash.clone(),
            used_gas: None,
            gas_limit,
            max_fee_per_gas: Some(i64::try_from(fees.max_fee_per_gas)?),
            max_priority_fee: Some(i64::try_from(fees.max_priority_fee_per_gas)?),
            max_fee_per_blob_gas: None,
            tx_value: tx_context.batch_tx_value,
            outcome: None,
            error_object: None,
            retryable: None,
        })
    }

    fn standard_failed(
        tx_context: &ExecuteBatchTxContext,
        operator_wallet_id: Uuid,
        error_object: ExecutionErrorObject,
        retryable: bool,
    ) -> anyhow::Result<Self> {
        if tx_context.successfully_simulated {
            let Some(fees) = tx_context.fees else {
                bail!("Tx fees should be known after simulation");
            };
            let nonce_used = try_option_u64_to_option_i64(tx_context.assigned_nonce)?;
            let gas_limit = try_option_u64_to_option_i64(tx_context.gas_limit)?;

            return Ok(NewExecutionAttempt {
                chain_id: tx_context.chain_id,
                operator_wallet_id: operator_wallet_id,
                nonce_used,
                tx_value: tx_context.batch_tx_value,
                tx_type: TxType::STANDARD,
                tx_hash: tx_context.tx_hash.clone(),
                gas_limit,
                used_gas: None,
                max_fee_per_gas: Some(i64::try_from(fees.max_fee_per_gas)?),
                max_priority_fee: Some(i64::try_from(fees.max_priority_fee_per_gas)?),
                max_fee_per_blob_gas: None,
                outcome: Some(TxExecutionOutcome::REVERTED),
                error_object: Some(error_object.to_json_string()?),
                retryable: Some(retryable),
            });
        } else {
            return Ok(NewExecutionAttempt {
                chain_id: tx_context.chain_id,
                operator_wallet_id: operator_wallet_id,
                nonce_used: None,
                tx_value: tx_context.batch_tx_value,
                tx_type: TxType::STANDARD,
                tx_hash: None,
                gas_limit: None,
                used_gas: None,
                max_fee_per_gas: None,
                max_priority_fee: None,
                max_fee_per_blob_gas: None,
                outcome: Some(TxExecutionOutcome::REVERTED),
                error_object: Some(error_object.to_json_string()?),
                retryable: Some(retryable),
            });
        }
    }
}

fn try_option_u64_to_option_i64(input: Option<u64>) -> anyhow::Result<Option<i64>> {
    let Some(output_u64) = input else {
        bail!("Can't parse None value");
    };

    let output = Some(i64::try_from(output_u64)?);

    Ok(output)
}
