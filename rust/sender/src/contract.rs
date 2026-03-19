use std::{collections::HashMap, str::FromStr};

use alloy::{
    network,
    primitives::{Address, Bytes, FixedBytes, Uint, keccak256},
    providers::{
        ProviderBuilder,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
    sol,
};
use network_db::networks::Network;
use ow_wallet_adapter::wallet::OwWallet;
use transaction_db::transactions::Transaction;

use crate::transaction::ExecuteBatchTxContext;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    SEOA,
    "../../contracts/artifacts/contracts/sEOA.sol/sEOA.json"
);

type HardlyTypedProvider = FillProvider<
    JoinFill<
        alloy::providers::Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    alloy::providers::RootProvider,
>;

pub struct ContractManager {
    contract_address_by_chain_id: HashMap<i64, Address>,
    providers_by_chain_id: HashMap<i64, HardlyTypedProvider>,
}

impl ContractManager {
    pub fn build(networks: &Vec<Network>) -> anyhow::Result<Self> {
        let mut contract_address_by_chain_id = HashMap::new();
        let mut providers_by_chain_id = HashMap::new();
        for network in networks {
            contract_address_by_chain_id.insert(
                network.chain_id,
                Address::from_str(network.contract_address.as_str())?,
            );
            let provider = ProviderBuilder::new().connect_http(network.rpc_url.parse()?);
            providers_by_chain_id.insert(network.chain_id, provider);
        }

        Ok(Self {
            contract_address_by_chain_id,
            providers_by_chain_id,
        })
    }

    pub async fn send_batch(
        &self,
        tx_context: ExecuteBatchTxContext,
        wallet: OwWallet,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
