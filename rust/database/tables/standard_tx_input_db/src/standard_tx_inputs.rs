use serde::{Deserialize, Serialize};
use sqlx::{PgPool, types::time::OffsetDateTime};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct StandardTxInput {
    pub tx_id: String,
    pub signature: Vec<u8>,
    pub calldata: Vec<u8>,
    pub to_address: String,
    pub value_wei: i64,
    pub deadline_timestamp: i64,
    pub pass_value_from_operator_wallet: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewStandardTxInput {
    pub tx_id: String,
    pub signature: Vec<u8>,
    pub calldata: Vec<u8>,
    pub to_address: String,
    pub value_wei: i64,
    pub deadline_timestamp: i64,
    pub pass_value_from_operator_wallet: bool,
}

pub struct StandardTxInputRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> StandardTxInputRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    // pub async fn insert_ignore_conflict(&self, input: &NewTxRequest) -> Result<bool, sqlx::Error> {
    //     let result = sqlx::query!(
    //         r#"
    //         INSERT INTO tx_requests (
    //             tx_id,
    //             requester_id,
    //             tx_type,
    //             tx_status,
    //             calldata,
    //             to_address,
    //             value_wei,
    //             chain_id,
    //             deadline_timestamp,
    //             pass_value_from_operator_wallet,
    //             signature,
    //             source_file_path,
    //             use_operator_wallet_id
    //         )
    //         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
    //         ON CONFLICT (tx_id) DO NOTHING
    //         "#,
    //         input.tx_id,
    //         input.requester_id,
    //         input.tx_type.clone() as TxType,
    //         input.tx_status.clone() as TxStatus,
    //         input.calldata,
    //         input.to_address,
    //         input.value_wei,
    //         input.chain_id,
    //         input.deadline_timestamp,
    //         input.pass_value_from_operator_wallet,
    //         input.signature,
    //         input.source_file_path,
    //         input.use_operator_wallet_id
    //     )
    //     .execute(self.pool)
    //     .await?;

    //     Ok(result.rows_affected() == 1)
    // }

    pub async fn find_by_tx_id(&self, tx_id: &String) -> anyhow::Result<StandardTxInput> {
        let transaction = sqlx::query_as!(
            StandardTxInput,
            r#"
            SELECT 
                tx_id, 
                calldata,
                to_address,
                value_wei,
                deadline_timestamp,
                signature,
                pass_value_from_operator_wallet,
                created_at,
                updated_at
            FROM 
                standard_tx_inputs
            WHERE
                tx_id = $1"#,
            tx_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(transaction)
    }

    // pub async fn select_and_lock_many(&self, ids: &Vec<String>) -> anyhow::Result<Vec<TxRequest>> {
    //     let rows = sqlx::query_as!(
    //         TxRequest,
    //         r#"
    //     WITH selected AS (
    //         SELECT tx_id
    //         FROM tx_requests
    //         WHERE tx_id = ANY($1)
    //           AND tx_status = 'SIGNED'
    //         FOR UPDATE SKIP LOCKED
    //     )
    //     UPDATE tx_requests t
    //     SET
    //         tx_status = 'LOCKED',
    //         attempts = attempts + 1
    //     FROM selected
    //     WHERE t.tx_id = selected.tx_id
    //     RETURNING
    //         t.sequence_id,
    //         t.tx_id,
    //         t.requester_id,
    //         t.tx_status as "tx_status: TxStatus",
    //         t.tx_type as "tx_type: TxType",
    //         t.source_file_path,
    //         t.calldata,
    //         t.to_address,
    //         t.value_wei,
    //         t.chain_id,
    //         t.deadline_timestamp,
    //         t.signature,
    //         t.attempts,
    //         t.use_operator_wallet_id,
    //         t.pass_value_from_operator_wallet,
    //         t.created_at,
    //         t.updated_at
    //     "#,
    //         ids,
    //     )
    //     // .bind(ids)
    //     .fetch_all(self.pool)
    //     .await?;

    //     Ok(rows)
    // }

    // pub async fn release_many(&self, ids: &Vec<String>) -> anyhow::Result<()> {
    //     sqlx::query!(
    //         r#"
    //     UPDATE tx_requests
    //     SET
    //         tx_status = 'SIGNED'
    //     WHERE tx_id = ANY($1)
    //     "#,
    //         ids
    //     )
    //     .execute(self.pool)
    //     .await?;

    //     Ok(())
    // }

    // pub async fn mark_as_invalid(&self, tx_id: &String) -> anyhow::Result<()> {
    //     sqlx::query!(
    //         r#"
    //     UPDATE tx_requests
    //     SET
    //         tx_status = 'INVALID'
    //     WHERE tx_id = $1
    //     "#,
    //         tx_id
    //     )
    //     .execute(self.pool)
    //     .await?;

    //     Ok(())
    // }

    // pub async fn mark_many_as_broadcasted(&self, tx_ids: &Vec<String>) -> anyhow::Result<()> {
    //     sqlx::query!(
    //         r#"
    //     UPDATE tx_requests
    //     SET
    //         tx_status = 'BROADCASTED'
    //     WHERE tx_id = ANY($1)
    //     "#,
    //         tx_ids
    //     )
    //     .execute(self.pool)
    //     .await?;

    //     Ok(())
    // }
}
