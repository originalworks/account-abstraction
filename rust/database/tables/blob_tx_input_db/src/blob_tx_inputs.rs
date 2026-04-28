use db_types::BlobStorageType;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, types::time::OffsetDateTime};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BlobTxInput {
    pub tx_id: String,
    pub signature: Vec<u8>,
    pub image_id: Vec<u8>,
    pub commitment: Vec<u8>,
    pub blob_sha2: Vec<u8>,
    pub deadline_timestamp: i64,
    pub source_file_path: String,
    pub storage_type: BlobStorageType,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewBlobTxInput {
    pub tx_id: String,
    pub signature: Vec<u8>,
    pub image_id: Vec<u8>,
    pub commitment: Vec<u8>,
    pub blob_sha2: Vec<u8>,
    pub deadline_timestamp: i64,
    pub storage_type: BlobStorageType,
    pub source_file_path: String,
}

pub struct BlobTxInputRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> BlobTxInputRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_tx_id(&self, tx_id: &String) -> anyhow::Result<BlobTxInput> {
        let transaction = sqlx::query_as!(
            BlobTxInput,
            r#"
            SELECT 
                tx_id, 
                signature,
                image_id,
                commitment,
                blob_sha2,
                deadline_timestamp,
                source_file_path,
                storage_type as "storage_type: BlobStorageType",
                created_at,
                updated_at
            FROM 
                blob_tx_inputs
            WHERE
                tx_id = $1"#,
            tx_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(transaction)
    }
}
