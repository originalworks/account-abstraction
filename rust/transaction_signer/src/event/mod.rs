#[cfg(feature = "aws")]
pub mod sqs;

use serde::{Deserialize, Serialize};
use transaction_db::transactions::{InsertTransactionInput, TxStatus, TxType};

#[derive(Deserialize, Serialize)]
pub struct SignTxRequest {
    pub calldata: String,
    pub chain_id: i32,
    pub tx_id: String,
    pub sender_id: String,
    pub tx_type: TxType,
    pub blob_file_path: Option<String>,
}

impl SignTxRequest {
    pub fn into_db_transaction(&self, signature: String) -> anyhow::Result<InsertTransactionInput> {
        Ok(InsertTransactionInput {
            calldata: self.calldata.clone(),
            chain_id: self.chain_id.clone(),
            tx_id: self.tx_id.clone(),
            sender_id: self.sender_id.clone(),
            tx_type: self.tx_type.clone(),
            blob_file_path: self.blob_file_path.clone(),
            signature,
            tx_status: TxStatus::SIGNED,
        })
    }
}
