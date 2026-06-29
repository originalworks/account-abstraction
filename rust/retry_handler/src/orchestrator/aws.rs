#![cfg(feature = "aws")]
use crate::{
    Config,
    transaction::{FeeBufferExt, IntoExecuteBatchTxContext, calculate_batch_tx_value},
};
use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
use db_types::{TxExecutionOutcome, TxStatus};
use execution_attempt_db::{
    execution_attempts::{ExecutionAttempt, ExecutionAttemptRepo, NewExecutionAttempt},
    types::{ExecutionAttemptWithTxInputs, OutcomePropagationInput},
};
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use lambda_runtime::{LambdaEvent, tracing};
use network_db::networks::{Network, NetworkRepo};
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use outcome_emitter::{emitter::event_bridge::AwsEventBridgeOutcomeEmitter, outcome::OutcomeEvent};
use receipt_poller_queue::ReceiptPollerQueueMessageBody;
use retry_queue::RetryEvent;
use seoa_contract::{
    contract::{ContractManager, sEOA::ExecuteInput},
    transaction::{ExecuteBatchTxContext, IntoExecuteInput},
};
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use standard_tx_sender::{
    error::ExecutionErrorHandler, execution_attempt::ExecutionAttemptFromStandardSuccessful,
};
use std::{collections::HashMap, str::FromStr};
use tx_request_db::repo::TxRequestRepo;
use uuid::Uuid;
use wallet_assignment_db::wallet_assignments::WalletAssignmentRepo;
use wallet_pool::{manager::WalletPoolManager, wallet::Wallet};

pub struct AwsLambdaOrchestrator {
    pub wallet_assignment_repo: WalletAssignmentRepo,
    pub tx_request_repo: TxRequestRepo,
    pub execution_attempt_repo: ExecutionAttemptRepo,
    pub execution_attempt_item_repo: ExecutionAttemptItemRepo,
    pub wallet_pool_manager: WalletPoolManager,
    pub contract_manager: ContractManager,
    pub networks_by_chain_id: HashMap<i64, Network>,
    pub receipt_poller_queue: SqsQueue,
    pub retry_queue: SqsQueue,
    pub outcome_emitter: AwsEventBridgeOutcomeEmitter,
}

impl AwsLambdaOrchestrator {
    pub async fn build(
        pool: &sqlx::Pool<sqlx::Postgres>,
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<Self> {
        tracing::info!("Building retry_handler...");

        let config = Config::build()?;

        let wallet_assignment_repo = WalletAssignmentRepo::new(pool.clone());
        let execution_attempt_repo = ExecutionAttemptRepo::new(pool.clone());
        let execution_attempt_item_repo = ExecutionAttemptItemRepo::new(pool.clone());
        let tx_request_repo = TxRequestRepo::new(pool.clone());
        let operator_wallet_repo = OperatorWalletRepo::new(pool.clone());
        let network_repo = NetworkRepo::new(pool.clone());
        let networks = network_repo.select_all().await?;
        let wallet_pool_manager = WalletPoolManager::build(operator_wallet_repo, &networks);
        let contract_manager = ContractManager::build(&networks).await?;
        let mut networks_by_chain_id = HashMap::new();
        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let receipt_poller_queue = SqsQueue::build(
            &sqs_client,
            &config.receipt_poller_queue_url,
            &config.receipt_poller_queue_message_group_id,
        )?;

        let retry_queue = SqsQueue::build(
            &sqs_client,
            &config.retry_queue_url,
            &config.retry_queue_message_group_id,
        )?;

        let event_bridge_client = aws_sdk_eventbridge::Client::new(&aws_config);
        let outcome_emitter = AwsEventBridgeOutcomeEmitter::build(
            &event_bridge_client,
            config.outcome_event_bus_name,
        );

        for network in networks {
            networks_by_chain_id.insert(network.chain_id, network.clone());
        }

        Ok(Self {
            wallet_assignment_repo,
            execution_attempt_repo,
            wallet_pool_manager,
            contract_manager,
            networks_by_chain_id,
            execution_attempt_item_repo,
            tx_request_repo,
            receipt_poller_queue,
            outcome_emitter,
            retry_queue,
        })
    }

    pub async fn function_handler(
        &self,
        event: LambdaEvent<SqsEvent>,
    ) -> anyhow::Result<SqsBatchResponse, lambda_runtime::Error> {
        let mut sqs_batch_response = SqsBatchResponse::default();
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
                    TxExecutionOutcome::REVERTED => {
                        self.retry_reverted(
                            &execution_attempt,
                            &mut sqs_batch_response,
                            &queue_message.message_id,
                        )
                        .await?
                    }
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
            let mut tx_context = retried_execution_attempt.into_execute_batch_context()?;

            tx_context.apply_fee_buffer(u128::try_from(network.gas_estimation_buffer_ppm)?)?;

            match self
                .contract_manager
                .send_batch(&mut tx_context, &wallet)
                .await
            {
                Ok(_) => {
                    let new_execution_attempt = self
                        .save_successful_tx(
                            &tx_context,
                            &wallet,
                            &retried_execution_attempt.execution_attempt.id,
                        )
                        .await?;

                    self.send_receipt_poller_queue_message(
                        &tx_context,
                        &new_execution_attempt.id.to_string(),
                    )
                    .await?;
                }
                Err(err) => {
                    tracing::error!("{err:?}");
                    self.handle_error(&tx_context, &wallet, err).await?;
                }
            };
        } else {
            tracing::warn!(
                "No stuck transaction found for execution attempt: {retried_execution_attempt:?}"
            );
        }

        Ok(())
    }

