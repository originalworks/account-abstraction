use crate::Config;
use anyhow::bail;
use network_db::networks::Network;
use ow_wallet_adapter::{OwWalletConfig, wallet::OwWallet};
use std::collections::HashMap;

pub struct SignerWalletManager {
    pub wallets_by_chain_id: HashMap<i64, OwWallet>,
    networks_by_chain_id: HashMap<i64, Network>,
    use_kms: bool,
    private_key: Option<String>,
    signer_kms_id: Option<String>,
}

impl SignerWalletManager {
    pub fn build(networks: &Vec<Network>, config: &Config) -> anyhow::Result<Self> {
        let wallets_by_chain_id = HashMap::<i64, OwWallet>::new();
        let mut networks_by_chain_id = HashMap::<i64, Network>::new();
        for network in networks {
            networks_by_chain_id.insert(network.chain_id, network.clone());
        }
        Ok(Self {
            wallets_by_chain_id,
            networks_by_chain_id,
            use_kms: config.use_kms,
            private_key: config.private_key.clone(),
            signer_kms_id: config.signer_kms_id.clone(),
        })
    }

    pub async fn get_wallet(&mut self, chain_id: i64) -> anyhow::Result<&OwWallet> {
        if self.wallets_by_chain_id.contains_key(&chain_id) {
            return Ok(self
                .wallets_by_chain_id
                .get(&chain_id)
                .expect("Wallet not found"));
        } else {
            if self.networks_by_chain_id.contains_key(&chain_id) {
                let network = self
                    .networks_by_chain_id
                    .get(&chain_id)
                    .expect("Network not found");
                let wallet_config = OwWalletConfig {
                    rpc_url: network.rpc_url.clone(),
                    use_kms: self.use_kms,
                    private_key: self.private_key.clone(),
                    signer_kms_id: self.signer_kms_id.clone(),
                };
                let wallet = OwWallet::build(&wallet_config).await?;
                self.wallets_by_chain_id.insert(chain_id, wallet);
                return Ok(self
                    .wallets_by_chain_id
                    .get(&chain_id)
                    .expect("Wallet not found"));
            } else {
                bail!("Network for chain_id not found {chain_id}")
            }
        }
    }
}
