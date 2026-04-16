use crate::wallet::Wallet;
use anyhow::bail;
use network_db::networks::Network;
use operator_wallet_db::operator_wallets::{OperatorWallet, OperatorWalletRepo};
use ow_wallet_adapter::wallet::OwWallet;
use std::collections::HashMap;
use uuid::Uuid;

enum AcquireAttemptResult {
    Acquired(Wallet),
    NoWalletAvailable,
    InsufficientFunds(Uuid),
}

pub struct WalletPoolManager<'a> {
    operator_wallet_repo: OperatorWalletRepo<'a>,
    networks_map: HashMap<i64, Network>,
}

impl<'a> WalletPoolManager<'a> {
    pub fn build(operator_wallet_repo: OperatorWalletRepo<'a>, networks: &Vec<Network>) -> Self {
        let mut networks_map: HashMap<i64, Network> = HashMap::new();
        for network in networks {
            networks_map.insert(network.chain_id, network.clone());
        }
        Self {
            operator_wallet_repo,
            networks_map,
        }
    }

    async fn fetch_and_lock(
        &self,
        chain_id: i64,
        use_operator_wallet_id: Option<Uuid>,
    ) -> anyhow::Result<Option<OperatorWallet>> {
        if let Some(operator_wallet_id) = use_operator_wallet_id {
            return Ok(self
                .operator_wallet_repo
                .lock_by_id(operator_wallet_id, chain_id)
                .await?);
        } else {
            return Ok(self
                .operator_wallet_repo
                .lock_any_by_chain(chain_id)
                .await?);
        }
    }

    async fn try_acquire_once(
        &self,
        chain_id: i64,
        use_operator_wallet_id: Option<Uuid>,
    ) -> anyhow::Result<AcquireAttemptResult> {
        let Some(network) = self.networks_map.get(&chain_id) else {
            bail!("Network not found for chain_id: {chain_id}");
        };

        let Some(operator_wallet) = self
            .fetch_and_lock(chain_id, use_operator_wallet_id)
            .await?
        else {
            return Ok(AcquireAttemptResult::NoWalletAvailable);
        };

        let mut wallet = Wallet::build(&operator_wallet, &network).await?;

        if wallet.has_enough_balance().await? == false {
            return Ok(AcquireAttemptResult::InsufficientFunds(operator_wallet.id));
        }

        wallet.set_next_nonce().await?;

        Ok(AcquireAttemptResult::Acquired(wallet))
    }

    pub async fn acquire(
        &self,
        chain_id: i64,
        use_operator_wallet_id: Option<Uuid>,
    ) -> anyhow::Result<Option<Wallet>> {
        loop {
            match self
                .try_acquire_once(chain_id, use_operator_wallet_id)
                .await?
            {
                AcquireAttemptResult::Acquired(wallet) => return Ok(Some(wallet)),

                AcquireAttemptResult::NoWalletAvailable => return Ok(None),

                AcquireAttemptResult::InsufficientFunds(operator_wallet_id) => {
                    self.operator_wallet_repo
                        .mark_no_funds(operator_wallet_id)
                        .await?;
                    if use_operator_wallet_id.is_some() {
                        return Ok(None);
                    } else {
                        continue;
                    }
                }
            }
        }
    }

    pub async fn release(&self, operator_wallet_id: Uuid) -> anyhow::Result<()> {
        self.operator_wallet_repo
            .release(operator_wallet_id)
            .await?;
        Ok(())
    }
}
