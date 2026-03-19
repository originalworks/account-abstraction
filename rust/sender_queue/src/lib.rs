#[cfg(feature = "aws")]
pub mod sqs;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct TxSenderQueueMessageBody {
    pub tx_id: String,
}

pub struct TxSenderQueueMessage {
    pub message_id: String,
    pub body: TxSenderQueueMessageBody,
}

pub struct TxSenderQueueEvent {
    pub messages: Vec<TxSenderQueueMessage>,
    pub tx_id_to_message_id: HashMap<String, String>,
}
