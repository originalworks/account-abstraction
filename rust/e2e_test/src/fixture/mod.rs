pub mod env_vars;
pub mod orchestrators;
pub mod repositories;
use crate::{
    aws::{
        config::build_aws_sdk_config, event_bridge::attach_outcome_event_bridge_to_queue,
        s3::S3BlobStorageManagerTestFeatures, sqs::TestQueueManager,
    },
    constants::{DEFAULT_TX_MAX_AGE_SEC, MAX_SCHEDULER_RUNS, SCHEDULER_INTERVAL_SEC},
    db::{get_pool, network::AnvilTestNetwork, operator_wallet::InsertFromMnemonic},
    fixture::{
        env_vars::E2eTestEnvVars, orchestrators::TestOrchestrators, repositories::DbRepositories,
    },
};
use anyhow::bail;
use aws_lambda_events::sqs::SqsEvent;
use blob_storage::storage::s3::S3BlobStorageManager;
use db_types::TxStatus;
use lambda_runtime::LambdaEvent;
use network_db::networks::NetworkRepo;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::OnceCell;

static STATIC_TEST_ENVIRONMENT: OnceCell<StaticTestEnvironment> = OnceCell::const_new();

pub struct StaticTestEnvironment {
    pub test_queue_manager: TestQueueManager,
    pub env_vars: E2eTestEnvVars,
    pub aws_config: aws_config::SdkConfig,
}

pub struct E2eTestFixture {
    pub test_queue_manager: TestQueueManager,
    pub db_repositories: DbRepositories,
    pub pool: PgPool,
    pub env_vars: E2eTestEnvVars,
    pub aws_config: aws_config::SdkConfig,
    pub orchestrators: TestOrchestrators,
}

impl E2eTestFixture {
    pub async fn build(pool: &PgPool) -> anyhow::Result<Self> {
        let static_test_environment = get_or_init_static_test_environment().await;

        Ok(E2eTestFixture {
            test_queue_manager: static_test_environment.test_queue_manager.clone(),
            db_repositories: DbRepositories::build(pool).await?,
            orchestrators: TestOrchestrators::build(pool, &static_test_environment.aws_config)
                .await?,
            env_vars: static_test_environment.env_vars.clone(),
            aws_config: static_test_environment.aws_config.clone(),
            pool: pool.clone(),
        })
    }
    pub async fn get_tx_status_by_id(&self, tx_id: &String) -> anyhow::Result<TxStatus> {
        Ok(self
            .db_repositories
            .tx_request_repo
            .find_by_tx_id(&tx_id)
            .await?
            .tx_status)
    }

    pub async fn poll_for_receipt_with_tx_max_age(
        &self,
        receipt_poller_queue_event: &LambdaEvent<SqsEvent>,
        tx_max_age_sec: i64,
    ) -> anyhow::Result<()> {
        let receipt_poller_orchestrator = self
            .get_receipt_poller_with_tx_max_age(tx_max_age_sec)
            .await?;
        match receipt_poller_orchestrator
            .sqs_event_handler(receipt_poller_queue_event.clone().payload)
            .await
        {
            Ok(_) => {}
            Err(err) => {
                println!("{err:#?}")
            }
        }
        Ok(())
    }

    pub async fn poll_for_receipt_with_scheduler(
        &self,
        wait_for_tx_id: &String,
        receipt_poller_queue_event: &LambdaEvent<SqsEvent>,
    ) -> anyhow::Result<()> {
        match self
            .orchestrators
            .receipt_poller_orchestrator
            .sqs_event_handler(receipt_poller_queue_event.clone().payload)
            .await
        {
            Ok(_) => {}
            Err(err) => {
                println!("{err:#?}")
            }
        }

        if self.get_tx_status_by_id(&wait_for_tx_id).await? != TxStatus::EXECUTED {
            self.run_receipt_poller_scheduler(
                &wait_for_tx_id,
                &self.orchestrators.receipt_poller_orchestrator,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn run_receipt_poller_scheduler(
        &self,
        wait_for_tx_id: &String,
        receipt_poller_orchestrator: &receipt_poller::orchestrator::aws::AwsLambdaOrchestrator,
    ) -> anyhow::Result<()> {
        for _ in 0..MAX_SCHEDULER_RUNS {
            match receipt_poller_orchestrator.scheduler_event_handler().await {
                Ok(_) => {}
                Err(err) => {
                    println!("{err:#?}")
                }
            }

            if self.get_tx_status_by_id(&wait_for_tx_id).await? == TxStatus::EXECUTED {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(SCHEDULER_INTERVAL_SEC)).await;
        }

        bail!("No receipt was found for {}", wait_for_tx_id)
    }

    pub async fn get_receipt_poller_with_tx_max_age(
        &self,
        tx_max_age_sec: i64,
    ) -> anyhow::Result<receipt_poller::orchestrator::aws::AwsLambdaOrchestrator> {
        self.db_repositories
            .network_repo
            .set_tx_max_age(tx_max_age_sec, self.env_vars.anvil_chain_id)
            .await?;
        // Get receipt_poller with new network data cached
        let receipt_poller = receipt_poller::orchestrator::aws::AwsLambdaOrchestrator::build(
            &self.pool,
            &self.aws_config,
        )
        .await?;
        self.db_repositories
            .network_repo
            .set_tx_max_age(DEFAULT_TX_MAX_AGE_SEC, self.env_vars.anvil_chain_id)
            .await?;

        Ok(receipt_poller)
    }
}

async fn get_or_init_static_test_environment() -> &'static StaticTestEnvironment {
    STATIC_TEST_ENVIRONMENT
        .get_or_init(|| async {
            let aws_config: aws_config::SdkConfig = build_aws_sdk_config().await.unwrap();
            let pool = get_pool().await.unwrap();

            let e2e_test_env_vars = E2eTestEnvVars::build().unwrap();
            let operator_wallet_repo = OperatorWalletRepo::new(pool.clone());
            let network_repo = NetworkRepo::new(pool.clone());

            let seoa_address = e2e_test_env_vars.get_seoa_address().unwrap();
            network_repo
                .add_anvil(seoa_address.to_string(), e2e_test_env_vars.anvil_chain_id)
                .await
                .unwrap();
            operator_wallet_repo
                .insert_from_mnemonic(
                    &e2e_test_env_vars.anvil_mnemonic,
                    e2e_test_env_vars.anvil_chain_id,
                    5,
                )
                .await
                .unwrap();

            let test_queue_manager = TestQueueManager::build(&aws_config)
                .await
                .expect("Failed to build TestQueueManager");

            attach_outcome_event_bridge_to_queue(
                &aws_config,
                &e2e_test_env_vars.outcome_event_bus_name,
                &test_queue_manager.tx_outcome_queue,
            )
            .await
            .unwrap();

            let blob_storage_manager = S3BlobStorageManager::build(
                &aws_config,
                &e2e_test_env_vars.blob_storage_bucket_name,
            );

            blob_storage_manager.prepare_for_test().await.unwrap();

            StaticTestEnvironment {
                test_queue_manager,
                env_vars: e2e_test_env_vars,
                aws_config,
            }
        })
        .await
}