    async fn save_successful_tx(
        &self,
        tx_context: &ExecuteBatchTxContext,
        wallet: &Wallet,
        retried_execution_attempt_id: &Uuid,
    ) -> anyhow::Result<ExecutionAttempt> {
        let execution_attempt_input = NewExecutionAttempt::standard_successful(
            &tx_context,
            wallet.db_record.id,
            Some(retried_execution_attempt_id.clone()),
        )?;

        let new_execution_attempt = self
            .execution_attempt_repo
            .insert(&execution_attempt_input)
            .await?;

        self.execution_attempt_item_repo
            .insert_many(new_execution_attempt.id, &tx_context.get_tx_ids())
            .await?;

        self.tx_request_repo
            .mark_many_as_broadcasted_and_bump_attempts(&tx_context.get_tx_ids())
            .await?;

        Ok(new_execution_attempt)
    }

    async fn send_receipt_poller_queue_message(
        &self,
        tx_context: &ExecuteBatchTxContext,
        execution_attempt_id: &String,
    ) -> anyhow::Result<()> {
        let receipt_poller_queue_message_body = ReceiptPollerQueueMessageBody {
            execution_attempt_id: execution_attempt_id.clone(),
            batch_size: u8::try_from(tx_context.tx_requests.len())?,
        };

        self.receipt_poller_queue
            .send_new(&receipt_poller_queue_message_body.to_json_string()?)
            .await?;
        Ok(())
    }

    fn split_into_execute_batch_context(
        &self,
        execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<Vec<ExecuteBatchTxContext>> {
        let mut execute_batch_contexts = Vec::new();

        let use_operator_wallet_id = execution_attempt.tx_requests[0].use_operator_wallet_id;

        let mid = execution_attempt.tx_requests.len().div_ceil(2);

        let (tx_request_batch_a, tx_request_batch_b) = execution_attempt.tx_requests.split_at(mid);

        let context_a = ExecuteBatchTxContext {
            chain_id: execution_attempt.execution_attempt.chain_id,
            use_operator_wallet_id,
            execute_batch_input: tx_request_batch_a
                .iter()
                .map(|tx_request| tx_request.into_execute_input())
                .collect::<anyhow::Result<Vec<ExecuteInput>>>()?,
            batch_tx_value: calculate_batch_tx_value(&tx_request_batch_a.to_vec())?,
            tx_requests: tx_request_batch_a.to_vec(),
            successfully_simulated: false,
            assigned_nonce: None,
            fees: None,
            gas_limit: None,
            tx_hash: None,
        };

        let context_b = ExecuteBatchTxContext {
            chain_id: execution_attempt.execution_attempt.chain_id,
            use_operator_wallet_id,
            execute_batch_input: tx_request_batch_b
                .iter()
                .map(|tx_request| tx_request.into_execute_input())
                .collect::<anyhow::Result<Vec<ExecuteInput>>>()?,
            batch_tx_value: calculate_batch_tx_value(&tx_request_batch_b.to_vec())?,
            tx_requests: tx_request_batch_b.to_vec(),
            successfully_simulated: false,
            assigned_nonce: None,
            fees: None,
            gas_limit: None,
            tx_hash: None,
        };

        execute_batch_contexts.push(context_a);
        execute_batch_contexts.push(context_b);

        Ok(execute_batch_contexts)
    }

    async fn retry_reverted(
        &self,
        retried_execution_attempt: &ExecutionAttemptWithTxInputs,
        sqs_batch_response: &mut SqsBatchResponse,
        queue_message_id: &String,
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
            let original_execution_id = retried_execution_attempt.execution_attempt.id;
            let split_execute_batch_context =
                self.split_into_execute_batch_context(retried_execution_attempt)?;

            for mut tx_context in split_execute_batch_context {
                let Some(mut wallet) = self
                    .wallet_pool_manager
                    .acquire(tx_context.chain_id, None)
                    .await?
                else {
                    sqs_batch_response.add_failure(queue_message_id);
                    continue;
                };

                let wallet_assignment_ids = self
                    .wallet_assignment_repo
                    .new_assignments(&tx_context.get_tx_ids(), wallet.db_record.id)
                    .await?;

                match self
                    .contract_manager
                    .simulate_send_batch_tx(&mut tx_context, &mut wallet)
                    .await
                {
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!("{err:?}");
                        self.wallet_pool_manager
                            .release_unused(wallet.db_record.id)
                            .await?;
                        let failed_execution_attempt =
                            self.handle_error(&tx_context, &wallet, err).await?;
                        self.execution_attempt_repo
                            .set_source_execution_attempt_id(
                                &failed_execution_attempt.id,
                                &retried_execution_attempt.execution_attempt.id,
                            )
                            .await?;
                        continue;
                    }
                };
                match self
                    .contract_manager
                    .send_batch(&mut tx_context, &wallet)
                    .await
                {
                    Ok(_) => {
                        let new_execution_attempt = self
                            .save_successful_tx(&tx_context, &wallet, &original_execution_id)
                            .await?;
                        self.send_receipt_poller_queue_message(
                            &tx_context,
                            &new_execution_attempt.id.to_string(),
                        )
                        .await?;
                    }
                    Err(err) => {
                        tracing::error!("{err:?}");
                        self.handle_error(&tx_context, &wallet, err).await?;
                    }
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
