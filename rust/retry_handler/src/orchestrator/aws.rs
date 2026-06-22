#![cfg(feature = "aws")]
use std::str::FromStr;

use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
use db_types::TxExecutionOutcome;
use execution_attempt_db::{
    execution_attempts::ExecutionAttemptRepo, types::ExecutionAttemptWithTxInputs,
};
use lambda_runtime::{LambdaEvent, tracing};
use retry_queue::RetryEvent;
use uuid::Uuid;

pub struct AwsLambdaOrchestrator {
    pub execution_attempt_repo: ExecutionAttemptRepo,
}

impl AwsLambdaOrchestrator {
    pub async fn build(
        pool: &sqlx::Pool<sqlx::Postgres>,
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<Self> {
        tracing::info!("Building retry_handler...");
        let execution_attempt_repo = ExecutionAttemptRepo::new(pool.clone());

        Ok(Self {
            execution_attempt_repo,
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
                    "execution_attepmt not found: {:?}",
                    queue_message.body.execution_attempt_id
                );
                continue;
            };

            if let Some(ref outcome) = execution_attempt.execution_attempt.outcome {
                match outcome {
                    TxExecutionOutcome::STUCK => self.handle_stuck(&execution_attempt)?,
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

    fn handle_stuck(&self, execution_attempt: &ExecutionAttemptWithTxInputs) -> anyhow::Result<()> {
        println!("handle_stuck: {execution_attempt:#?}");

        // resend with higher gas using the same nonce
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
}
