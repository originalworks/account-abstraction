pub mod receipt;

use std::env;

pub struct Config {
    pub database_url: String,
    pub retry_queue_message_group_id: String,
    pub retry_queue_url: String,
    pub tx_max_age_sec: u64,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let database_url = Self::get_env_var("DATABASE_URL");
        let tx_max_age_sec = Self::get_env_var("TX_MAX_AGE_SEC").parse()?;
        let retry_queue_message_group_id = Self::get_env_var("RETRY_QUEUE_MESSAGE_GROUP_ID");
        let retry_queue_url = Self::get_env_var("RETRY_QUEUE_URL");

        Ok(Self {
            database_url,
            tx_max_age_sec,
            retry_queue_message_group_id,
            retry_queue_url,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}

#[cfg(feature = "aws")]
pub mod aws_lambda {
    use crate::{Config, receipt::ReceiptReader};
    use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};
    use aws_lambda_events::sqs::SqsEvent;
    use execution_attempt_db::execution_attempts::{ExecutionAttemptRepo, TxExecutionOutcome};
    use lambda_runtime::LambdaEvent;
    use network_db::networks::NetworkRepo;
    use operator_wallet_db::operator_wallets::OperatorWalletRepo;
    use receipt_poller_queue::queue::ReceiptPollerEvent;
    use retry_queue::queue::{RetryQueueMessageBody, sqs::RetrySqsQueue};
    use std::str::FromStr;
    use wallet_pool::manager::WalletPoolManager;

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        println!("Building...");
        let event = ReceiptPollerEvent::from_sqs_event(event)?;

        let config = Config::build()?;

        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        let network_repo = NetworkRepo::new(&pool);
        let execution_attempt_repo = ExecutionAttemptRepo::new(&pool);
        let operator_wallet_repo = OperatorWalletRepo::new(&pool);
        let networks = network_repo.select_all().await?;

        let receipt_reader = ReceiptReader::build(&networks, config.tx_max_age_sec).await?;
        let wallet_pool = WalletPoolManager::build(operator_wallet_repo, &networks);
        let retry_queue = RetrySqsQueue::build(
            &aws_config,
            &config.retry_queue_url,
            &config.retry_queue_message_group_id,
        )?;

        for queue_message in event.messages {
            let execution_attempt_uuid =
                uuid::Uuid::from_str(queue_message.body.execution_attempt_id.as_str())?;
            let execution_attempt = execution_attempt_repo
                .find_by_id(execution_attempt_uuid)
                .await?;

            let execution_outcome = receipt_reader
                .check_execution_outcome(&execution_attempt)
                .await?;

            if let Some(outcome) = execution_outcome {
                match outcome {
                    TxExecutionOutcome::SUCCEED => {
                        execution_attempt_repo
                            .propagate_success(execution_attempt.id, outcome)
                            .await?;

                        wallet_pool
                            .release(execution_attempt.operator_wallet_id)
                            .await?;
                    }
                    TxExecutionOutcome::DROPPED | TxExecutionOutcome::FAILED => {
                        retry_queue
                            .send_new(&RetryQueueMessageBody {
                                execution_attempt_id: execution_attempt.id.to_string(),
                            })
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }
}
