use network_db::networks::{NetworkRepo, NewNetwork};

#[allow(async_fn_in_trait)]
pub trait AddAnvilNetwork {
    async fn add_anvil(&self, contract_address: String, chain_id: i64) -> anyhow::Result<()>;
    async fn set_tx_max_age(&self, tx_max_age_sec: i64, chain_id: i64) -> anyhow::Result<()>;
}

impl AddAnvilNetwork for NetworkRepo {
    async fn add_anvil(&self, contract_address: String, chain_id: i64) -> anyhow::Result<()> {
        self.insert_new_network(&NewNetwork {
            rpc_url: "http://anvil:8545".to_string(),
            chain_id,
            contract_address,
            chain_name: "anvil".to_string(),
            min_operator_wallet_balance: 1_000_000,
            gas_estimation_buffer_ppm: 1_200_000,
            blob_gas_estimation_buffer_ppm: 200_000_000,
            tx_max_age_sec: 3600,
        })
        .await?;
        Ok(())
    }

    async fn set_tx_max_age(&self, tx_max_age_sec: i64, chain_id: i64) -> anyhow::Result<()> {
        let result = sqlx::query!(
            r#"
        UPDATE networks
        SET
            tx_max_age_sec = $1
        WHERE
            chain_id = $2
        "#,
            tx_max_age_sec,
            chain_id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("network not found");
        }
        Ok(())
    }
}
