use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct RetryQueueMessageBody {
    pub execution_attempt_id: String,
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

#[cfg(feature = "aws")]
mod aws {
    use crate::RetryEvent;
    use crate::{RetryQueueMessage, RetryQueueMessageBody};
    use aws_lambda_events::sqs::SqsEvent;
    use lambda_runtime::LambdaEvent;
    use sqs_queue::event::{FromSqsRecord, build_typed_event};

    impl FromSqsRecord<RetryQueueMessageBody> for RetryQueueMessage {
        fn from_parts(message_id: String, body: RetryQueueMessageBody) -> Self {
            Self { message_id, body }
        }
    }

    impl RetryEvent {
        pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Self> {
            Ok(Self {
                messages: build_typed_event::<RetryQueueMessageBody, RetryQueueMessage>(event)?,
            })
        }
    }
}
