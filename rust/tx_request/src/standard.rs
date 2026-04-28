use db_types::{TxStatus, TxType};
use serde::{Deserialize, Serialize};
use standard_tx_input_db::standard_tx_inputs::NewStandardTxInput;
// use standard_tx_input_db::NewStandardTxInput;
use tx_request_db::tx_requests::{NewTxInput, NewTxRequest, NewTxRequestWithTxInput};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StandardTxRequestBody {
    pub tx_id: String,
    pub requester_id: String,
    pub chain_id: i64,
    pub calldata: String,
    pub to_address: String,
    pub value_wei: i64,
    pub deadline_timestamp: i64,
    pub pass_value_from_operator_wallet: bool,
    pub use_operator_wallet_id: Option<Uuid>,
}

impl StandardTxRequestBody {
    pub fn into_db_input(&self, signature: Vec<u8>) -> anyhow::Result<NewTxRequestWithTxInput> {
        Ok(NewTxRequestWithTxInput {
            new_tx_request: NewTxRequest {
                tx_id: self.tx_id.clone(),
                requester_id: self.requester_id.clone(),
                tx_status: TxStatus::SIGNED,
                tx_type: TxType::STANDARD,
                chain_id: self.chain_id,
                use_operator_wallet_id: self.use_operator_wallet_id.clone(),
            },
            tx_input: NewTxInput::Standard(NewStandardTxInput {
                tx_id: self.tx_id.clone(),
                signature,
                calldata: self.calldata_vec()?,
                to_address: self.to_address.clone(),
                value_wei: self.value_wei,
                deadline_timestamp: self.deadline_timestamp,
                pass_value_from_operator_wallet: self.pass_value_from_operator_wallet,
            }),
        })
    }

    fn calldata_vec(&self) -> anyhow::Result<Vec<u8>> {
        let hex = self
            .calldata
            .strip_prefix("0x")
            .unwrap_or(self.calldata.as_str());
        Ok(hex::decode(hex)?)
    }
}
