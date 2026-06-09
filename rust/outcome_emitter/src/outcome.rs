use db_types::TxExecutionOutcome;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OutcomeEvent {
    pub outcome: TxExecutionOutcome,
    pub tx_request_id: String,
    pub gas_fee: Option<i64>,
    pub transaction_hash: Option<String>,
    pub error: Option<String>,
    pub metadata: Option<String>,
}
