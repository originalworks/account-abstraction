use alloy::providers::Provider;
use alloy::{eips::BlockId, primitives::U256};
use anyhow::bail;
use network_db::networks::Network;
use operator_wallet_db::operator_wallets::{KeyType, OperatorWallet, OperatorWalletRepo};
use ow_wallet_adapter::{OwWalletConfig, wallet::OwWallet};
use std::collections::HashMap;
use uuid::Uuid;

pub struct Wallet {
    pub db_record: OperatorWallet,
    pub ow_wallet: OwWallet,
    pub chain_id: i64,
    pub min_balance: i64,
}
impl Wallet {
    pub async fn build(
        operator_wallet: &OperatorWallet,
        network: &Network,
    ) -> anyhow::Result<Self> {
        let ow_wallet_config;

        match operator_wallet.key_type {
            KeyType::AwsKms => {
                ow_wallet_config = OwWalletConfig {
                    use_kms: true,
                    rpc_url: network.rpc_url.clone(),
                    signer_kms_id: Some(operator_wallet.key_ref.clone()),
                    private_key: None,
                };
            }
            #[cfg(feature = "test-keys")]
            KeyType::TestPrivateKey => {
                ow_wallet_config = OwWalletConfig {
                    use_kms: false,
                    rpc_url: network.rpc_url.clone(),
                    signer_kms_id: None,
                    private_key: Some(operator_wallet.key_ref.clone()),
                }
            }
        }

        let ow_wallet = OwWallet::build(&ow_wallet_config).await?;
        Ok(Self {
            ow_wallet,
            db_record: operator_wallet.clone(),
            chain_id: network.chain_id,
            min_balance: network.min_operator_wallet_balance,
        })
    }

    pub async fn has_enough_balance(&self) -> anyhow::Result<bool> {
        let wallet_address = self.ow_wallet.get_address()?;
        let balance = self
            .ow_wallet
            .provider
            .get_balance(wallet_address)
            .block_id(BlockId::latest())
            .await?;

        if U256::from(self.min_balance) > balance {
            return Ok(false);
        } else {
            return Ok(true);
        }
    }
}

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

    async fn fetch_from_db(
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

        let Some(operator_wallet) = self.fetch_from_db(chain_id, use_operator_wallet_id).await?
        else {
            return Ok(AcquireAttemptResult::NoWalletAvailable);
        };

        let wallet = Wallet::build(&operator_wallet, &network).await?;

        if wallet.has_enough_balance().await? == false {
            return Ok(AcquireAttemptResult::InsufficientFunds(operator_wallet.id));
        }

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

    pub async fn release(&self, ow_wallet: OwWallet) -> anyhow::Result<()> {
        Ok(())
    }
}
