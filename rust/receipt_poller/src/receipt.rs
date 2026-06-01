use alloy::{
    primitives::FixedBytes,
    providers::{
        Provider, ProviderBuilder,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
};
use anyhow::bail;
use execution_attempt_db::execution_attempts::{ExecutionAttempt, TxExecutionOutcome};
use network_db::networks::Network;
use sqlx::types::time::OffsetDateTime;
use std::str::FromStr;
use std::{collections::HashMap, time::Duration};

type HardlyTypedProvider = FillProvider<
    JoinFill<
        alloy::providers::Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    alloy::providers::RootProvider,
>;

pub struct ReceiptReader {
    providers_by_chain_id: HashMap<i64, HardlyTypedProvider>,
    tx_max_age_by_chain_id: HashMap<i64, i64>,
}

impl ReceiptReader {
    pub async fn build(networks: &Vec<Network>) -> anyhow::Result<Self> {
        let mut providers_by_chain_id = HashMap::new();
        let mut tx_max_age_by_chain_id = HashMap::new();
        for network in networks {
            let provider = ProviderBuilder::new().connect_http(network.rpc_url.parse()?);

            providers_by_chain_id.insert(network.chain_id, provider);
            tx_max_age_by_chain_id.insert(network.chain_id, network.tx_max_age_sec);
        }

        Ok(Self {
            tx_max_age_by_chain_id,
            providers_by_chain_id,
        })
    }

    pub async fn check_execution_outcome(
        &self,
        execution_attempt: &ExecutionAttempt,
    ) -> anyhow::Result<Option<TxExecutionOutcome>> {
        if let Some(outcome) = execution_attempt.outcome.clone() {
            return Ok(Some(outcome));
        }
        let tx_hash = FixedBytes::<32>::from_str(
            execution_attempt
                .tx_hash
                .clone()
                .expect("tx_hash is required")
                .as_str(),
        )?;

        let Some(provider) = self.providers_by_chain_id.get(&execution_attempt.chain_id) else {
            bail!(
                "Provider not found for chain_id: {}. Tx_hash: {}",
                &execution_attempt.chain_id,
                tx_hash
            );
        };

        if let Some(receipt) = provider.get_transaction_receipt(tx_hash).await? {
            if receipt.status() == true {
                return Ok(Some(TxExecutionOutcome::SUCCEED));
            } else {
                return Ok(Some(TxExecutionOutcome::FAILED));
            }
        } else {
            let tx_max_age = Duration::from_secs(u64::try_from(
                self.tx_max_age_by_chain_id
                    .get(&execution_attempt.chain_id)
                    .expect(
                        &format!(
                            "execution attempt with unrecognized chain id: {}",
                            execution_attempt.chain_id,
                        )
                        .to_string(),
                    )
                    .clone(),
            )?);
            if execution_attempt.created_at + tx_max_age < OffsetDateTime::now_utc() {
                if let Some(_) = provider.get_transaction_by_hash(tx_hash).await? {
                    return Ok(Some(TxExecutionOutcome::STUCK));
                } else {
                    return Ok(Some(TxExecutionOutcome::DROPPED));
                }
            } else {
                return Ok(None);
            }
        }
    }
}
