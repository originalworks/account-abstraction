use serde::{Deserialize, Serialize};
use sqlx::{PgPool, types::time::OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct OperatorWallet {
    pub id: Uuid,
    pub wallet_address: String,
    pub key_ref: String,
    pub key_type: String,
    pub chain_id: i64,
    pub nonce: i64,
    pub is_enabled: bool,
    pub in_use: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct OperatorWalletRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> OperatorWalletRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, operator_wallet_id: Uuid) -> anyhow::Result<OperatorWallet> {
        let transaction = sqlx::query_as!(
            OperatorWallet,
            r#"
            SELECT
                id,
                wallet_address,
                key_ref,
                key_type,
                chain_id,
                nonce,
                is_enabled,
                in_use,
                created_at,
                updated_at
            FROM
                operator_wallets
            WHERE
                id = $1"#,
            operator_wallet_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(transaction)
    }
}
