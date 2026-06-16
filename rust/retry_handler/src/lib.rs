use std::env;

use execution_attempt_db::{
    execution_attempts::ExecutionAttempt, types::ExecutionAttemptWithTxInputs,
};

pub struct Config {
    pub database_url: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let database_url = Self::get_env_var("DATABASE_URL");

        Ok(Self { database_url })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}

#[cfg(feature = "aws")]
pub mod aws_lambda {
    use std::str::FromStr;

    use aws_lambda_events::sqs::SqsEvent;
    use db_types::TxExecutionOutcome;
    use execution_attempt_db::execution_attempts::ExecutionAttemptRepo;
    use lambda_runtime::LambdaEvent;
    use retry_queue::RetryEvent;
    use uuid::Uuid;

    use crate::{handle_dropped, handle_failed, handle_reverted, handle_stuck};

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        println!("Building retry_handler...");

        let event = RetryEvent::from_sqs_lambda_event(event)?;

        println!("retry event received: {:?}", event);

        let execution_attempt_repo = ExecutionAttemptRepo::new(pool.clone());

        for queue_message in event.messages {
            let Some(execution_attempt) = execution_attempt_repo
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
                    TxExecutionOutcome::STUCK => handle_stuck(&execution_attempt)?,
                    TxExecutionOutcome::DROPPED => handle_dropped(&execution_attempt)?,
                    TxExecutionOutcome::FAILED => handle_failed(&execution_attempt)?,
                    TxExecutionOutcome::REVERTED => handle_reverted(&execution_attempt)?,
                    TxExecutionOutcome::SUCCEED => continue,
                }
            }
        }

        Ok(())
    }
}

fn handle_dropped(execution_attempt: &ExecutionAttemptWithTxInputs) -> anyhow::Result<()> {
    println!("handle_dropped: {execution_attempt:#?}");
    Ok(())
}

fn handle_stuck(execution_attempt: &ExecutionAttemptWithTxInputs) -> anyhow::Result<()> {
    println!("handle_stuck: {execution_attempt:#?}");
    Ok(())
}

fn handle_failed(execution_attempt: &ExecutionAttemptWithTxInputs) -> anyhow::Result<()> {
    println!("handle_failed: {execution_attempt:#?}");
    Ok(())
}

fn handle_reverted(execution_attempt: &ExecutionAttemptWithTxInputs) -> anyhow::Result<()> {
    println!("handle_reverted: {execution_attempt:#?}");
    Ok(())
}
