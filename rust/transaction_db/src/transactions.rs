use serde::{Deserialize, Serialize};
use sqlx::Type;
use sqlx::{PgPool, types::time::OffsetDateTime};

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
    pub chain_id: i32,
    pub signature: Vec<u8>,
    pub retry_count: i32,
    pub tx_hash: Option<String>,
    pub blob_file_path: Option<String>,
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
    pub chain_id: i32,
    pub signature: Vec<u8>,
    pub blob_file_path: Option<String>,
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
                signature,
                blob_file_path
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
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
            input.signature,
            input.blob_file_path,
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
}
