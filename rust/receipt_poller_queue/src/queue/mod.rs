#[cfg(feature = "aws")]
pub mod sqs;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct ReceiptPollerQueueMessageBody {
    pub execution_attempt_id: String,
}

#[derive(Debug)]
pub struct ReceiptPollerQueueMessage {
    pub message_id: String,
    pub body: ReceiptPollerQueueMessageBody,
}

#[derive(Debug)]
pub struct ReceiptPollerEvent {
    pub messages: Vec<ReceiptPollerQueueMessage>,
}
