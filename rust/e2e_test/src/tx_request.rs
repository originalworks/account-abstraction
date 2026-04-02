use alloy::primitives::Address;
use db_types::TxType;
use signer_queue::tx_request::TxRequestBody;
use uuid::Uuid;

pub struct TxRequestBodyOptional {
    pub tx_id: Option<String>,
    pub requester_id: Option<String>,
    pub tx_type: TxType,
    pub calldata: Option<String>,
    pub to_address: Option<String>,
    pub value_wei: Option<i64>,
    pub chain_id: i64,
    pub pass_value_from_operator_wallet: Option<bool>,
    pub blob_file_path: Option<String>,
    pub use_operator_wallet_id: Option<Uuid>,
}

impl TxRequestBodyOptional {
    pub fn default(tx_type: TxType, chain_id: i64) -> Self {
        Self {
            tx_id: None,
            requester_id: None,
            tx_type,
            calldata: None,
            to_address: None,
            value_wei: None,
            chain_id,
            pass_value_from_operator_wallet: None,
            blob_file_path: None,
            use_operator_wallet_id: None,
        }
    }
}

pub trait CreateTestTxRequestBody {
    fn build_test_tx_request_body(input: TxRequestBodyOptional) -> anyhow::Result<TxRequestBody>;
    fn to_string(&self) -> String;
}

impl CreateTestTxRequestBody for TxRequestBody {
    fn build_test_tx_request_body(input: TxRequestBodyOptional) -> anyhow::Result<TxRequestBody> {
        let tx_id = Uuid::new_v4().to_string();
        let random_address = Address::random();
        let default_tx = "".to_string();
        Ok(TxRequestBody {
            tx_id: input.tx_id.unwrap_or(tx_id),
            requester_id: input.requester_id.unwrap_or("requester-1".to_string()),
            tx_type: input.tx_type,
            calldata: input.calldata.unwrap_or(default_tx),
            to_address: input.to_address.unwrap_or(random_address.to_string()),
            value_wei: input.value_wei.unwrap_or(0),
            chain_id: input.chain_id,
            pass_value_from_operator_wallet: input.pass_value_from_operator_wallet.unwrap_or(false),
            blob_file_path: input.blob_file_path,
            use_operator_wallet_id: input.use_operator_wallet_id,
        })
    }

    fn to_string(&self) -> String {
        serde_json::json!(self).to_string()
    }
}
