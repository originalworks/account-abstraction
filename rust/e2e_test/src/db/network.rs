use network_db::networks::{NetworkRepo, NewNetwork};

#[allow(async_fn_in_trait)]
pub trait AddAnvilNetwork {
    async fn add_anvil(&self, contract_address: String) -> anyhow::Result<()>;
}

impl AddAnvilNetwork for NetworkRepo<'_> {
    async fn add_anvil(&self, contract_address: String) -> anyhow::Result<()> {
        self.insert_new_network(&NewNetwork {
            rpc_url: "http://anvil:8545".to_string(),
            chain_id: 31337,
            contract_address,
            chain_name: "anvil".to_string(),
            min_operator_wallet_balance: 1_000_000,
            gas_estimation_buffer_ppm: 120_000,
        })
        .await?;
        Ok(())
    }
}
