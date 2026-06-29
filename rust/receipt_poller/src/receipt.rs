use alloy::{
    network::ReceiptResponse,
    primitives::FixedBytes,
    providers::{
        Provider, ProviderBuilder,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
};
use anyhow::bail;
use db_types::TxExecutionOutcome;
use execution_attempt_db::execution_attempts::ExecutionAttempt;
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

#[derive(Debug, Clone)]
pub struct OutcomeWithGas {
    pub outcome: TxExecutionOutcome,
    pub used_gas: Option<i64>,
}

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

    pub async fn check_execution(
        &self,
        execution_attempt: &ExecutionAttempt,
    ) -> anyhow::Result<Option<OutcomeWithGas>> {
        let Some(tx_hash) = execution_attempt.tx_hash.clone() else {
            return Ok(None);
        };
        if execution_attempt.outcome.is_some() {
            // already resolved by different worker/execution
            return Ok(None);
        }
        let tx_hash = FixedBytes::<32>::from_str(tx_hash.as_str())?;

        let Some(provider) = self.providers_by_chain_id.get(&execution_attempt.chain_id) else {
            bail!(
                "Provider not found for chain_id: {}. Tx_hash: {}",
                &execution_attempt.chain_id,
                tx_hash
            );
        };

        if let Some(receipt) = provider.get_transaction_receipt(tx_hash).await? {
            let used_gas = Some(i64::try_from(receipt.gas_used())?);
            if receipt.status() == true {
                return Ok(Some(OutcomeWithGas {
                    outcome: TxExecutionOutcome::SUCCEED,
                    used_gas,
                }));
            } else {
                return Ok(Some(OutcomeWithGas {
                    outcome: TxExecutionOutcome::FAILED,
                    used_gas,
                }));
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
                    return Ok(Some(OutcomeWithGas {
                        outcome: TxExecutionOutcome::STUCK,
                        used_gas: None,
                    }));
                } else {
                    return Ok(Some(OutcomeWithGas {
                        outcome: TxExecutionOutcome::DROPPED,
                        used_gas: None,
                    }));
                }
            } else {
                return Ok(None);
            }
        }
    }
}
