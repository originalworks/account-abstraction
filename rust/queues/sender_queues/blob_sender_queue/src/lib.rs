use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct BlobSenderQueueMessageBody {
    pub tx_id: String,
}

pub struct BlobSenderQueueMessage {
    pub message_id: String,
    pub body: BlobSenderQueueMessageBody,
}

pub struct BlobSenderQueueEvent {
    pub messages: Vec<BlobSenderQueueMessage>,
    pub tx_id_to_message_id: HashMap<String, String>,
}

#[cfg(feature = "aws")]
mod aws {
    use std::collections::HashMap;

    use crate::BlobSenderQueueEvent;
    use crate::{BlobSenderQueueMessage, BlobSenderQueueMessageBody};
    use aws_lambda_events::sqs::SqsEvent;
    use lambda_runtime::LambdaEvent;
    use sqs_queue::event::{FromSqsRecord, build_typed_event};
    use sqs_queue::parser::parse_sqs_message_body;

    impl FromSqsRecord<BlobSenderQueueMessageBody> for BlobSenderQueueMessage {
        fn from_parts(message_id: String, body: BlobSenderQueueMessageBody) -> Self {
            Self { message_id, body }
        }
    }

    impl BlobSenderQueueEvent {
        pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Self> {
            let mut tx_id_to_message_id = HashMap::new();
            for record in &event.payload.records {
                let Some(message_body) =
                    parse_sqs_message_body::<BlobSenderQueueMessageBody>(&record)?
                else {
                    continue;
                };
                let Some(message_id) = record.message_id.clone() else {
                    continue;
                };
                tx_id_to_message_id.insert(message_body.tx_id.clone(), message_id.clone());
            }
            Ok(Self {
                messages: build_typed_event::<BlobSenderQueueMessageBody, BlobSenderQueueMessage>(
                    event,
                )?,
                tx_id_to_message_id,
            })
        }
    }
}
