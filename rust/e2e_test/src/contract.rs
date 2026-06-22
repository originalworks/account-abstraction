use std::str::FromStr;

use alloy::{
    primitives::{Address, Uint},
    providers::{Provider, ProviderBuilder},
};
use anyhow::bail;
use execution_attempt_db::execution_attempts::NewExecutionAttempt;
use seoa_contract::{
    contract::{ContractManager, SEOA},
    transaction::ExecuteBatchTxContext,
};
use standard_tx_sender::execution_attempt::NewStandardExecutionAttemptBuilder;
use wallet_pool::wallet::Wallet;

#[allow(async_fn_in_trait)]
pub trait ContractManagerForTests {
    async fn send_batch_with_underpriced_gas(
        &self,
        tx_context: &ExecuteBatchTxContext,
        wallet: Wallet,
    ) -> anyhow::Result<NewExecutionAttempt>;
}

impl ContractManagerForTests for ContractManager {
    async fn send_batch_with_underpriced_gas(
        &self,
        tx_context: &ExecuteBatchTxContext,
        mut wallet: Wallet,
    ) -> anyhow::Result<NewExecutionAttempt> {
        let Some(network) = self.networks_by_chain_id.get(&tx_context.chain_id) else {
            bail!(
                "Contract address not found for chain id: {}",
                tx_context.chain_id
            );
        };
        let Some(root_provider) = self.providers_by_chain_id.get(&tx_context.chain_id) else {
            bail!("Provider not found for chain id: {}", tx_context.chain_id);
        };
        let nonce = wallet.use_nonce()?;
        let provider = ProviderBuilder::new()
            .wallet(wallet.ow_wallet.wallet)
            .connect_provider(root_provider);
        let contract = SEOA::new(
            Address::from_str(network.contract_address.as_str())?,
            &provider,
        );

        let tx_value = Uint::<256, 4>::from(tx_context.batch_tx_value);

        let mut fees = provider.estimate_eip1559_fees().await?;
        fees.max_fee_per_gas = 3;
        fees.max_priority_fee_per_gas = 2;

        let call_builder = contract
            .executeBatch(tx_context.execute_batch_input.clone())
            .value(tx_value)
            .nonce(nonce)
            .max_fee_per_gas(fees.max_fee_per_gas)
            .max_priority_fee_per_gas(fees.max_priority_fee_per_gas);

        let gas = i64::try_from(call_builder.estimate_gas().await?)?;
        let gas_with_buffer = gas + gas * network.gas_estimation_buffer_ppm / 1_000_000;

        let pending_tx = call_builder
            .gas(u64::try_from(gas_with_buffer)?)
            .send()
            .await?;

        let tx_hash = pending_tx.tx_hash().to_string();

        let new_execution_attempt =
            NewExecutionAttempt::standard_successful(tx_context, wallet.db_record.id)?;

        Ok(new_execution_attempt)
    }
}
