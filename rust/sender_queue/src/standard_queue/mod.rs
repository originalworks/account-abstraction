#[cfg(feature = "aws")]
pub mod sqs;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct SenderQueueStandardMessageBody {
    pub tx_id: String,
}

pub struct SenderStandardQueueMessage {
    pub message_id: String,
    pub body: SenderQueueStandardMessageBody,
}

pub struct SenderQueueStandardEvent {
    pub messages: Vec<SenderStandardQueueMessage>,
    pub tx_id_to_message_id: HashMap<String, String>,
}
