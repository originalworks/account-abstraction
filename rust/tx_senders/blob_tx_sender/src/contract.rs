use crate::transaction::BlobBatchTxContext;
use alloy::{
    consensus::BlobTransactionSidecarEip7594,
    eips::eip1559::Eip1559Estimation,
    primitives::Address,
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
use wallet_pool::wallet::Wallet;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug)]
    SEOA,
    "../../../contracts/artifacts/contracts/sEOA.sol/sEOA.json"
);

trait BuildNewBlobExecutionAttempt {
    fn build_for_blob_tx(
        fees: Eip1559Estimation,
        max_fee_per_blob_gas: u128,
        gas_limit: i64,
        tx_hash: String,
        nonce: u64,
        operator_wallet_id: Uuid,
        chain_id: i64,
        tx_value: i64,
    ) -> anyhow::Result<NewExecutionAttempt>;
}

impl BuildNewBlobExecutionAttempt for NewExecutionAttempt {
    fn build_for_blob_tx(
        fees: Eip1559Estimation,
        max_fee_per_blob_gas: u128,
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
            tx_type: TxType::BLOB,
            tx_hash,
            gas_limit,
            max_fee_per_gas: i64::try_from(fees.max_fee_per_gas)?,
            max_priority_fee: i64::try_from(fees.max_priority_fee_per_gas)?,
            max_fee_per_blob_gas: Some(i64::try_from(max_fee_per_blob_gas)?),
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

            providers_by_chain_id.insert(network.chain_id, provider);
        }

        Ok(Self {
            networks_by_chain_id,
            providers_by_chain_id,
        })
    }

    pub async fn send_blob_batch(
        &self,
        tx_context: &BlobBatchTxContext,
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

        let tx_input = tx_context
            .blob_batch_with_sidecar_vec
            .iter()
            .map(|entry| entry.blob_batch_input.clone())
            .collect();

        let tx_sidecar = Self::flat_sidecars(&tx_context)?;

        let fees = provider.estimate_eip1559_fees().await?;
        let max_fee_per_blob_gas = provider.get_blob_base_fee().await? * 2;
        let call_builder = contract
            .sendBlobBatch(tx_input)
            .sidecar_7594(tx_sidecar)
            .nonce(nonce)
            .max_fee_per_gas(fees.max_fee_per_gas)
            .max_priority_fee_per_gas(fees.max_priority_fee_per_gas)
            .max_fee_per_blob_gas(max_fee_per_blob_gas);

        let gas = i64::try_from(call_builder.estimate_gas().await?)?;
        let gas_with_buffer = gas + gas * network.gas_estimation_buffer_ppm / 1_000_000;

        let pending_tx = call_builder
            .gas(u64::try_from(gas_with_buffer)?)
            .send()
            .await?;

        let tx_hash = pending_tx.tx_hash().to_string();

        let new_execution_attempt = NewExecutionAttempt::build_for_blob_tx(
            fees,
            max_fee_per_blob_gas,
            gas_with_buffer,
            tx_hash,
            nonce,
            wallet.db_record.id,
            tx_context.chain_id,
            0,
        )?;

        Ok(new_execution_attempt)
    }

    fn flat_sidecars(
        tx_context: &BlobBatchTxContext,
    ) -> anyhow::Result<BlobTransactionSidecarEip7594> {
        let mut flat_sidecar = BlobTransactionSidecarEip7594::default();
        for blob_input in &tx_context.blob_batch_with_sidecar_vec {
            println!("blob_input before flattening: {blob_input:#?}");
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
}
