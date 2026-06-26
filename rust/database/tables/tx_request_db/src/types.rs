use anyhow::bail;
use blob_tx_input_db::blob_tx_inputs::NewBlobTxInput;
use db_types::{BlobStorageType, TxStatus, TxType};
use serde::{Deserialize, Serialize};
use standard_tx_input_db::standard_tx_inputs::{NewStandardTxInput, StandardTxInput};
use time::OffsetDateTime;
use tx_input_types::TxInput;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StandardTxRequestRaw {
    pub sequence_id: i64,
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub chain_id: i64,
    pub use_operator_wallet_id: Option<Uuid>,
    pub attempts: i32,
    pub metadata: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,

    pub signature: Vec<u8>,
    pub calldata: Vec<u8>,
    pub to_address: String,
    pub value_wei: i64,
    pub deadline_timestamp: i64,
    pub pass_value_from_operator_wallet: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlobTxRequestRaw {
    pub sequence_id: i64,
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub chain_id: i64,
    pub use_operator_wallet_id: Option<Uuid>,
    pub attempts: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,

    pub signature: Vec<u8>,
    pub image_id: Vec<u8>,
    pub commitment: Vec<u8>,
    pub blob_sha2: Vec<u8>,
    pub deadline_timestamp: i64,
    pub source_file_path: String,
    pub storage_type: BlobStorageType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxRequestWithInput {
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub tx_input: TxInput,
    pub attempts: i32,
    pub metadata: Option<String>,
    pub use_operator_wallet_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct TxRequest {
    pub sequence_id: i64,
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub chain_id: i64,
    pub use_operator_wallet_id: Option<Uuid>,
    pub attempts: i32,
    pub metadata: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewTxRequest {
    pub tx_id: String,
    pub requester_id: String,
    pub tx_type: TxType,
    pub tx_status: TxStatus,
    pub chain_id: i64,
    pub use_operator_wallet_id: Option<Uuid>,
    pub metadata: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NewTxInput {
    Blob(NewBlobTxInput),
    Standard(NewStandardTxInput),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewTxRequestWithTxInput {
    pub new_tx_request: NewTxRequest,
    pub tx_input: NewTxInput,
}

pub trait IntoTxRequestWithInput {
    fn into_tx_request_with_input(&self) -> anyhow::Result<TxRequestWithInput>;
}

impl IntoTxRequestWithInput for StandardTxRequestRaw {
    fn into_tx_request_with_input(&self) -> anyhow::Result<TxRequestWithInput> {
        if self.tx_type == TxType::BLOB {
            bail!("Trying to parse StandardTxRequestRaw into BLOB tx");
        } else {
            let tx_input = TxInput::Standard(StandardTxInput {
                tx_id: self.tx_id.clone(),
                signature: self.signature.clone(),
                calldata: self.calldata.clone(),
                to_address: self.to_address.clone(),
                value_wei: self.value_wei.clone(),
                deadline_timestamp: self.deadline_timestamp.clone(),
                pass_value_from_operator_wallet: self.pass_value_from_operator_wallet.clone(),
                created_at: self.created_at.clone(),
            });
            Ok(TxRequestWithInput {
                tx_id: self.tx_id.clone(),
                requester_id: self.requester_id.clone(),
                tx_type: self.tx_type.clone(),
                tx_status: self.tx_status.clone(),
                attempts: self.attempts,
                tx_input,
                metadata: self.metadata.clone(),
                use_operator_wallet_id: self.use_operator_wallet_id.clone(),
            })
        }
    }
}
