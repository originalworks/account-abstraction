#[cfg(feature = "aws")]
pub mod sqs;

use execution_attempt_db::execution_attempts::TxExecutionOutcome;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct RetryQueueMessageBody {
    pub execution_attempt_id: String,
    pub execution_outcome: TxExecutionOutcome,
}

#[derive(Debug)]
pub struct RetryQueueMessage {
    pub message_id: String,
    pub body: RetryQueueMessageBody,
}

#[derive(Debug)]
pub struct RetryEvent {
    pub messages: Vec<RetryQueueMessage>,
}
