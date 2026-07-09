#![cfg(feature = "aws")]
use crate::{
    Config,
    transaction::{
        FeeBufferExt,
        blob_tx::BlobTxRetryManager,
        standard_tx::{IntoExecuteBatchTxContext, StandardTxRetryManager},
    },
};
use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
use db_types::{TxExecutionOutcome, TxStatus, TxType};
use execution_attempt_db::{
    execution_attempts::ExecutionAttemptRepo,
    types::{ExecutionAttemptWithTxInputs, OutcomePropagationInput},
};
use lambda_runtime::{LambdaEvent, tracing};
use network_db::networks::{Network, NetworkRepo};
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use outcome_emitter::{emitter::event_bridge::AwsEventBridgeOutcomeEmitter, outcome::OutcomeEvent};
use retry_queue::RetryEvent;
use seoa_contract::contract::ContractManager;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use uuid::Uuid;
use wallet_pool::manager::WalletPoolManager;

pub struct AwsLambdaOrchestrator {
    pub execution_attempt_repo: ExecutionAttemptRepo,
    pub wallet_pool_manager: Arc<WalletPoolManager>,
    pub networks_by_chain_id: HashMap<i64, Network>,
    pub outcome_emitter: AwsEventBridgeOutcomeEmitter,
    pub standard_tx_retry_manager: StandardTxRetryManager,
    pub blob_tx_retry_manager: BlobTxRetryManager,
}

