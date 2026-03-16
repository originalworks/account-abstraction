use std::collections::HashMap;

use network_db::networks::Network;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use ow_wallet_adapter::wallet::OwWallet;
use transaction_assignment_db::transaction_assignments::TransactionAssignmentRepo;

pub struct WalletPoolManager<'a> {
    operator_wallet_repo: OperatorWalletRepo<'a>,
    transaction_assignment_repo: TransactionAssignmentRepo<'a>,
    networks_map: HashMap<i64, Network>,
}

impl<'a> WalletPoolManager<'a> {
    pub fn build(
        operator_wallet_repo: OperatorWalletRepo<'a>,
        transaction_assignment_repo: TransactionAssignmentRepo<'a>,
        networks: &Vec<Network>,
    ) -> Self {
        let mut networks_map: HashMap<i64, Network> = HashMap::new();
        for network in networks {
            networks_map.insert(network.chain_id, network.clone());
        }
        Self {
            operator_wallet_repo,
            networks_map,
            transaction_assignment_repo,
        }
    }

    pub async fn acquire(&self) -> anyhow::Result<()> {
        Ok(())
    }

    pub async fn release(&self, ow_wallet: OwWallet) -> anyhow::Result<()> {
        Ok(())
    }
}
