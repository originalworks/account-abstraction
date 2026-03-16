use serde::{Deserialize, Serialize};
use sqlx::Type;
use sqlx::{PgPool, types::time::OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "text")]
pub enum TxType {
    STANDARD,
    BLOB,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "text")]
pub enum TxStatus {
    SIGNED,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    pub sequence_id: i64,
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub calldata: Vec<u8>,
    pub to_address: String,
    pub value_wei: i64,
    pub chain_id: i64,
    pub signature: Vec<u8>,
    pub retry_count: i32,
    pub tx_hash: Option<String>,
    pub blob_file_path: Option<String>,
    pub use_operator_wallet_id: Option<Uuid>,
    pub pass_value_from_operator_wallet: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InsertTransactionInput {
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub calldata: Vec<u8>,
    pub to_address: String,
    pub value_wei: i64,
    pub chain_id: i64,
    pub pass_value_from_operator_wallet: bool,
    pub signature: Vec<u8>,
    pub blob_file_path: Option<String>,
    pub use_operator_wallet_id: Option<Uuid>,
}

pub struct TransactionRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> TransactionRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_ignore_conflict(
        &self,
        input: &InsertTransactionInput,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            INSERT INTO transactions (
                tx_id,
                requester_id,
                tx_type,
                tx_status,
                calldata,
                to_address,
                value_wei,
                chain_id,
                pass_value_from_operator_wallet,
                signature,
                blob_file_path,
                use_operator_wallet_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (tx_id) DO NOTHING
            "#,
            input.tx_id,
            input.requester_id,
            input.tx_type.clone() as TxType,
            input.tx_status.clone() as TxStatus,
            input.calldata,
            input.to_address,
            input.value_wei,
            input.chain_id,
            input.pass_value_from_operator_wallet,
            input.signature,
            input.blob_file_path,
            input.use_operator_wallet_id
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn find_by_tx_id(&self, tx_id: String) -> anyhow::Result<Transaction> {
        let transaction = sqlx::query_as!(
            Transaction,
            r#"
            SELECT 
                sequence_id,
                tx_id, 
                requester_id, 
                tx_status as "tx_status: TxStatus", 
                tx_type as "tx_type: TxType", 
                blob_file_path,
                calldata,
                to_address,
                value_wei,
                chain_id,
                signature,
                tx_hash,
                retry_count,
                pass_value_from_operator_wallet,
                use_operator_wallet_id,
                created_at,
                updated_at
            FROM 
                transactions
            WHERE
                tx_id = $1"#,
            tx_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(transaction)
    }

    pub async fn select_and_lock_many(
        &self,
        ids: &Vec<String>,
    ) -> anyhow::Result<Vec<Transaction>> {
        let rows = sqlx::query_as!(
            Transaction,
            r#"
        WITH selected AS (
            SELECT tx_id
            FROM transactions
            WHERE tx_id = ANY($1)
              AND tx_status = 'SIGNED'
            FOR UPDATE SKIP LOCKED
        )
        UPDATE transactions t
        SET tx_status = 'LOCKED'
        FROM selected
        WHERE t.tx_id = selected.tx_id
        RETURNING
            t.sequence_id,
            t.tx_id, 
            t.requester_id, 
            t.tx_status as "tx_status: TxStatus", 
            t.tx_type as "tx_type: TxType", 
            t.blob_file_path,
            t.calldata,
            t.to_address,
            t.value_wei,
            t.chain_id,
            t.signature,
            t.tx_hash,
            t.retry_count,
            t.use_operator_wallet_id,
            t.pass_value_from_operator_wallet,
            t.created_at,
            t.updated_at
        "#,
            ids,
        )
        // .bind(ids)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }
}
