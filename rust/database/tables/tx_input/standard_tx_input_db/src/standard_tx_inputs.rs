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

pub struct StandardTxInputRepo {
    pool: PgPool,
}

impl StandardTxInputRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

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
                created_at
            FROM 
                standard_tx_inputs
            WHERE
                tx_id = $1"#,
            tx_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(transaction)
    }
}
