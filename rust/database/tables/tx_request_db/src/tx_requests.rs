use anyhow::bail;
use blob_tx_input_db::blob_tx_inputs::NewBlobTxInput;
use db_types::{BlobStorageType, TxStatus, TxType};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, types::time::OffsetDateTime};
use standard_tx_input_db::standard_tx_inputs::NewStandardTxInput;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct TxRequest {
    pub sequence_id: i64,
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub chain_id: i64,
    pub use_operator_wallet_id: Option<Uuid>,
    pub attempts: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewTxRequest {
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub chain_id: i64,
    pub use_operator_wallet_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NewTxInput {
    Blob(NewBlobTxInput),
    Standard(NewStandardTxInput),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewTxRequestWithTxInput {
    pub new_tx_request: NewTxRequest,
    pub tx_input: NewTxInput,
}

pub struct TxRequestRepo<'a> {
    pool: &'a PgPool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StandardTxRequestRaw {
    pub sequence_id: i64,
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub chain_id: i64,
    pub use_operator_wallet_id: Option<Uuid>,
    pub attempts: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,

    pub signature: Vec<u8>,
    pub calldata: Vec<u8>,
    pub to_address: String,
    pub value_wei: i64,
    pub deadline_timestamp: i64,
    pub pass_value_from_operator_wallet: bool,
}

impl<'a> TxRequestRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_tx_request_with_tx_input(
        &self,
        request: &NewTxRequestWithTxInput,
    ) -> anyhow::Result<()> {
        let mut postgres_tx = self.pool.begin().await?;

        sqlx::query!(
            r#"
            INSERT INTO tx_requests (
                tx_id,
                requester_id,
                tx_type,
                tx_status,
                chain_id,
                use_operator_wallet_id
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (tx_id) DO NOTHING
            "#,
            request.new_tx_request.tx_id,
            request.new_tx_request.requester_id,
            request.new_tx_request.tx_type.clone() as TxType,
            request.new_tx_request.tx_status.clone() as TxStatus,
            request.new_tx_request.chain_id,
            request.new_tx_request.use_operator_wallet_id
        )
        .execute(&mut *postgres_tx)
        .await?;

        match &request.tx_input {
            NewTxInput::Blob(new_blob_tx_input) => {
                if request.new_tx_request.tx_type != TxType::BLOB {
                    bail!("Non BLOB TxType: request: {:#?}", request)
                }
                sqlx::query!(
                    r#"
                    INSERT INTO blob_tx_inputs (
                        tx_id,
                        signature,
                        image_id,
                        commitment,
                        blob_sha2,
                        deadline_timestamp,
                        source_file_path,
                        storage_type
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    ON CONFLICT (tx_id) DO NOTHING
                    "#,
                    request.new_tx_request.tx_id,
                    new_blob_tx_input.signature,
                    new_blob_tx_input.image_id,
                    new_blob_tx_input.commitment,
                    new_blob_tx_input.blob_sha2,
                    new_blob_tx_input.deadline_timestamp,
                    new_blob_tx_input.source_file_path,
                    new_blob_tx_input.storage_type.clone() as BlobStorageType
                )
                .execute(&mut *postgres_tx)
                .await?;
            }
            NewTxInput::Standard(new_standard_tx_input) => {
                if request.new_tx_request.tx_type != TxType::STANDARD {
                    bail!("Non STANDARD TxType: request: {:#?}", request,)
                }
                sqlx::query!(
                    r#"
                    INSERT INTO standard_tx_inputs (
                        tx_id,
                        signature,
                        calldata,
                        to_address,
                        value_wei,
                        deadline_timestamp,
                        pass_value_from_operator_wallet
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    ON CONFLICT (tx_id) DO NOTHING
                    "#,
                    request.new_tx_request.tx_id,
                    new_standard_tx_input.signature,
                    new_standard_tx_input.calldata,
                    new_standard_tx_input.to_address,
                    new_standard_tx_input.value_wei,
                    new_standard_tx_input.deadline_timestamp,
                    new_standard_tx_input.pass_value_from_operator_wallet
                )
                .execute(&mut *postgres_tx)
                .await?;
            }
        }

        postgres_tx.commit().await?;

        Ok(())
    }

    pub async fn find_by_tx_id(&self, tx_id: &String) -> anyhow::Result<TxRequest> {
        let transaction = sqlx::query_as!(
            TxRequest,
            r#"
            SELECT 
                sequence_id,
                tx_id, 
                requester_id, 
                tx_status as "tx_status: TxStatus", 
                tx_type as "tx_type: TxType", 
                chain_id,
                attempts,
                use_operator_wallet_id,
                created_at,
                updated_at
            FROM 
                tx_requests
            WHERE
                tx_id = $1"#,
            tx_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(transaction)
    }

    pub async fn select_and_lock_many_standard(
        &self,
        ids: &Vec<String>,
    ) -> anyhow::Result<Vec<StandardTxRequestRaw>> {
        let rows = sqlx::query_as!(
            StandardTxRequestRaw,
            r#"
            WITH selected AS (
                SELECT tx_id
                FROM tx_requests
                WHERE tx_id = ANY($1)
                  AND tx_status = 'SIGNED'
                  AND tx_type = 'STANDARD'
                FOR UPDATE SKIP LOCKED
            ),
            updated AS (
                UPDATE tx_requests t
                SET
                    tx_status = 'LOCKED',
                    attempts = attempts + 1
                FROM selected
                WHERE t.tx_id = selected.tx_id
                RETURNING
                    t.sequence_id,
                    t.tx_id,
                    t.requester_id,
                    t.tx_type,
                    t.tx_status,
                    t.chain_id,
                    t.use_operator_wallet_id,
                    t.attempts,
                    t.created_at,
                    t.updated_at
            )
            SELECT
                u.sequence_id,
                u.tx_id,
                u.requester_id,
                u.tx_type as "tx_type: TxType",
                u.tx_status as "tx_status: TxStatus",
                u.chain_id,
                u.use_operator_wallet_id,
                u.attempts,
                u.created_at,
                u.updated_at,

                s.signature,
                s.calldata,
                s.to_address,
                s.value_wei,
                s.deadline_timestamp,
                s.pass_value_from_operator_wallet

            FROM updated u
            INNER JOIN standard_tx_inputs s ON s.tx_id = u.tx_id
            "#,
            ids
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn release_many(&self, ids: &Vec<String>) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
        UPDATE tx_requests
        SET 
            tx_status = 'SIGNED'
        WHERE tx_id = ANY($1)
        "#,
            ids
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_as_invalid(&self, tx_id: &String) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
        UPDATE tx_requests
        SET 
            tx_status = 'INVALID'
        WHERE tx_id = $1
        "#,
            tx_id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_many_as_broadcasted(&self, tx_ids: &Vec<String>) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
        UPDATE tx_requests
        SET 
            tx_status = 'BROADCASTED'
        WHERE tx_id = ANY($1)
        "#,
            tx_ids
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }
}
