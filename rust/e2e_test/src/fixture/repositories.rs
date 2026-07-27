use crate::{
    db::{network::AnvilTestNetwork, operator_wallet::InsertFromMnemonic},
    fixture::env_vars::E2eTestEnvVars,
};
use blob_tx_input_db::blob_tx_inputs::BlobTxInputRepo;
use execution_attempt_db::execution_attempts::ExecutionAttemptRepo;
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use network_db::networks::NetworkRepo;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use sqlx::PgPool;
use standard_tx_input_db::standard_tx_inputs::StandardTxInputRepo;
use tx_request_db::repo::TxRequestRepo;
use wallet_assignment_db::wallet_assignments::WalletAssignmentRepo;

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

impl DbRepositories {
    pub async fn build(pool: &PgPool, env_vars: &E2eTestEnvVars) -> anyhow::Result<Self> {
        let network_repo = NetworkRepo::new(pool.clone());
        let standard_tx_input_repo = StandardTxInputRepo::new(pool.clone());
        let blob_tx_input_repo = BlobTxInputRepo::new(pool.clone());
        let operator_wallet_repo = OperatorWalletRepo::new(pool.clone());
        let tx_request_repo = TxRequestRepo::new(pool.clone());
        let execution_attempt_repo: ExecutionAttemptRepo = ExecutionAttemptRepo::new(pool.clone());
        let execution_attempt_item_repo = ExecutionAttemptItemRepo::new(pool.clone());
        let wallet_assignment_repo = WalletAssignmentRepo::new(pool.clone());
        // let seoa_address = env_vars.get_seoa_address()?;
        // network_repo
        //     .add_anvil(seoa_address.to_string(), env_vars.anvil_chain_id)
        //     .await
        //     .unwrap();
        // operator_wallet_repo
        //     .insert_from_mnemonic(&env_vars.anvil_mnemonic, env_vars.anvil_chain_id, 5)
        //     .await
        //     .unwrap();

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
}
