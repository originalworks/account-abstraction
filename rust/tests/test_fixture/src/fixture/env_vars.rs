use alloy::{primitives::Address, signers::local::PrivateKeySigner};

#[derive(Clone)]
pub struct E2eTestEnvVars {
    pub anvil_chain_id: i64,
    pub anvil_mnemonic: String,
    pub blob_storage_bucket_name: String,
    pub seoa_private_key: String,
    pub outcome_event_bus_name: String,
}

impl E2eTestEnvVars {
    pub fn build() -> anyhow::Result<Self> {
        let anvil_chain_id = std::env::var("ANVIL_CHAIN_ID").unwrap().parse().unwrap();
        let anvil_mnemonic = std::env::var("ANVIL_MNEMONIC").unwrap();
        let blob_storage_bucket_name = std::env::var("BLOB_STORAGE_BUCKET_NAME").unwrap();
        let outcome_event_bus_name = std::env::var("OUTCOME_EVENT_BUS_NAME").unwrap();
        let seoa_private_key = std::env::var("PRIVATE_KEY").unwrap();

        Ok(E2eTestEnvVars {
            anvil_chain_id,
            anvil_mnemonic,
            blob_storage_bucket_name,
            outcome_event_bus_name,
            seoa_private_key,
        })
    }
    pub fn get_seoa_address(&self) -> anyhow::Result<Address> {
        let pk_signer: PrivateKeySigner = self.seoa_private_key.parse().unwrap();

        Ok(pk_signer.address())
    }
}
