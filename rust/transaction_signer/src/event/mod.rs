#[cfg(feature = "aws")]
pub mod sqs;

use serde::{Deserialize, Serialize};
use transaction_db::transactions::{InsertTransactionInput, TxStatus, TxType};

#[derive(Deserialize, Serialize, Debug)]
pub struct SignTxRequest {
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub calldata: String,
    pub to_address: String,
    pub value_wei: i64,
    pub chain_id: i32,
    pub blob_file_path: Option<String>,
}

impl SignTxRequest {
    pub fn into_db_transaction(
        &self,
        signature: Vec<u8>,
    ) -> anyhow::Result<InsertTransactionInput> {
        Ok(InsertTransactionInput {
            tx_id: self.tx_id.clone(),
            requester_id: self.requester_id.clone(),
            tx_type: self.tx_type.clone(),
            tx_status: TxStatus::SIGNED,
            calldata: self.calldata_vec()?,
            to_address: self.to_address.clone(),
            value_wei: self.value_wei,
            chain_id: self.chain_id,
            blob_file_path: self.blob_file_path.clone(),
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
