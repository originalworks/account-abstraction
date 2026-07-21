use crate::transaction::{BlobBatchTxContext, ExecuteBatchTxContext};
use alloy::consensus::BlobTransactionSidecarEip7594;
use alloy::eips::eip4844::calc_blob_gasprice;
use alloy::{
    primitives::{Address, Uint},
    providers::{
        Provider, ProviderBuilder,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
    sol,
};
use anyhow::bail;
use network_db::networks::Network;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::{collections::HashMap, str::FromStr};
use wallet_pool::wallet::Wallet;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug, Deserialize, Serialize)]
    SEOA,
    "../../contracts/artifacts/contracts/sEOA.sol/sEOA.json"
);

type HardlyTypedProvider = FillProvider<
    JoinFill<
        alloy::providers::Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    alloy::providers::RootProvider,
>;

const GNOSIS_MIN_BLOB_GAS_PRICE: u128 = 1_000_000_000;

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
    pub async fn get_blob_gasprice(
        &self,
        root_provider: &HardlyTypedProvider,
        network: &Network,
    ) -> anyhow::Result<u128> {
        let provider = ProviderBuilder::new().connect_provider(root_provider);

        let block = provider
            .get_block_by_number(alloy::rpc::types::BlockNumberOrTag::Latest)
            .await?
            .unwrap();

        let excess_blob_gas = block.header.excess_blob_gas.unwrap_or(1);

        let mut blob_base_fee = calc_blob_gasprice(excess_blob_gas);

        // detects Gnosis Mainnet and Chiado testnet, where min. blob gas price is 1 Gwei
        if network.chain_id == 100 || network.chain_id == 10200 {
            blob_base_fee = max(blob_base_fee, GNOSIS_MIN_BLOB_GAS_PRICE);
        }
        let max_fee_per_blob_gas = blob_base_fee
            + blob_base_fee * u128::try_from(network.blob_gas_estimation_buffer_ppm)? / 1_000_000;
        Ok(max_fee_per_blob_gas)
    }

    fn flat_sidecars(
        tx_context: &BlobBatchTxContext,
    ) -> anyhow::Result<BlobTransactionSidecarEip7594> {
        let mut flat_sidecar = BlobTransactionSidecarEip7594::default();
        for blob_input in &tx_context.blob_batch_with_sidecar_vec {
            if blob_input.sidecar.blobs.len() != 1 {
                bail!(
                    "Expecting one BLOB per tx request, got: {}",
                    blob_input.sidecar.blobs.len()
                );
            }
            flat_sidecar.blobs.push(
                blob_input
                    .sidecar
                    .blobs
                    .first()
                    .expect("No BLOB in the input")
                    .clone(),
            );
            flat_sidecar.commitments.push(
                blob_input
                    .sidecar
                    .commitments
                    .first()
                    .expect("No commitments in the input")
                    .clone(),
            );
            flat_sidecar
                .cell_proofs
                .extend_from_slice(&blob_input.sidecar.cell_proofs);
        }
        Ok(flat_sidecar)
    }

    pub async fn simulate_send_blob_batch(
        &self,
        tx_context: &mut BlobBatchTxContext,
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

        let fees = provider.estimate_eip1559_fees().await?;
        let max_fee_per_blob_gas = self.get_blob_gasprice(root_provider, &network).await?;

        // let max_fee_per_blob_gas = 5000000000;

        println!("++-- max_fee_per_blob_gas applied at the end: {max_fee_per_blob_gas}");

        let tx_sidecar = Self::flat_sidecars(&tx_context)?;

        let call = contract
            .sendBlobBatch(
                tx_context
                    .blob_batch_with_sidecar_vec
                    .iter()
                    .map(|c| c.blob_batch_input.clone())
                    .collect(),
            )
            .sidecar_7594(tx_sidecar)
            .nonce(nonce)
            .max_fee_per_gas(fees.max_fee_per_gas)
            .max_priority_fee_per_gas(fees.max_priority_fee_per_gas)
            .max_fee_per_blob_gas(max_fee_per_blob_gas);

        let estimated_gas = call.estimate_gas().await?;

        let gas_limit = estimated_gas
            + estimated_gas * u64::try_from(network.gas_estimation_buffer_ppm)? / 1_000_000;

        call.gas(gas_limit).call().await?;

        tx_context.assigned_nonce = Some(nonce);
        tx_context.fees = Some(fees);
        tx_context.gas_limit = Some(gas_limit);
        tx_context.max_fee_per_blob_gas = Some(max_fee_per_blob_gas);
        tx_context.successfully_simulated = true;

        Ok(())
    }

    pub async fn send_blob_batch(
        &self,
        tx_context: &mut BlobBatchTxContext,
        wallet: &Wallet,
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

        let Some(nonce) = tx_context.assigned_nonce else {
            bail!("Nonce should be assinged at this point. Use simulate_send_batch_tx first");
        };

        let Some(fees) = tx_context.fees else {
            bail!("Fees should be calculated at this point. Use simulate_send_batch_tx first");
        };

        let Some(max_fee_per_blob_gas) = tx_context.max_fee_per_blob_gas else {
            bail!(
                "max_fee_per_blob_gas should be calculated at this point. Use simulate_send_batch_tx first"
            );
        };

        let Some(gas_limit) = tx_context.gas_limit else {
            bail!("Gas limit should be calculated at this point. Use simulate_send_batch_tx first");
        };

        let tx_sidecar = Self::flat_sidecars(&tx_context)?;

        let provider = ProviderBuilder::new()
            .wallet(wallet.ow_wallet.wallet.clone())
            .connect_provider(root_provider);
        let contract = SEOA::new(
            Address::from_str(network.contract_address.as_str())?,
            &provider,
        );

        let pending_tx = contract
            .sendBlobBatch(
                tx_context
                    .blob_batch_with_sidecar_vec
                    .iter()
                    .map(|c| c.blob_batch_input.clone())
                    .collect(),
            )
            .sidecar_7594(tx_sidecar)
            .nonce(nonce)
            .max_fee_per_gas(fees.max_fee_per_gas)
            .max_priority_fee_per_gas(fees.max_priority_fee_per_gas)
            .max_fee_per_blob_gas(max_fee_per_blob_gas)
            .gas(gas_limit)
            .send()
            .await?;

        tx_context.tx_hash = Some(pending_tx.tx_hash().to_string());

        Ok(())
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

        let pending_tx = contract
            .executeBatch(tx_context.execute_batch_input.clone())
            .value(Uint::<256, 4>::from(tx_context.batch_tx_value))
            .nonce(nonce)
            .max_fee_per_gas(fees.max_fee_per_gas)
            .max_priority_fee_per_gas(fees.max_priority_fee_per_gas)
            .gas(gas_limit)
            .send()
            .await?;

        tx_context.tx_hash = Some(pending_tx.tx_hash().to_string());

        Ok(())
    }
}
