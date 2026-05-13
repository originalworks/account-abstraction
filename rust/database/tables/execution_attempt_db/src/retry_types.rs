use anyhow::anyhow;
use blob_tx_input_db::blob_tx_inputs::BlobTxInput;
use db_types::{BlobStorageType, TxStatus, TxType};
use sqlx::types::time::OffsetDateTime;
use standard_tx_input_db::standard_tx_inputs::StandardTxInput;
use tx_input_types::TxInput;
use tx_request_db::tx_requests::TxRequestWithInput;

use crate::execution_attempts::ExecutionAttempt;

#[derive(Debug, Clone)]
pub struct RetriedExecutionAttempt {
    pub execution_attempt: ExecutionAttempt,
    pub tx_requests: Vec<TxRequestWithInput>,
}

#[derive(sqlx::FromRow, Debug)]
pub struct ExecutionAttemptWithTxRequestRow {
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,

    pub blob_signature: Option<Vec<u8>>,
    pub image_id: Option<Vec<u8>>,
    pub commitment: Option<Vec<u8>>,
    pub blob_sha2: Option<Vec<u8>>,
    pub blob_deadline_timestamp: Option<i64>,
    pub storage_type: Option<BlobStorageType>,
    pub source_file_path: Option<String>,
    pub blob_created_at: Option<OffsetDateTime>,

    pub standard_signature: Option<Vec<u8>>,
    pub calldata: Option<Vec<u8>>,
    pub to_address: Option<String>,
    pub value_wei: Option<i64>,
    pub standard_deadline_timestamp: Option<i64>,
    pub pass_value_from_operator_wallet: Option<bool>,
    pub standard_created_at: Option<OffsetDateTime>,
}

impl ExecutionAttemptWithTxRequestRow {
    pub fn into_tx_input(&self) -> anyhow::Result<TxInput> {
        let tx_input: TxInput;
        if self.tx_type == TxType::BLOB {
            tx_input = TxInput::Blob(BlobTxInput {
                tx_id: self.tx_id.clone(),
                signature: self
                    .blob_signature
                    .clone()
                    .ok_or(anyhow!("missing blob_signature in execution attempt row"))?,
                image_id: self
                    .image_id
                    .clone()
                    .ok_or(anyhow!("missing image_id in execution attempt row"))?,
                commitment: self
                    .commitment
                    .clone()
                    .ok_or(anyhow!("missing commitment in execution attempt row"))?,
                blob_sha2: self
                    .blob_sha2
                    .clone()
                    .ok_or(anyhow!("missing blob_sha2 in execution attempt row"))?,
                deadline_timestamp: self.blob_deadline_timestamp.clone().ok_or(anyhow!(
                    "missing blob_deadline_timestamp in execution attempt row"
                ))?,
                storage_type: self
                    .storage_type
                    .clone()
                    .ok_or(anyhow!("missing storage_type in execution attempt row"))?,
                source_file_path: self
                    .source_file_path
                    .clone()
                    .ok_or(anyhow!("missing source_file_path in execution attempt row"))?,
                created_at: self
                    .blob_created_at
                    .clone()
                    .ok_or(anyhow!("missing blob_created_at in execution attempt row"))?,
            })
        } else {
            tx_input = TxInput::Standard(StandardTxInput {
                tx_id: self.tx_id.clone(),
                signature: self.standard_signature.clone().ok_or(anyhow!(
                    "missing standard_signature in execution attempt row"
                ))?,
                calldata: self
                    .calldata
                    .clone()
                    .ok_or(anyhow!("missing calldata in execution attempt row"))?,
                to_address: self
                    .to_address
                    .clone()
                    .ok_or(anyhow!("missing to_address in execution attempt row"))?,
                value_wei: self
                    .value_wei
                    .clone()
                    .ok_or(anyhow!("missing value_wei in execution attempt row"))?,
                deadline_timestamp: self.standard_deadline_timestamp.clone().ok_or(anyhow!(
                    "missing standard_deadline_timestamp in execution attempt row"
                ))?,
                pass_value_from_operator_wallet: self
                    .pass_value_from_operator_wallet
                    .clone()
                    .ok_or(anyhow!(
                        "missing pass_value_from_operator_wallet in execution attempt row"
                    ))?,
                created_at: self.standard_created_at.clone().ok_or(anyhow!(
                    "missing standard_created_at in execution attempt row"
                ))?,
            });
        }
        Ok(tx_input)
    }
}
