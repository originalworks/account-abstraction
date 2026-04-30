use alloy::{consensus::BlobTransactionSidecarEip7594, primitives::FixedBytes};
use blob_tx_input_db::blob_tx_inputs::NewBlobTxInput;
use db_types::{BlobStorageType, TxStatus, TxType};
use serde::{Deserialize, Serialize};
// use standard_tx_input_db::standard_tx_inputs::NewStandardTxInput;
// use blob_tx_input_db::NewBlobTxInput;
use tx_request_db::tx_requests::{NewTxInput, NewTxRequest, NewTxRequestWithTxInput};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BlobTxRequestBody {
    pub tx_id: String,
    pub requester_id: String,
    pub chain_id: i64,
    pub deadline_timestamp: i64,
    pub storage_type: BlobStorageType,
    pub source_file_path: String,
    pub use_operator_wallet_id: Option<Uuid>,
}

#[derive(Deserialize, Serialize)]
pub struct BlobInputJsonFile {
    pub image_id: FixedBytes<32>,
    pub commitment: Vec<u8>,
    pub blob_sha2: FixedBytes<32>,
    pub blob_sidecar: BlobTransactionSidecarEip7594,
}

impl BlobTxRequestBody {
    pub fn into_db_input(
        &self,
        blob_input_json_file: &BlobInputJsonFile,
        signature: Vec<u8>,
    ) -> anyhow::Result<NewTxRequestWithTxInput> {
        Ok(NewTxRequestWithTxInput {
            new_tx_request: NewTxRequest {
                tx_id: self.tx_id.clone(),
                requester_id: self.requester_id.clone(),
                tx_status: TxStatus::SIGNED,
                tx_type: TxType::BLOB,
                chain_id: self.chain_id,
                use_operator_wallet_id: self.use_operator_wallet_id.clone(),
            },
            tx_input: NewTxInput::Blob(NewBlobTxInput {
                tx_id: self.tx_id.clone(),
                signature,
                commitment: blob_input_json_file.commitment.clone(),
                image_id: blob_input_json_file.image_id.to_vec(),
                blob_sha2: blob_input_json_file.blob_sha2.to_vec(),
                deadline_timestamp: self.deadline_timestamp,
                storage_type: self.storage_type.clone(),
                source_file_path: self.source_file_path.clone(),
            }),
        })
    }
}
