use db_types::BlobStorageType;
use serde::{Deserialize, Serialize};
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
