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
}
