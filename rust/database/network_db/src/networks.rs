use serde::{Deserialize, Serialize};
use sqlx::{PgPool, types::time::OffsetDateTime};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Network {
    pub chain_id: i64,
    pub chain_name: String,
    pub rpc_url: String,
    pub contract_address: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct InsertNetworkInput {
    pub chain_id: i64,
    pub chain_name: String,
    pub rpc_url: String,
    pub contract_address: String,
}

pub struct NetworkRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> NetworkRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_chain_id(&self, chain_id: i64) -> anyhow::Result<Network> {
        let network = sqlx::query_as!(
            Network,
            r#"
            SELECT
                chain_id,
                chain_name,
                rpc_url,
                contract_address,
                created_at,
                updated_at
            FROM
                networks
            WHERE
                chain_id = $1"#,
            chain_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(network)
    }

    pub async fn select_all(&self) -> anyhow::Result<Vec<Network>> {
        let networks = sqlx::query_as!(
            Network,
            r#"
            SELECT
                chain_id,
                chain_name,
                rpc_url,
                contract_address,
                created_at,
                updated_at
            FROM
                networks"#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(networks)
    }

    pub async fn insert_new_network(
        &self,
        network: &InsertNetworkInput,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            INSERT INTO networks (
                chain_id,
                chain_name,
                rpc_url,
                contract_address
            )
            VALUES ($1, $2, $3, $4)"#,
            network.chain_id,
            network.chain_name,
            network.rpc_url,
            network.contract_address,
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }
}
