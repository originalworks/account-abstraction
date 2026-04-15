use crate::{transaction::ExecuteBatchTxContext, wallet_pool::Wallet};
use alloy::{
    eips::eip1559::Eip1559Estimation,
    network::ReceiptResponse,
    primitives::{Address, Uint},
    providers::{
        Provider, ProviderBuilder,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
    sol,
};
use anyhow::bail;
use db_types::TxType;
use execution_attempt_db::execution_attempts::NewExecutionAttempt;
use network_db::networks::Network;
use std::{collections::HashMap, str::FromStr};
use uuid::Uuid;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug)]
    SEOA,
    "../../contracts/artifacts/contracts/sEOA.sol/sEOA.json"
);

trait BuildNewExecutionAttempt {
    fn build_standard(
        fees: Eip1559Estimation,
        gas_limit: i64,
        tx_hash: String,
        nonce: u64,
        operator_wallet_id: Uuid,
        chain_id: i64,
        tx_value: i64,
    ) -> anyhow::Result<NewExecutionAttempt>;
}

impl BuildNewExecutionAttempt for NewExecutionAttempt {
    fn build_standard(
        fees: Eip1559Estimation,
        gas_limit: i64,
        tx_hash: String,
        nonce: u64,
        operator_wallet_id: Uuid,
        chain_id: i64,
        tx_value: i64,
    ) -> anyhow::Result<Self> {
        Ok(NewExecutionAttempt {
            chain_id,
            operator_wallet_id,
            nonce_used: i64::try_from(nonce)?,
            tx_type: TxType::STANDARD,
            tx_hash,
            gas_limit,
            max_fee_per_gas: i64::try_from(fees.max_fee_per_gas)?,
            max_priority_fee: i64::try_from(fees.max_priority_fee_per_gas)?,
            max_fee_per_blob_gas: None,
            tx_value,
        })
    }
}

type HardlyTypedProvider = FillProvider<
    JoinFill<
        alloy::providers::Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    alloy::providers::RootProvider,
>;

pub struct ContractManager {
    networks_by_chain_id: HashMap<i64, Network>,
    providers_by_chain_id: HashMap<i64, HardlyTypedProvider>,
}

impl ContractManager {
    pub async fn build(networks: &Vec<Network>) -> anyhow::Result<Self> {
        let mut networks_by_chain_id = HashMap::new();
        let mut providers_by_chain_id = HashMap::new();
        for network in networks {
            networks_by_chain_id.insert(network.chain_id, network.clone());
            let provider = ProviderBuilder::new().connect_http(network.rpc_url.parse()?);
            let chain_id = provider.get_chain_id().await?;
            if i64::try_from(chain_id)? != network.chain_id {
                bail!(
                    "Chain id mismatch for {:?}. Fetched chain_id: {}",
                    network,
                    chain_id
                );
            }
            providers_by_chain_id.insert(network.chain_id, provider);
        }

        Ok(Self {
            networks_by_chain_id,
            providers_by_chain_id,
        })
    }

    pub async fn send_batch(
        &self,
        tx_context: &ExecuteBatchTxContext,
        wallet: Wallet,
        nonce: u64,
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
        let provider = ProviderBuilder::new()
            .wallet(wallet.ow_wallet.wallet)
            .connect_provider(root_provider);
        let contract = SEOA::new(
            Address::from_str(network.contract_address.as_str())?,
            &provider,
        );

        let tx_value = Uint::<256, 4>::from(tx_context.batch_tx_value);

        let fees = provider.estimate_eip1559_fees().await?;
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

        let new_execution_attempt = NewExecutionAttempt::build_standard(
            fees,
            gas_with_buffer,
            tx_hash,
            nonce,
            wallet.db_record.id,
            tx_context.chain_id,
            tx_context.batch_tx_value,
        )?;

        Ok(new_execution_attempt)
    }
}