impl AwsLambdaOrchestrator {
    pub async fn build(
        pool: &sqlx::Pool<sqlx::Postgres>,
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<Self> {
        tracing::info!("Building retry_handler...");

        let config = Config::build()?;
        let execution_attempt_repo = ExecutionAttemptRepo::new(pool.clone());
        let operator_wallet_repo = OperatorWalletRepo::new(pool.clone());
        let network_repo = NetworkRepo::new(pool.clone());
        let networks = network_repo.select_all().await?;
        let wallet_pool_manager =
            Arc::new(WalletPoolManager::build(operator_wallet_repo, &networks));
        let contract_manager = Arc::new(ContractManager::build(&networks).await?);

        let mut networks_by_chain_id = HashMap::new();

        let event_bridge_client = aws_sdk_eventbridge::Client::new(&aws_config);
        let outcome_emitter = AwsEventBridgeOutcomeEmitter::build(
            &event_bridge_client,
            &config.outcome_event_bus_name,
        );

        for network in networks {
            networks_by_chain_id.insert(network.chain_id, network.clone());
        }

        let standard_tx_retry_manager = StandardTxRetryManager::build(
            &contract_manager,
            &wallet_pool_manager,
            aws_config,
            pool,
            &config,
        )?;
        let blob_tx_retry_manager = BlobTxRetryManager::build(
            &contract_manager,
            &wallet_pool_manager,
            aws_config,
            pool,
            &config,
        )?;

        Ok(Self {
            execution_attempt_repo,
            standard_tx_retry_manager,
            wallet_pool_manager,
            networks_by_chain_id,
            outcome_emitter,
            blob_tx_retry_manager,
        })
    }

    pub async fn function_handler(
        &self,
        event: LambdaEvent<SqsEvent>,
    ) -> anyhow::Result<SqsBatchResponse, lambda_runtime::Error> {
        let sqs_batch_response = SqsBatchResponse::default();
        tracing::info!("Reading...");

        let event = RetryEvent::from_sqs_lambda_event(event)?;

        tracing::info!("Executing...");

        for queue_message in event.messages {
            let Some(execution_attempt) = self
                .execution_attempt_repo
                .select_and_lock_for_retry(Uuid::from_str(
                    queue_message.body.execution_attempt_id.as_str(),
                )?)
                .await?
            else {
                tracing::warn!(
                    "execution_attempt not found: {:?}",
                    queue_message.body.execution_attempt_id
                );
                continue;
            };

            if let Some(ref outcome) = execution_attempt.execution_attempt.outcome {
                match outcome {
                    TxExecutionOutcome::STUCK | TxExecutionOutcome::DROPPED => {
                        self.retry_stuck_or_dropped(&execution_attempt).await?
                    }
                    TxExecutionOutcome::REVERTED => self.retry_reverted(&execution_attempt).await?,
                    TxExecutionOutcome::FAILED | TxExecutionOutcome::SUCCEED => continue,
                }
            }
        }

        Ok(sqs_batch_response)
    }

    async fn retry_stuck_or_dropped(
        &self,
        retried_execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<()> {
        let network = self
            .networks_by_chain_id
            .get(&retried_execution_attempt.execution_attempt.chain_id)
            .ok_or(anyhow::anyhow!("Network not found"))?;
        let wallet = self
            .wallet_pool_manager
            .get_by_id(
                retried_execution_attempt
                    .execution_attempt
                    .operator_wallet_id,
            )
            .await?;

        let latest_nonce = wallet.get_latest_nonce().await?;
        let retried_execution_nonce = u64::try_from(
            retried_execution_attempt
                .execution_attempt
                .nonce_used
                .ok_or(anyhow::anyhow!(
                    "Stuck/Dropped executions should have nonce"
                ))?,
        )?;

        if latest_nonce == retried_execution_nonce {
            match retried_execution_attempt.execution_attempt.tx_type {
                TxType::BLOB => {
                    let mut tx_context = self
                        .blob_tx_retry_manager
                        .recreate_blob_batch_context(retried_execution_attempt)
                        .await?;

                    tx_context.apply_fee_buffer(&network)?;

                    self.blob_tx_retry_manager
                        .send_retry(
                            &mut tx_context,
                            &wallet,
                            &retried_execution_attempt.execution_attempt.id,
                        )
                        .await?;
                }
                TxType::STANDARD => {
                    let mut tx_context = retried_execution_attempt.into_execute_batch_context()?;

                    tx_context.apply_fee_buffer(&network)?;

                    self.standard_tx_retry_manager
                        .send_retry(
                            &mut tx_context,
                            &wallet,
                            &retried_execution_attempt.execution_attempt.id,
                        )
                        .await?;
                }
            }
        } else {
            tracing::warn!(
                "No stuck transaction found for execution attempt: {retried_execution_attempt:?}"
            );
        }

        Ok(())
    }

    async fn retry_reverted(
        &self,
        retried_execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<()> {
        if retried_execution_attempt.tx_requests[0]
            .use_operator_wallet_id
            .is_some()
        {
            tracing::warn!(
                "Can't handle reverted execution with use_operator_wallet_id. Marking as FAILED..."
            );
            self.execution_attempt_repo
                .propagate_outcome(&OutcomePropagationInput {
                    execution_attempt_id: retried_execution_attempt.execution_attempt.id,
                    outcome: TxExecutionOutcome::FAILED,
                    tx_requests_status: TxStatus::FAILED,
                    retryable: Some(false),
                    used_gas: retried_execution_attempt.execution_attempt.used_gas,
                })
                .await?;
            for tx_request in retried_execution_attempt.tx_requests.clone() {
                self.outcome_emitter
                    .emit_outcome(&OutcomeEvent {
                        outcome: TxExecutionOutcome::FAILED,
                        tx_request_id: tx_request.tx_id,
                        gas_fee: retried_execution_attempt.execution_attempt.used_gas,
                        transaction_hash: retried_execution_attempt
                            .execution_attempt
                            .tx_hash
                            .clone(),
                        error: retried_execution_attempt
                            .execution_attempt
                            .error_object
                            .clone(),
                        metadata: tx_request.metadata,
                    })
                    .await?;
            }
            return Ok(());
        }
        if retried_execution_attempt.tx_requests.len() > 1 {
            match retried_execution_attempt.execution_attempt.tx_type {
                TxType::BLOB => {
                    self.blob_tx_retry_manager
                        .split_blob_batch_and_retry(retried_execution_attempt)
                        .await?;
                }
                TxType::STANDARD => {
                    self.standard_tx_retry_manager
                        .split_batch_and_retry(retried_execution_attempt)
                        .await?;
                }
            }
        } else {
            tracing::warn!(
                "Can't handle reverted execution with only one tx. Marking as FAILED..."
            );
            self.execution_attempt_repo
                .propagate_outcome(&OutcomePropagationInput {
                    execution_attempt_id: retried_execution_attempt.execution_attempt.id,
                    outcome: TxExecutionOutcome::FAILED,
                    tx_requests_status: TxStatus::FAILED,
                    retryable: Some(false),
                    used_gas: retried_execution_attempt.execution_attempt.used_gas,
                })
                .await?;

            for tx_request in retried_execution_attempt.tx_requests.clone() {
                self.outcome_emitter
                    .emit_outcome(&OutcomeEvent {
                        outcome: TxExecutionOutcome::FAILED,
                        tx_request_id: tx_request.tx_id,
                        gas_fee: retried_execution_attempt.execution_attempt.used_gas,
                        transaction_hash: retried_execution_attempt
                            .execution_attempt
                            .tx_hash
                            .clone(),
                        error: retried_execution_attempt
                            .execution_attempt
                            .error_object
                            .clone(),
                        metadata: tx_request.metadata,
                    })
                    .await?;
            }
        }
        Ok(())
    }
}
