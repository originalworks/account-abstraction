pub mod receipt_poller_queue;
pub mod sender_queue;

use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
use lambda_runtime::{Context, LambdaEvent};

pub fn build_lambda_sqs_event(
    messages: Vec<TestEventMessage>,
) -> anyhow::Result<LambdaEvent<SqsEvent>> {
    let mut sqs_event = SqsEvent::default();
    for message in messages {
        sqs_event.records.push(message.into_sqs_message());
    }
    let event = LambdaEvent::<SqsEvent>::new(sqs_event, Context::default());

    Ok(event)
}

pub struct TestEventMessage {
    body: String,
    message_id: String,
}

impl TestEventMessage {
    pub fn new(body: &String, message_id: Option<String>) -> Self {
        Self {
            body: body.clone(),
            message_id: message_id
                .clone()
                .unwrap_or(uuid::Uuid::new_v4().to_string()),
        }
    }
    pub fn into_sqs_message(&self) -> SqsMessage {
        let mut sqs_message = SqsMessage::default();
        sqs_message.body = Some(self.body.clone());
        sqs_message.message_id = Some(self.message_id.clone());
        sqs_message
    }
}
