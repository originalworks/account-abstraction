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

#[cfg(feature = "aws")]
mod aws {
    use crate::ReceiptPollerEvent;
    use crate::{ReceiptPollerQueueMessage, ReceiptPollerQueueMessageBody};
    use aws_lambda_events::sqs::SqsEvent;
    use lambda_runtime::LambdaEvent;
    use sqs_queue::event::{FromSqsRecord, build_typed_event};

    impl FromSqsRecord<ReceiptPollerQueueMessageBody> for ReceiptPollerQueueMessage {
        fn from_parts(message_id: String, body: ReceiptPollerQueueMessageBody) -> Self {
            Self { message_id, body }
        }
    }

    impl ReceiptPollerEvent {
        pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Self> {
            Ok(Self {
                messages: build_typed_event::<
                    ReceiptPollerQueueMessageBody,
                    ReceiptPollerQueueMessage,
                >(event)?,
            })
        }
    }
}
