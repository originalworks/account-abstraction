use alloy::primitives::Address;
use db_types::{BlobStorageType, TxType};
use tx_request::{blob_tx::BlobTxRequestBody, standard::StandardTxRequestBody};
// use signer_queue::tx_request::TxRequestBody;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub struct StandardTxRequestBodyOptional {
    pub tx_id: Option<String>,
    pub requester_id: Option<String>,
    pub calldata: Option<String>,
    pub to_address: Option<String>,
    pub value_wei: Option<i64>,
    pub chain_id: i64,
    pub deadline_timestamp: Option<i64>,
    pub pass_value_from_operator_wallet: Option<bool>,
    pub use_operator_wallet_id: Option<Uuid>,
}

impl StandardTxRequestBodyOptional {
    pub fn default(chain_id: i64) -> Self {
        Self {
            tx_id: None,
            requester_id: None,
            calldata: None,
            to_address: None,
            value_wei: None,
            chain_id,
            deadline_timestamp: None,
            pass_value_from_operator_wallet: None,
            use_operator_wallet_id: None,
        }
    }
}

pub trait StandardTxRequestBodyForTest {
    fn test_build(input: StandardTxRequestBodyOptional) -> anyhow::Result<StandardTxRequestBody>;
    fn to_string(&self) -> String;
}

impl StandardTxRequestBodyForTest for StandardTxRequestBody {
    fn test_build(input: StandardTxRequestBodyOptional) -> anyhow::Result<StandardTxRequestBody> {
        let tx_id = Uuid::new_v4().to_string();
        let random_address = Address::random();
        let default_tx = "".to_string();
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let deadline_timestamp = current_timestamp + 3600;

        Ok(StandardTxRequestBody {
            tx_id: input.tx_id.unwrap_or(tx_id),
            requester_id: input.requester_id.unwrap_or("requester-1".to_string()),
            calldata: input.calldata.unwrap_or(default_tx),
            to_address: input.to_address.unwrap_or(random_address.to_string()),
            value_wei: input.value_wei.unwrap_or(0),
            chain_id: input.chain_id,
            deadline_timestamp: input
                .deadline_timestamp
                .unwrap_or(deadline_timestamp.try_into()?),
            pass_value_from_operator_wallet: input.pass_value_from_operator_wallet.unwrap_or(false),
            use_operator_wallet_id: input.use_operator_wallet_id,
        })
    }

    fn to_string(&self) -> String {
        serde_json::json!(self).to_string()
    }
}

pub struct BlobTxRequestBodyOptional {
    pub tx_id: Option<String>,
    pub requester_id: Option<String>,
    pub chain_id: i64,
    pub deadline_timestamp: Option<i64>,
    pub storage_type: Option<BlobStorageType>,
    pub source_file_path: String,
    pub use_operator_wallet_id: Option<Uuid>,
}

impl BlobTxRequestBodyOptional {
    pub fn default(chain_id: i64, source_file_path: String) -> Self {
        Self {
            tx_id: None,
            requester_id: None,
            source_file_path,
            chain_id,
            deadline_timestamp: None,
            storage_type: None,
            use_operator_wallet_id: None,
        }
    }
}

pub trait BlobTxRequestBodyForTest {
    fn test_build(input: BlobTxRequestBodyOptional) -> anyhow::Result<BlobTxRequestBody>;
    fn to_string(&self) -> String;
}

impl BlobTxRequestBodyForTest for BlobTxRequestBody {
    fn test_build(input: BlobTxRequestBodyOptional) -> anyhow::Result<BlobTxRequestBody> {
        let tx_id = Uuid::new_v4().to_string();
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let deadline_timestamp = current_timestamp + 3600;

        Ok(BlobTxRequestBody {
            tx_id: input.tx_id.unwrap_or(tx_id),
            requester_id: input.requester_id.unwrap_or("requester-1".to_string()),
            chain_id: input.chain_id,
            deadline_timestamp: input
                .deadline_timestamp
                .unwrap_or(deadline_timestamp.try_into()?),
            use_operator_wallet_id: input.use_operator_wallet_id,
            storage_type: input.storage_type.unwrap_or(BlobStorageType::S3),
            source_file_path: input.source_file_path,
        })
    }

    fn to_string(&self) -> String {
        serde_json::json!(self).to_string()
    }
}
