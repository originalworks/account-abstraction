#[cfg(feature = "aws")]
pub mod sqs;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct TxSenderQueueMessageBody {
    pub tx_id: String,
}
