use alloy::eips::eip1559::Eip1559Estimation;
use anyhow::bail;
use execution_attempt_db::types::ExecutionAttemptWithTxInputs;
use seoa_contract::transaction::{ExecuteBatchTxContext, IntoExecuteInput};
use tx_input_types::TxInput;
use tx_request_db::types::TxRequestWithInput;

const BUFFER_DENOMINATOR: u128 = 1_000_000;

pub trait IntoExecuteBatchTxContext {
    fn into_execute_batch_context(&self) -> anyhow::Result<ExecuteBatchTxContext>;
}

impl IntoExecuteBatchTxContext for ExecutionAttemptWithTxInputs {
    fn into_execute_batch_context(&self) -> anyhow::Result<ExecuteBatchTxContext> {
        let max_fee_per_gas = u128::try_from(
            self.execution_attempt
                .max_fee_per_gas
                .ok_or(anyhow::anyhow!("Can't parse, missing max_fee_per_gas"))?,
        )?;
        let max_priority_fee_per_gas = u128::try_from(
            self.execution_attempt
                .max_priority_fee
                .ok_or(anyhow::anyhow!(
                    "Can't parse, missing max_priority_fee_per_gas"
                ))?,
        )?;
        let fees = Eip1559Estimation {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        };

        let execute_batch_input = self
            .tx_requests
            .iter()
            .map(|tx_request| tx_request.into_execute_input().unwrap())
            .collect();

        Ok(ExecuteBatchTxContext {
            chain_id: self.execution_attempt.chain_id,
            execute_batch_input,
            use_operator_wallet_id: None,
            batch_tx_value: calculate_batch_tx_value(&self.tx_requests)?,
            tx_requests: self.tx_requests.clone(),
            successfully_simulated: false,
            assigned_nonce: try_option_i64_to_option_u64(self.execution_attempt.nonce_used)?,
            fees: Some(fees),
            gas_limit: try_option_i64_to_option_u64(self.execution_attempt.gas_limit)?,
            tx_hash: self.execution_attempt.tx_hash.clone(),
        })
    }
}

fn try_option_i64_to_option_u64(input: Option<i64>) -> anyhow::Result<Option<u64>> {
    let Some(output_i64) = input else {
        bail!("Can't parse None value");
    };

    let output = Some(u64::try_from(output_i64)?);

    Ok(output)
}

pub fn calculate_batch_tx_value(tx_requests: &Vec<TxRequestWithInput>) -> anyhow::Result<i64> {
    let mut batch_tx_value = 0;

    for tx_request in tx_requests {
        let tx_input = match tx_request.tx_input.clone() {
            TxInput::Blob(_) => bail!("Can't calculate batch tx value for BLOB input"),
            TxInput::Standard(input) => input,
        };
        batch_tx_value += tx_input.value_wei;
    }

    Ok(batch_tx_value)
}

pub trait FeeBufferExt {
    fn apply_fee_buffer(&mut self, buffer_ppm: u128) -> anyhow::Result<()>
    where
        Self: Sized;
}

impl FeeBufferExt for ExecuteBatchTxContext {
    fn apply_fee_buffer(&mut self, buffer_ppm: u128) -> anyhow::Result<()> {
        let fees = self
            .fees
            .ok_or(anyhow::anyhow!("Can't apply buffer for undefined fees"))?;
        let fees_with_buffer = Eip1559Estimation {
            max_fee_per_gas: fees.max_fee_per_gas * buffer_ppm / 1_000_000,
            max_priority_fee_per_gas: fees.max_priority_fee_per_gas * buffer_ppm
                / BUFFER_DENOMINATOR,
        };
        let gas_limit_with_buffer = self.gas_limit.ok_or(anyhow::anyhow!(
            "Can't apply buffer for undefined gas_limit"
        ))? * u64::try_from(buffer_ppm)?
            / u64::try_from(BUFFER_DENOMINATOR)?;
        self.fees = Some(fees_with_buffer);
        self.gas_limit = Some(gas_limit_with_buffer);
        Ok(())
    }
}
