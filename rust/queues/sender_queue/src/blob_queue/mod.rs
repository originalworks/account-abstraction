#[cfg(feature = "aws")]
pub mod sqs;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct SenderQueueBlobMessageBody {
    pub tx_id: String,
}

pub struct SenderBlobQueueMessage {
    pub message_id: String,
    pub body: SenderQueueBlobMessageBody,
}

pub struct SenderQueueBlobEvent {
    pub messages: Vec<SenderBlobQueueMessage>,
    pub tx_id_to_message_id: HashMap<String, String>,
}
