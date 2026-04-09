#[cfg(feature = "aws")]
pub mod sqs;

use db_types::TxType;
use serde::{Deserialize, Serialize};
use tx_request_db::tx_requests::{NewTxRequest, TxStatus};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TxRequestBody {
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub calldata: String,
    pub to_address: String,
    pub value_wei: i64,
    pub chain_id: i64,
    pub deadline_timestamp: i64,
    pub pass_value_from_operator_wallet: bool,
    pub blob_file_path: Option<String>,
    pub use_operator_wallet_id: Option<Uuid>,
}

impl TxRequestBody {
    pub fn into_db_input(&self, signature: Vec<u8>) -> anyhow::Result<NewTxRequest> {
        Ok(NewTxRequest {
            tx_id: self.tx_id.clone(),
            requester_id: self.requester_id.clone(),
            tx_type: self.tx_type.clone(),
            tx_status: TxStatus::SIGNED,
            calldata: self.calldata_vec()?,
            to_address: self.to_address.clone(),
            value_wei: self.value_wei,
            chain_id: self.chain_id,
            deadline_timestamp: self.deadline_timestamp,
            blob_file_path: self.blob_file_path.clone(),
            use_operator_wallet_id: self.use_operator_wallet_id.clone(),
            pass_value_from_operator_wallet: self.pass_value_from_operator_wallet.clone(),
            signature,
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
