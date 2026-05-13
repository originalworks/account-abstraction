use crate::{
    aws::{
        config::build_aws_sdk_config, s3::S3BlobStorageManagerTestFeatures, sqs::TestQueueManager,
    },
    db::{get_pool, network::AddAnvilNetwork, operator_wallet::InsertFromMnemonic},
};
use alloy::{primitives::Address, signers::local::PrivateKeySigner};
use blob_storage::storage::s3::S3BlobStorageManager;
use blob_tx_input_db::blob_tx_inputs::BlobTxInputRepo;
use execution_attempt_db::execution_attempts::ExecutionAttemptRepo;
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use network_db::networks::NetworkRepo;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use sqlx::PgPool;
use standard_tx_input_db::standard_tx_inputs::StandardTxInputRepo;
use tokio::sync::OnceCell;
use tx_request_db::tx_requests::TxRequestRepo;
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

pub struct E2eTestFixture {
    pub test_queue_manager: TestQueueManager,
    pub db_repositories: DbRepositories,
    pub pool: PgPool,
    pub env_vars: E2eTestEnvVars,
    pub blob_storage_manager: S3BlobStorageManager,
    pub aws_config: aws_config::SdkConfig,
}

pub struct E2eTestEnvVars {
    pub anvil_chain_id: i64,
    pub anvil_mnemonic: String,
    pub blob_storage_bucket_name: String,
    pub tx_max_age_sec: String,
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

            let blob_storage_manager = S3BlobStorageManager::build(
                &aws_config,
                &e2e_test_env_vars.blob_storage_bucket_name,
            );

            blob_storage_manager.prepare_for_test().await.unwrap();

            E2eTestFixture {
                pool,
                test_queue_manager,
                db_repositories,
                env_vars: e2e_test_env_vars,
                blob_storage_manager,
                aws_config,
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
    let tx_max_age_sec = std::env::var("TX_MAX_AGE_SEC").unwrap();

    Ok(E2eTestEnvVars {
        anvil_chain_id,
        anvil_mnemonic,
        blob_storage_bucket_name,
        tx_max_age_sec,
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
