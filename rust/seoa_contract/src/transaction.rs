use crate::contract::sEOA::ExecuteInput;
use alloy::{
    eips::eip1559::Eip1559Estimation,
    primitives::{Address, Uint, keccak256},
};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tx_input_types::TxInput;
use tx_request_db::types::{StandardTxRequestRaw, TxRequestWithInput};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecuteBatchTxContext {
    pub chain_id: i64,
    pub execute_batch_input: Vec<ExecuteInput>,
    pub use_operator_wallet_id: Option<Uuid>,
    pub batch_tx_value: i64,
    pub tx_requests: Vec<TxRequestWithInput>,
    pub successfully_simulated: bool,
    pub assigned_nonce: Option<u64>,
    pub fees: Option<Eip1559Estimation>,
    pub gas_limit: Option<u64>,
    pub tx_hash: Option<String>,
}

impl ExecuteBatchTxContext {
    pub fn get_tx_ids(&self) -> Vec<String> {
        self.tx_requests
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

impl IntoExecuteInput for TxRequestWithInput {
    fn into_execute_input(&self) -> anyhow::Result<ExecuteInput> {
        match self.tx_input.clone() {
            TxInput::Blob(_) => {
                bail!("Can't parse blob tx input into execute input");
            }
            TxInput::Standard(standard_tx_input) => Ok(ExecuteInput {
                target: Address::from_str(standard_tx_input.to_address.as_str())?,
                payload: standard_tx_input.calldata.clone().into(),
                value: Uint::<256, 4>::from(standard_tx_input.value_wei as u64),
                salt: keccak256(standard_tx_input.tx_id.clone().into_bytes()),
                deadline: Uint::<256, 4>::from(standard_tx_input.deadline_timestamp as u64),
                signature: standard_tx_input.signature.clone().into(),
            }),
        }
    }
}
