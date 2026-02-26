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
    pub tx_id: String,
    pub sender_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub calldata: String,
    pub chain_id: i32,
    pub signature: String,
    pub blob_file_path: Option<String>,
    pub tx_hash: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InsertTransactionInput {
    pub tx_id: String,
    pub sender_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub calldata: String,
    pub chain_id: i32,
    pub signature: String,
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
                sender_id,
                tx_status,
                tx_type,
                blob_file_path,
                calldata,
                chain_id,
                signature
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (tx_id) DO NOTHING
            "#,
            input.tx_id,
            input.sender_id,
            input.tx_status.clone() as TxStatus,
            input.tx_type.clone() as TxType,
            input.blob_file_path,
            input.calldata,
            input.chain_id,
            input.signature
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
                tx_id, 
                sender_id, 
                tx_status as "tx_status: TxStatus", 
                tx_type as "tx_type: TxType", 
                blob_file_path, 
                calldata, 
                chain_id,
                signature,
                tx_hash,
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
