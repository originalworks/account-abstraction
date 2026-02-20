use serde::{Deserialize, Serialize};
use sqlx::{PgPool, types::time::OffsetDateTime};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    pub tx_id: String,
    pub sender_id: String,
    pub tx_type: String,
    pub tx_status: String,
    pub calldata: String,
    pub chain_id: i32,
    pub signature: Option<String>,
    pub blob_file_path: Option<String>,
    pub tx_hash: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct TransactionRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> TransactionRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_tx_id(&self, tx_id: String) -> anyhow::Result<Transaction> {
        let transaction = sqlx::query_as!(
            Transaction,
            "
            SELECT 
                tx_id, 
                sender_id, 
                tx_status, 
                tx_type, 
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
                tx_id = $1",
            tx_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(transaction)
    }
}
