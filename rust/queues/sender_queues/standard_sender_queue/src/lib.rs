use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug)]
pub struct StandardSenderQueueMessageBody {
    pub tx_id: String,
}

#[derive(Debug)]
pub struct StandardSenderQueueMessage {
    pub message_id: String,
    pub body: StandardSenderQueueMessageBody,
}

#[derive(Debug)]
pub struct StandardSenderQueueEvent {
    pub messages: Vec<StandardSenderQueueMessage>,
    pub tx_id_to_message_id: HashMap<String, String>,
}

#[cfg(feature = "aws")]
mod aws {
    use std::collections::HashMap;

    use crate::StandardSenderQueueEvent;
    use crate::{StandardSenderQueueMessage, StandardSenderQueueMessageBody};
    use aws_lambda_events::sqs::SqsEvent;
    use lambda_runtime::LambdaEvent;
    use sqs_queue::event::{FromSqsRecord, build_typed_event};
    use sqs_queue::parser::parse_sqs_message_body;

    impl FromSqsRecord<StandardSenderQueueMessageBody> for StandardSenderQueueMessage {
        fn from_parts(message_id: String, body: StandardSenderQueueMessageBody) -> Self {
            Self { message_id, body }
        }
    }

    impl StandardSenderQueueEvent {
        pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Self> {
            let mut tx_id_to_message_id = HashMap::new();
            for record in &event.payload.records {
                let Some(message_body) =
                    parse_sqs_message_body::<StandardSenderQueueMessageBody>(&record)?
                else {
                    continue;
                };
                let Some(message_id) = record.message_id.clone() else {
                    continue;
                };
                tx_id_to_message_id.insert(message_body.tx_id.clone(), message_id.clone());
            }
            Ok(Self {
                messages: build_typed_event::<
                    StandardSenderQueueMessageBody,
                    StandardSenderQueueMessage,
                >(event)?,
                tx_id_to_message_id,
            })
        }
    }
}
