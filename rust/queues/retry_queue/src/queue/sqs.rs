use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
use lambda_runtime::{LambdaEvent, tracing::log::warn};

use crate::queue::{RetryEvent, RetryQueueMessage, RetryQueueMessageBody};

pub struct RetrySqsQueue {
    pub client: aws_sdk_sqs::Client,
    pub queue_url: String,
    pub message_group_id: String,
}

impl RetrySqsQueue {
    pub fn build(
        aws_config: &aws_config::SdkConfig,
        queue_url: &String,
        message_group_id: &String,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            client: aws_sdk_sqs::Client::new(aws_config),
            queue_url: queue_url.clone(),
            message_group_id: message_group_id.clone(),
        })
    }

    pub async fn send_new(&self, message_body: &RetryQueueMessageBody) -> anyhow::Result<()> {
        let sqs_message_body = serde_json::json!(message_body).to_string();
        let response = self
            .client
            .send_message()
            .queue_url(&self.queue_url)
            .message_body(sqs_message_body)
            .message_group_id(&self.message_group_id)
            .send()
            .await?;
        println!("{response:?}");
        Ok(())
    }
}

impl RetryQueueMessageBody {
    pub fn from_sqs_message(message: &SqsMessage) -> anyhow::Result<Option<Self>> {
        let body = match message.body.clone() {
            Some(b) => b,
            None => {
                warn!("Missing message body for: {:?}", &message);
                return Ok(None);
            }
        };
        let queue_message_body = match serde_json::from_str::<RetryQueueMessageBody>(&body).ok() {
            Some(v) => v,
            None => {
                warn!("Failed to parse queue message body: {:?}", body);
                return Ok(None);
            }
        };

        Ok(Some(queue_message_body))
    }

    pub fn from_sqs_message_vec(messages: &Vec<SqsMessage>) -> anyhow::Result<Vec<Self>> {
        let mut output = Vec::new();
        for message in messages {
            if let Some(body) = Self::from_sqs_message(message)? {
                output.push(body);
            }
        }
        Ok(output)
    }
}

impl RetryEvent {
    pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<RetryEvent> {
        let mut messages = Vec::new();

        for record in event.payload.records {
            let Some(message_body) = RetryQueueMessageBody::from_sqs_message(&record)? else {
                continue;
            };
            let Some(message_id) = record.message_id else {
                continue;
            };

            messages.push(RetryQueueMessage {
                message_id,
                body: message_body,
            });
        }
        Ok(Self { messages })
    }
}
