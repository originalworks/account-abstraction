#![cfg(feature = "aws")]
use std::{collections::HashMap, str::FromStr};

use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
use db_types::{TxExecutionOutcome, TxStatus};
use execution_attempt_db::{
    execution_attempts::{ExecutionAttemptRepo, NewExecutionAttempt},
    types::{ExecutionAttemptWithTxInputs, OutcomePropagationInput},
};
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use lambda_runtime::{LambdaEvent, tracing};
use network_db::networks::{Network, NetworkRepo};
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use receipt_poller_queue::ReceiptPollerQueueMessageBody;
use retry_queue::RetryEvent;
use seoa_contract::contract::ContractManager;
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use standard_tx_sender::execution_attempt::ExecutionAttemptFromStandardSuccessful;
use tx_request_db::repo::TxRequestRepo;
use uuid::Uuid;
use wallet_pool::manager::WalletPoolManager;

use crate::{
    Config,
    transaction::{FeeBufferExt, IntoExecuteBatchTxContext},
};

pub struct AwsLambdaOrchestrator {
    pub tx_request_repo: TxRequestRepo,
    pub execution_attempt_repo: ExecutionAttemptRepo,
    pub execution_attempt_item_repo: ExecutionAttemptItemRepo,
    pub wallet_pool_manager: WalletPoolManager,
    pub contract_manager: ContractManager,
    pub networks_by_chain_id: HashMap<i64, Network>,
    pub receipt_poller_queue: SqsQueue,
}

impl AwsLambdaOrchestrator {
    pub async fn build(
        pool: &sqlx::Pool<sqlx::Postgres>,
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<Self> {
        tracing::info!("Building retry_handler...");

        let config = Config::build()?;

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

        for network in networks {
            networks_by_chain_id.insert(network.chain_id, network.clone());
        }

        Ok(Self {
            execution_attempt_repo,
            wallet_pool_manager,
            contract_manager,
            networks_by_chain_id,
            execution_attempt_item_repo,
            tx_request_repo,
            receipt_poller_queue,
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
                println!(
                    "execution_attempt not found: {:?}",
                    queue_message.body.execution_attempt_id
                );
                continue;
            };

            if let Some(ref outcome) = execution_attempt.execution_attempt.outcome {
                match outcome {
                    TxExecutionOutcome::STUCK => self.handle_stuck(&execution_attempt).await?,
                    TxExecutionOutcome::DROPPED => self.handle_dropped(&execution_attempt)?,
                    TxExecutionOutcome::REVERTED => self.handle_reverted(&execution_attempt)?,
                    TxExecutionOutcome::FAILED | TxExecutionOutcome::SUCCEED => continue,
                }
            }
        }

        Ok(sqs_batch_response)
    }

    fn handle_dropped(
        &self,
        execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<()> {
        println!("handle_dropped: {execution_attempt:#?}");
        // recheck nonces of wallet and resend it with higher gas
        Ok(())
    }

    async fn handle_stuck(
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
        let pending_nonce = wallet.get_pending_nonce().await?;
        println!("pending: {pending_nonce}, latest: {latest_nonce}");
        let attempts = retried_execution_attempt.tx_requests[0].attempts;
        if attempts >= network.max_retry_attempts {
            self.execution_attempt_repo
                .propagate_outcome(&OutcomePropagationInput {
                    execution_attempt_id: retried_execution_attempt.execution_attempt.id,
                    outcome: TxExecutionOutcome::FAILED,
                    tx_requests_status: TxStatus::FAILED,
                    retryable: Some(false),
                    used_gas: retried_execution_attempt.execution_attempt.used_gas,
                })
                .await?;
        }
        if latest_nonce < pending_nonce {
            let mut tx_context = retried_execution_attempt.into_execute_batch_context()?;

            tx_context.apply_fee_buffer(u128::try_from(network.gas_estimation_buffer_ppm)?)?;

            match self
                .contract_manager
                .send_batch(&mut tx_context, &wallet)
                .await
            {
                Ok(_) => {
                    let execution_attempt_input =
                        NewExecutionAttempt::standard_successful(&tx_context, wallet.db_record.id)?;

                    let new_execution_attempt = self
                        .execution_attempt_repo
                        .insert(&execution_attempt_input)
                        .await?;

                    self.execution_attempt_repo
                        .update_retried_by(
                            &retried_execution_attempt.execution_attempt.id,
                            &new_execution_attempt.id,
                        )
                        .await?;

                    self.execution_attempt_item_repo
                        .insert_many(new_execution_attempt.id, &tx_context.get_tx_ids())
                        .await?;

                    self.tx_request_repo
                        .mark_many_as_broadcasted_and_bump_attempts(&tx_context.get_tx_ids())
                        .await?;

                    let receipt_poller_queue_message_body = ReceiptPollerQueueMessageBody {
                        execution_attempt_id: new_execution_attempt.id.to_string(),
                        batch_size: u8::try_from(tx_context.tx_requests.len())?,
                    };

                    self.receipt_poller_queue
                        .send_new(&receipt_poller_queue_message_body.to_json_string()?)
                        .await?;
                }
                Err(err) => {
                    println!("{err:#?}");
                    self.handle_error()?;
                }
            };
        } else {
            tracing::warn!(
                "No stuck transaction found for execution attempt: {retried_execution_attempt:?}"
            );
        }

        Ok(())
    }
    fn handle_error(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn handle_reverted(
        &self,
        execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<()> {
        println!("handle_reverted: {execution_attempt:#?}");
        // break batch into two and retry
        Ok(())
    }

    // async fn propagate_successful_tx(&self, )
}
