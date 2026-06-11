use crate::{
    execution_attempt::NewStandardExecutionAttemptBuilder, transaction::ExecuteBatchTxContext,
};
use alloy::{
    primitives::{Address, Uint},
    providers::{
        Provider, ProviderBuilder,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
    sol,
};
use anyhow::bail;
use execution_attempt_db::execution_attempts::NewExecutionAttempt;
use network_db::networks::Network;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};
use wallet_pool::wallet::Wallet;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug, Deserialize, Serialize)]
    SEOA,
    "../../../contracts/artifacts/contracts/sEOA.sol/sEOA.json"
);

type HardlyTypedProvider = FillProvider<
    JoinFill<
        alloy::providers::Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    alloy::providers::RootProvider,
>;

pub struct ContractManager {
    pub networks_by_chain_id: HashMap<i64, Network>,
    pub providers_by_chain_id: HashMap<i64, HardlyTypedProvider>,
}

impl ContractManager {
    pub async fn build(networks: &Vec<Network>) -> anyhow::Result<Self> {
        let mut networks_by_chain_id = HashMap::new();
        let mut providers_by_chain_id = HashMap::new();
        for network in networks {
            networks_by_chain_id.insert(network.chain_id, network.clone());
            let provider = ProviderBuilder::new().connect_http(network.rpc_url.parse()?);
            providers_by_chain_id.insert(network.chain_id, provider);
        }

        Ok(Self {
            networks_by_chain_id,
            providers_by_chain_id,
        })
    }

    pub async fn simulate_send_batch_tx(
        &self,
        tx_context: &mut ExecuteBatchTxContext,
        wallet: &mut Wallet,
    ) -> anyhow::Result<()> {
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
            .wallet(&wallet.ow_wallet.wallet)
            .connect_provider(root_provider);
        let contract = SEOA::new(
            Address::from_str(network.contract_address.as_str())?,
            &provider,
        );

        let tx_value = Uint::<256, 4>::from(tx_context.batch_tx_value);

        let fees = provider.estimate_eip1559_fees().await?;
        let call = contract
            .executeBatch(tx_context.execute_batch_input.clone())
            .value(tx_value)
            .nonce(nonce)
            .max_fee_per_gas(fees.max_fee_per_gas)
            .max_priority_fee_per_gas(fees.max_priority_fee_per_gas);

        let estimated_gas = call.estimate_gas().await?;

        let gas_limit = estimated_gas
            + estimated_gas * u64::try_from(network.gas_estimation_buffer_ppm)? / 1_000_000;

        call.gas(gas_limit).call().await?;

        tx_context.assigned_nonce = Some(nonce);
        tx_context.fees = Some(fees);
        tx_context.gas_limit = Some(gas_limit);
        tx_context.successfully_simulated = true;

        Ok(())
    }

    pub async fn send_batch(
        &self,
        tx_context: &mut ExecuteBatchTxContext,
        wallet: &Wallet,
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

        let Some(nonce) = tx_context.assigned_nonce else {
            bail!("Nonce should be assinged at this point. Use simulate_send_batch_tx first");
        };

        let Some(fees) = tx_context.fees else {
            bail!("Fees should be calculated at this point. Use simulate_send_batch_tx first");
        };

        let Some(gas_limit) = tx_context.gas_limit else {
            bail!("Gas limit should be calculated at this point. Use simulate_send_batch_tx first");
        };

        let provider = ProviderBuilder::new()
            .wallet(wallet.ow_wallet.wallet.clone())
            .connect_provider(root_provider);
        let contract = SEOA::new(
            Address::from_str(network.contract_address.as_str())?,
            &provider,
        );
        println!("broadcasting tx with nonce: {}", nonce);

        let pending_tx = contract
            .executeBatch(tx_context.execute_batch_input.clone())
            .value(Uint::<256, 4>::from(tx_context.batch_tx_value))
            .nonce(nonce)
            .max_fee_per_gas(fees.max_fee_per_gas)
            .max_priority_fee_per_gas(fees.max_priority_fee_per_gas)
            .gas(gas_limit)
            .send()
            .await?;
        println!("braodcasted, all good. Tx nonce: {}", nonce);

        tx_context.tx_hash = Some(pending_tx.tx_hash().to_string());

        let new_execution_attempt =
            NewExecutionAttempt::standard_successful(tx_context, wallet.db_record.id)?;

        Ok(new_execution_attempt)
    }
}
