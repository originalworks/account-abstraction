use alloy::providers::Provider;
use alloy::{eips::BlockId, primitives::U256};
use anyhow::{Ok, bail};
use network_db::networks::Network;
use operator_wallet_db::operator_wallets::{KeyType, OperatorWallet};
use ow_wallet_adapter::{OwWalletConfig, wallet::OwWallet};

pub struct Wallet {
    pub db_record: OperatorWallet,
    pub ow_wallet: OwWallet,
    pub chain_id: i64,
    pub min_balance: i64,
    pub next_nonce: Option<u64>,
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
            next_nonce: None,
        })
    }

    pub async fn set_next_nonce(&mut self) -> anyhow::Result<()> {
        let pending_nonce = self.get_pending_nonce().await?;
        let latest_nonce = self.get_latest_nonce().await?;
        let db_nonce = u64::try_from(self.db_record.nonce)?;
        if pending_nonce != latest_nonce {
            bail!(
                "Can't set next nonce for wallet with pending tx: {}",
                self.db_record.id
            )
        }
        if latest_nonce == db_nonce {
            self.next_nonce = Some(db_nonce);
        } else {
            bail!(
                "Nonce mismatch! latest nonce: {}, db nonce: {}",
                latest_nonce,
                db_nonce
            )
        }

        Ok(())
    }

    pub fn use_nonce(&mut self) -> anyhow::Result<u64> {
        let Some(next_nonce) = self.next_nonce else {
            bail!("Wallet next nonce was not set")
        };
        self.next_nonce = None;
        Ok(next_nonce)
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

    pub async fn get_pending_nonce(&self) -> anyhow::Result<u64> {
        let address = self.ow_wallet.get_address()?;

        let nonce = self
            .ow_wallet
            .provider
            .get_transaction_count(address)
            .block_id(BlockId::pending())
            .await?;

        Ok(nonce)
    }

    pub async fn get_latest_nonce(&self) -> anyhow::Result<u64> {
        let address = self.ow_wallet.get_address()?;

        let nonce = self
            .ow_wallet
            .provider
            .get_transaction_count(address)
            .block_id(BlockId::latest())
            .await?;

        Ok(nonce)
    }
}
