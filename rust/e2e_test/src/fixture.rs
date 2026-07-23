use std::time::Duration;

use crate::{
    aws::{
        config::build_aws_sdk_config, event_bridge::attach_outcome_event_bridge_to_queue,
        s3::S3BlobStorageManagerTestFeatures, sqs::TestQueueManager,
    },
    constants::{DEFAULT_TX_MAX_AGE_SEC, MAX_SCHEDULER_RUNS, SCHEDULER_INTERVAL_SEC},
    db::{get_pool, network::AnvilTestNetwork, operator_wallet::InsertFromMnemonic},
};
use alloy::{primitives::Address, signers::local::PrivateKeySigner};
use anyhow::bail;
use aws_lambda_events::sqs::SqsEvent;
use blob_storage::storage::s3::S3BlobStorageManager;
use blob_tx_input_db::blob_tx_inputs::BlobTxInputRepo;
use db_types::TxStatus;
use execution_attempt_db::execution_attempts::ExecutionAttemptRepo;
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use lambda_runtime::LambdaEvent;
use network_db::networks::NetworkRepo;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use sqlx::PgPool;
use standard_tx_input_db::standard_tx_inputs::StandardTxInputRepo;
use tokio::sync::OnceCell;
use tx_request_db::repo::TxRequestRepo;
use wallet_assignment_db::wallet_assignments::WalletAssignmentRepo;

static E2E_TEST_FIXTURE: OnceCell<E2eTestFixture> = OnceCell::const_new();

pub struct DbRepositories {
    pub network_repo: NetworkRepo,
    pub standard_tx_input_repo: StandardTxInputRepo,
    pub blob_tx_input_repo: BlobTxInputRepo,
    pub operator_wallet_repo: OperatorWalletRepo,
    pub tx_request_repo: TxRequestRepo,
    pub execution_attempt_repo: ExecutionAttemptRepo,
    pub execution_attempt_item_repo: ExecutionAttemptItemRepo,
    pub wallet_assignment_repo: WalletAssignmentRepo,
}

pub struct TestOrchestrators {
    pub standard_tx_sender_orchestrator:
        standard_tx_sender::orchestrator::aws::AwsLambdaOrchestrator,
    pub blob_tx_sender_orchestrator: blob_tx_sender::orchestrator::aws::AwsLambdaOrchestrator,
    pub receipt_poller_orchestrator: receipt_poller::orchestrator::aws::AwsLambdaOrchestrator,
    pub retry_handler_orchestrator: retry_handler::orchestrator::aws::AwsLambdaOrchestrator,
}

pub struct E2eTestFixture {
    pub test_queue_manager: TestQueueManager,
    pub db_repositories: DbRepositories,
    pub pool: PgPool,
    pub env_vars: E2eTestEnvVars,
    pub blob_storage_manager: S3BlobStorageManager,
    pub aws_config: aws_config::SdkConfig,
    pub orchestrators: TestOrchestrators,
}

impl E2eTestFixture {
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
        // with_tx_max_age_override: Option<i64>,
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
        // if let Some(tx_max_age_sec) = with_tx_max_age_override {
        //     let receipt_poller_orchestrator = self
        //         .get_receipt_poller_with_tx_max_age(tx_max_age_sec)
        //         .await?;
        //     match receipt_poller_orchestrator
        //         .sqs_event_handler(receipt_poller_queue_event.clone().payload)
        //         .await
        //     {
        //         Ok(_) => {}
        //         Err(err) => {
        //             println!("{err:#?}")
        //         }
        //     }

        //     if self.get_tx_status_by_id(&wait_for_tx_id).await? != TxStatus::EXECUTED {
        //         self.run_receipt_poller_scheduler(&wait_for_tx_id, &receipt_poller_orchestrator)
        //             .await?;
        //     }
        // } else {
        //     match self
        //         .orchestrators
        //         .receipt_poller_orchestrator
        //         .sqs_event_handler(receipt_poller_queue_event.clone().payload)
        //         .await
        //     {
        //         Ok(_) => {}
        //         Err(err) => {
        //             println!("{err:#?}")
        //         }
        //     }

        //     if self.get_tx_status_by_id(&wait_for_tx_id).await? != TxStatus::EXECUTED {
        //         self.run_receipt_poller_scheduler(
        //             &wait_for_tx_id,
        //             &self.orchestrators.receipt_poller_orchestrator,
        //         )
        //         .await?;
        //     }
        // }
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

    // pub async fn set_default_tx_max_age(&mut self) -> anyhow::Result<()> {
    //     self.db_repositories
    //         .network_repo
    //         .set_tx_max_age(DEFAULT_TX_MAX_AGE_SEC, self.env_vars.anvil_chain_id)
    //         .await?;

