use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Type, types::time::OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "text")]
pub enum KeyType {
    AwsKms,
    #[cfg(feature = "test-keys")]
    TestPrivateKey,
}

#[derive(Debug)]
pub struct NewOperatorWallet {
    pub id: Uuid,
    pub wallet_address: String,
    pub key_ref: String,
    pub key_type: KeyType,
    pub chain_id: i64,
    pub current_nonce: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct OperatorWallet {
    pub id: Uuid,
    pub wallet_address: String,
    pub key_ref: String,
    pub key_type: KeyType,
    pub chain_id: i64,
    pub nonce: i64,
    pub is_enabled: bool,
    pub in_use: bool,
    pub no_funds: bool,
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
                key_type as "key_type: KeyType",
                chain_id,
                nonce,
                is_enabled,
                in_use,
                no_funds,
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

    pub async fn lock_by_id(
        &self,
        operator_wallet_id: Uuid,
        chain_id: i64,
    ) -> anyhow::Result<Option<OperatorWallet>> {
        let wallet = sqlx::query_as!(
            OperatorWallet,
            r#"
        WITH candidate AS (
            SELECT id
            FROM operator_wallets
            WHERE
                id = $1
                AND chain_id = $2
                AND is_enabled = true
                AND in_use = false
                AND no_funds = false
            FOR UPDATE SKIP LOCKED
        )
        UPDATE operator_wallets ow
        SET
            in_use = true
        FROM candidate
        WHERE ow.id = candidate.id
        RETURNING
            ow.id,
            ow.wallet_address,
            ow.key_ref,
            ow.key_type as "key_type: KeyType",
            ow.chain_id,
            ow.nonce,
            ow.is_enabled,
            ow.in_use,
            ow.no_funds,
            ow.created_at,
            ow.updated_at
        "#,
            operator_wallet_id,
            chain_id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(wallet)
    }

    pub async fn lock_any_by_chain(&self, chain_id: i64) -> anyhow::Result<Option<OperatorWallet>> {
        let wallet = sqlx::query_as!(
            OperatorWallet,
            r#"
        WITH candidate AS (
            SELECT id
            FROM operator_wallets
            WHERE
                chain_id = $1
                AND is_enabled = true
                AND in_use = false
                AND no_funds = false
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
        UPDATE operator_wallets ow
        SET
            in_use = true
        FROM candidate
        WHERE ow.id = candidate.id
        RETURNING
            ow.id,
            ow.wallet_address,
            ow.key_ref,
            ow.key_type as "key_type: KeyType",
            ow.chain_id,
            ow.nonce,
            ow.is_enabled,
            ow.in_use,
            ow.no_funds,
            ow.created_at,
            ow.updated_at
        "#,
            chain_id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(wallet)
    }

    pub async fn mark_no_funds(&self, operator_wallet_id: Uuid) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
        UPDATE operator_wallets
        SET
            no_funds = true,
            in_use = false
        WHERE
            id = $1
        "#,
            operator_wallet_id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }
    pub async fn insert(&self, new_wallet: NewOperatorWallet) -> anyhow::Result<OperatorWallet> {
        let wallet = sqlx::query_as!(
            OperatorWallet,
            r#"
        INSERT INTO operator_wallets (
            id,
            wallet_address,
            key_ref,
            key_type,
            chain_id,
            nonce,
            is_enabled,
            in_use,
            no_funds
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            true,
            false,
            false
        )
        RETURNING
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
        "#,
            new_wallet.id,
            new_wallet.wallet_address,
            new_wallet.key_ref,
            new_wallet.key_type as KeyType,
            new_wallet.chain_id,
            new_wallet.current_nonce
        )
        .fetch_one(self.pool)
        .await?;

        Ok(wallet)
    }
}
