use alloy::signers::local::{MnemonicBuilder, coins_bip39::English};
use operator_wallet_db::operator_wallets::{KeyType, NewOperatorWallet, OperatorWalletRepo};

#[allow(async_fn_in_trait)]
pub trait InsertFromMnemonic {
    async fn insert_from_mnemonic(
        &self,
        mnemonic: String,
        chain_id: i64,
        limit: u32,
    ) -> anyhow::Result<()>;
}

impl InsertFromMnemonic for OperatorWalletRepo<'_> {
    async fn insert_from_mnemonic(
        &self,
        mnemonic: String,
        chain_id: i64,
        limit: u32,
    ) -> anyhow::Result<()> {
        for i in 100..100 + limit {
            let wallet = MnemonicBuilder::<English>::default()
                .phrase(&mnemonic)
                .index(i)?
                .build()?;
            let bytes = wallet.to_bytes();
            let private_key = hex::encode(bytes);
            let new_operator_wallet = NewOperatorWallet {
                id: uuid::Uuid::new_v4(),
                key_ref: private_key,
                key_type: KeyType::TestPrivateKey,
                wallet_address: wallet.address().to_string(),
                chain_id,
                current_nonce: 0,
            };
            self.insert(new_operator_wallet).await?;
        }
        Ok(())
    }
}