    //     // Can't use receipt_poller from e2e_test_fixture because it has old network data cached
    //     let receipt_poller = receipt_poller::orchestrator::aws::AwsLambdaOrchestrator::build(
    //         &self.pool,
    //         &self.aws_config,
    //     )
    //     .await?;
    //     self.orchestrators.receipt_poller_orchestrator = receipt_poller;
    //     Ok(())
    // }
}

pub struct E2eTestEnvVars {
    pub anvil_chain_id: i64,
    pub anvil_mnemonic: String,
    pub blob_storage_bucket_name: String,
    outcome_event_bus_name: String,
}

pub async fn get_e2e_test_fixture() -> &'static E2eTestFixture {
    E2E_TEST_FIXTURE
        .get_or_init(|| async {
            let aws_config: aws_config::SdkConfig = build_aws_sdk_config().await.unwrap();
            let pool = get_pool().await.unwrap();

            let e2e_test_env_vars = build_env_vars().unwrap();
            let db_repositories = build_db_repositories(&pool, &e2e_test_env_vars)
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

            let orchestrators = TestOrchestrators {
                standard_tx_sender_orchestrator:
                    standard_tx_sender::orchestrator::aws::AwsLambdaOrchestrator::build(
                        &pool,
                        &aws_config,
                    )
                    .await
                    .unwrap(),

                blob_tx_sender_orchestrator:
                    blob_tx_sender::orchestrator::aws::AwsLambdaOrchestrator::build(
                        &pool,
                        &aws_config,
                    )
                    .await
                    .unwrap(),

                receipt_poller_orchestrator:
                    receipt_poller::orchestrator::aws::AwsLambdaOrchestrator::build(
                        &pool,
                        &aws_config,
                    )
                    .await
                    .unwrap(),

                retry_handler_orchestrator:
                    retry_handler::orchestrator::aws::AwsLambdaOrchestrator::build(
                        &pool,
                        &aws_config,
                    )
                    .await
                    .unwrap(),
            };

            E2eTestFixture {
                pool,
                test_queue_manager,
                db_repositories,
                env_vars: e2e_test_env_vars,
                blob_storage_manager,
                aws_config,
                orchestrators,
            }
        })
        .await
}

pub fn get_seoa_address() -> anyhow::Result<Address> {
    let seoa_private_key = std::env::var("PRIVATE_KEY").unwrap();
    let pk_signer: PrivateKeySigner = seoa_private_key.parse().unwrap();

    Ok(pk_signer.address())
}

fn build_env_vars() -> anyhow::Result<E2eTestEnvVars> {
    let anvil_chain_id = std::env::var("ANVIL_CHAIN_ID").unwrap().parse().unwrap();
    let anvil_mnemonic = std::env::var("ANVIL_MNEMONIC").unwrap();
    let blob_storage_bucket_name = std::env::var("BLOB_STORAGE_BUCKET_NAME").unwrap();
    let outcome_event_bus_name = std::env::var("OUTCOME_EVENT_BUS_NAME").unwrap();

    Ok(E2eTestEnvVars {
        anvil_chain_id,
        anvil_mnemonic,
        blob_storage_bucket_name,
        outcome_event_bus_name,
    })
}

async fn build_db_repositories(
    pool: &PgPool,
    env_vars: &E2eTestEnvVars,
) -> anyhow::Result<DbRepositories> {
    let network_repo = NetworkRepo::new(pool.clone());
    let standard_tx_input_repo = StandardTxInputRepo::new(pool.clone());
    let blob_tx_input_repo = BlobTxInputRepo::new(pool.clone());
    let operator_wallet_repo = OperatorWalletRepo::new(pool.clone());
    let tx_request_repo = TxRequestRepo::new(pool.clone());
    let execution_attempt_repo: ExecutionAttemptRepo = ExecutionAttemptRepo::new(pool.clone());
    let execution_attempt_item_repo = ExecutionAttemptItemRepo::new(pool.clone());
    let wallet_assignment_repo = WalletAssignmentRepo::new(pool.clone());
    let seoa_address = get_seoa_address().unwrap();
    network_repo
        .add_anvil(seoa_address.to_string(), env_vars.anvil_chain_id)
        .await
        .unwrap();
    operator_wallet_repo
        .insert_from_mnemonic(&env_vars.anvil_mnemonic, env_vars.anvil_chain_id, 5)
        .await
        .unwrap();

    Ok(DbRepositories {
        network_repo,
        standard_tx_input_repo,
        operator_wallet_repo,
        blob_tx_input_repo,
        execution_attempt_item_repo,
        execution_attempt_repo,
        tx_request_repo,
        wallet_assignment_repo,
    })
}
