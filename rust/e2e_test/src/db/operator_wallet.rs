use alloy::signers::local::{MnemonicBuilder, coins_bip39::English};
use operator_wallet_db::operator_wallets::{
    KeyType, NewOperatorWallet, OperatorWallet, OperatorWalletRepo,
};

#[allow(async_fn_in_trait)]
pub trait InsertFromMnemonic {
    async fn insert_from_mnemonic(&self, mnemonic: String, chain_id: i64) -> anyhow::Result<()>;
    async fn select_all(&self) -> anyhow::Result<Vec<OperatorWallet>>;
}

impl InsertFromMnemonic for OperatorWalletRepo<'_> {
    async fn insert_from_mnemonic(&self, mnemonic: String, chain_id: i64) -> anyhow::Result<()> {
        let existing_entries = self.select_all().await?;
        if existing_entries.is_empty() {
            for i in 100..100 + 5 {
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
        }
        Ok(())
    }

    async fn select_all(&self) -> anyhow::Result<Vec<OperatorWallet>> {
        let wallets = sqlx::query_as!(
            OperatorWallet,
            r#"
            SELECT
                id,
                wallet_address,
                key_ref,
                key_type as "key_type: KeyType",
                chain_id,
                nonce,
                is_enabled,
                in_use,
                no_funds,
                created_at,
                updated_at
            FROM
                operator_wallets"#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(wallets)
    }
}
