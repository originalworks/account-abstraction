use aws_config::SdkConfig;
use sqlx::PgPool;

pub struct TestOrchestrators {
    pub standard_tx_sender_orchestrator:
        standard_tx_sender::orchestrator::aws::AwsLambdaOrchestrator,
    pub blob_tx_sender_orchestrator: blob_tx_sender::orchestrator::aws::AwsLambdaOrchestrator,
    pub receipt_poller_orchestrator: receipt_poller::orchestrator::aws::AwsLambdaOrchestrator,
    pub retry_handler_orchestrator: retry_handler::orchestrator::aws::AwsLambdaOrchestrator,
}

impl TestOrchestrators {
    pub async fn build(pool: &PgPool, aws_config: &SdkConfig) -> anyhow::Result<Self> {
        let orchestrators = TestOrchestrators {
            standard_tx_sender_orchestrator:
                standard_tx_sender::orchestrator::aws::AwsLambdaOrchestrator::build(
                    &pool,
                    &aws_config,
                )
                .await
                .unwrap(),

            blob_tx_sender_orchestrator:
                blob_tx_sender::orchestrator::aws::AwsLambdaOrchestrator::build(&pool, &aws_config)
                    .await
                    .unwrap(),

            receipt_poller_orchestrator:
                receipt_poller::orchestrator::aws::AwsLambdaOrchestrator::build(&pool, &aws_config)
                    .await
                    .unwrap(),

            retry_handler_orchestrator:
                retry_handler::orchestrator::aws::AwsLambdaOrchestrator::build(&pool, &aws_config)
                    .await
                    .unwrap(),
        };
        Ok(orchestrators)
    }
}
