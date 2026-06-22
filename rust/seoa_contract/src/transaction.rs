use crate::contract::sEOA::ExecuteInput;
use alloy::{
    eips::eip1559::Eip1559Estimation,
    primitives::{Address, Uint, keccak256},
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tx_request_db::tx_requests::StandardTxRequestRaw;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecuteBatchTxContext {
    pub chain_id: i64,
    pub execute_batch_input: Vec<ExecuteInput>,
    pub use_operator_wallet_id: Option<Uuid>,
    pub batch_tx_value: i64,
    pub raw_tx_requests: Vec<StandardTxRequestRaw>,
    pub successfully_simulated: bool,
    pub assigned_nonce: Option<u64>,
    pub fees: Option<Eip1559Estimation>,
    pub gas_limit: Option<u64>,
    pub tx_hash: Option<String>,
}

impl ExecuteBatchTxContext {
    pub fn get_tx_ids(&self) -> Vec<String> {
        self.raw_tx_requests
            .iter()
            .map(|val| val.tx_id.clone())
            .collect()
    }
}

pub trait IntoExecuteInput {
    fn into_execute_input(&self) -> anyhow::Result<ExecuteInput>;
}

impl IntoExecuteInput for StandardTxRequestRaw {
    fn into_execute_input(&self) -> anyhow::Result<ExecuteInput> {
        Ok(ExecuteInput {
            target: Address::from_str(self.to_address.as_str())?,
            payload: self.calldata.clone().into(),
            value: Uint::<256, 4>::from(self.value_wei as u64),
            salt: keccak256(self.tx_id.clone().into_bytes()),
            deadline: Uint::<256, 4>::from(self.deadline_timestamp as u64),
            signature: self.signature.clone().into(),
        })
    }
}
