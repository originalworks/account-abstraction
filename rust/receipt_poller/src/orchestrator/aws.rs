#![cfg(feature = "aws")]

use std::str::FromStr;

use aws_lambda_events::sqs::SqsEvent;
use db_types::{TxExecutionOutcome, TxStatus};
use execution_attempt_db::{
    execution_attempts::ExecutionAttemptRepo, types::OutcomePropagationInput,
};
use lambda_runtime::{LambdaEvent, tracing};
use network_db::networks::NetworkRepo;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use outcome_emitter::{emitter::event_bridge::AwsEventBridgeOutcomeEmitter, outcome::OutcomeEvent};
use receipt_poller_queue::ReceiptPollerEvent;
use retry_queue::RetryQueueMessageBody;
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use wallet_pool::manager::WalletPoolManager;

use crate::{Config, receipt::ReceiptReader};

pub struct AwsLambdaOrchestrator {
    execution_attempt_repo: ExecutionAttemptRepo,
    receipt_reader: ReceiptReader,
    wallet_pool: WalletPoolManager,
    retry_queue: SqsQueue,
    outcome_emitter: AwsEventBridgeOutcomeEmitter,
}

impl AwsLambdaOrchestrator {
    pub async fn build(
        pool: &sqlx::Pool<sqlx::Postgres>,
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<Self> {
        let config = Config::build()?;
        let network_repo = NetworkRepo::new(pool.clone());
        let execution_attempt_repo = ExecutionAttemptRepo::new(pool.clone());
        let operator_wallet_repo = OperatorWalletRepo::new(pool.clone());
        let networks = network_repo.select_all().await?;

        let receipt_reader = ReceiptReader::build(&networks).await?;
        let wallet_pool = WalletPoolManager::build(operator_wallet_repo, &networks);
        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let event_bridge_client = aws_sdk_eventbridge::Client::new(&aws_config);
        let retry_queue = SqsQueue::build(
            &sqs_client,
            &config.retry_queue_url,
            &config.retry_queue_message_group_id,
        )?;
        let outcome_emitter = AwsEventBridgeOutcomeEmitter::build(
            &event_bridge_client,
            config.outcome_event_bus_name,
        );

        Ok(Self {
            execution_attempt_repo,
            receipt_reader,
            wallet_pool,
            retry_queue,
            outcome_emitter,
        })
    }

    pub async fn function_handler(
        &self,
        event: LambdaEvent<SqsEvent>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        let event = ReceiptPollerEvent::from_sqs_event(event)?;
        for queue_message in event.messages {
            let execution_attempt_uuid =
                uuid::Uuid::from_str(queue_message.body.execution_attempt_id.as_str())?;

            let Some(execution_attempt_with_txs) = self
                .execution_attempt_repo
                .select_with_txs(execution_attempt_uuid)
                .await?
            else {
                tracing::warn!("Execution attempt not found! {execution_attempt_uuid:?}");
                return Ok(());
            };

            if let Some(outcome_with_gas) = self
                .receipt_reader
                .check_execution(&execution_attempt_with_txs.execution_attempt)
                .await?
            {
                match outcome_with_gas.outcome {
                    TxExecutionOutcome::SUCCEED => {
                        let propagation_input = OutcomePropagationInput {
                            execution_attempt_id: execution_attempt_with_txs.execution_attempt.id,
                            outcome: outcome_with_gas.outcome.clone(),
                            tx_requests_status: TxStatus::EXECUTED,
                            retryable: None,
                            used_gas: outcome_with_gas.used_gas,
                        };
                        self.execution_attempt_repo
                            .propagate_outcome(&propagation_input)
                            .await?;

                        self.wallet_pool
                            .release_used(
                                execution_attempt_with_txs
                                    .execution_attempt
                                    .operator_wallet_id,
                            )
                            .await?;
                        self.outcome_emitter
                            .emit_for_execution_attempt(
                                &execution_attempt_with_txs,
                                &outcome_with_gas.outcome,
                                outcome_with_gas.used_gas,
                            )
                            .await?;
                    }
                    TxExecutionOutcome::FAILED => {
                        if queue_message.body.batch_size > 1 {
                            let propagation_input = OutcomePropagationInput {
                                execution_attempt_id: execution_attempt_with_txs
                                    .execution_attempt
                                    .id,
                                outcome: outcome_with_gas.outcome.clone(),
                                tx_requests_status: TxStatus::RETRIED,
                                retryable: Some(true),
                                used_gas: outcome_with_gas.used_gas,
                            };
                            self.execution_attempt_repo
                                .propagate_outcome(&propagation_input)
                                .await?;
                            let message_body = &RetryQueueMessageBody {
                                execution_attempt_id: execution_attempt_with_txs
                                    .execution_attempt
                                    .id
                                    .to_string(),
                            };
                            let message_body_string = message_body.to_json_string()?;
                            self.retry_queue.send_new(&message_body_string).await?;
                        } else {
                            let propagation_input = OutcomePropagationInput {
                                execution_attempt_id: execution_attempt_with_txs
                                    .execution_attempt
                                    .id,
                                outcome: outcome_with_gas.outcome.clone(),
                                tx_requests_status: TxStatus::FAILED,
                                retryable: Some(false),
                                used_gas: outcome_with_gas.used_gas,
                            };
                            self.execution_attempt_repo
                                .propagate_outcome(&propagation_input)
                                .await?;
                        }
                    }
                    TxExecutionOutcome::STUCK | TxExecutionOutcome::DROPPED => {
                        let propagation_input = OutcomePropagationInput {
                            execution_attempt_id: execution_attempt_with_txs.execution_attempt.id,
                            outcome: outcome_with_gas.outcome.clone(),
                            tx_requests_status: TxStatus::RETRIED,
                            retryable: Some(true),
                            used_gas: outcome_with_gas.used_gas,
                        };

                        self.execution_attempt_repo
                            .propagate_outcome(&propagation_input)
                            .await?;

                        let message_body = &RetryQueueMessageBody {
                            execution_attempt_id: execution_attempt_with_txs
                                .execution_attempt
                                .id
                                .to_string(),
                        };
                        let message_body_string = message_body.to_json_string()?;
                        self.retry_queue.send_new(&message_body_string).await?;
                    }
                    TxExecutionOutcome::REVERTED => {
                        continue;
                    }
                }
            }
        }

        Ok(())
    }
}
