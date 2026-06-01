#![cfg(feature = "aws")]

use std::str::FromStr;

use aws_lambda_events::sqs::SqsEvent;
use db_types::TxStatus;
use execution_attempt_db::execution_attempts::{ExecutionAttemptRepo, TxExecutionOutcome};
use lambda_runtime::LambdaEvent;
use network_db::networks::NetworkRepo;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
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
        let retry_queue = SqsQueue::build(
            &sqs_client,
            &config.retry_queue_url,
            &config.retry_queue_message_group_id,
        )?;

        Ok(Self {
            execution_attempt_repo,
            receipt_reader,
            wallet_pool,
            retry_queue,
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
            let execution_attempt = self
                .execution_attempt_repo
                .find_by_id(execution_attempt_uuid)
                .await?;

            let execution_outcome = self
                .receipt_reader
                .check_execution_outcome(&execution_attempt)
                .await?;

            if let Some(outcome) = execution_outcome {
                match outcome {
                    TxExecutionOutcome::SUCCEED => {
                        self.execution_attempt_repo
                            .propagate_outcome(
                                execution_attempt.id,
                                outcome,
                                TxStatus::EXECUTED,
                                None,
                            )
                            .await?;

                        self.wallet_pool
                            .release_used(execution_attempt.operator_wallet_id)
                            .await?;
                    }
                    TxExecutionOutcome::FAILED => {
                        if queue_message.body.batch_size > 1 {
                            self.execution_attempt_repo
                                .propagate_outcome(
                                    execution_attempt.id,
                                    outcome,
                                    TxStatus::RETRIED,
                                    Some(true),
                                )
                                .await?;
                            let message_body = &RetryQueueMessageBody {
                                execution_attempt_id: execution_attempt.id.to_string(),
                            };
                            let message_body_string = message_body.to_json_string()?;
                            self.retry_queue.send_new(&message_body_string).await?;
                        } else {
                            self.execution_attempt_repo
                                .propagate_outcome(
                                    execution_attempt.id,
                                    outcome,
                                    TxStatus::FAILED,
                                    Some(false),
                                )
                                .await?;
                        }
                    }
                    TxExecutionOutcome::STUCK | TxExecutionOutcome::DROPPED => {
                        self.execution_attempt_repo
                            .propagate_outcome(
                                execution_attempt.id,
                                outcome,
                                TxStatus::RETRIED,
                                Some(true),
                            )
                            .await?;

                        let message_body = &RetryQueueMessageBody {
                            execution_attempt_id: execution_attempt.id.to_string(),
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
