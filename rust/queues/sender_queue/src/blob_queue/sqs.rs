use crate::blob_queue::{SenderBlobQueueMessage, SenderQueueBlobEvent, SenderQueueBlobMessageBody};
use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
use lambda_runtime::{LambdaEvent, tracing::log::warn};
use std::collections::HashMap;

pub struct SenderBlobSqsQueue {
    client: aws_sdk_sqs::Client,
    queue_url: String,
    message_group_id: String,
}

impl SenderBlobSqsQueue {
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

    pub async fn send_new(&self, message_body: &SenderQueueBlobMessageBody) -> anyhow::Result<()> {
        let sqs_message_body = serde_json::json!(message_body).to_string();
        let response = self
            .client
            .send_message()
            .queue_url(&self.queue_url)
            .message_body(sqs_message_body)
            .message_group_id(&self.message_group_id)
            .send()
            .await?;

        Ok(())
    }
}

impl SenderQueueBlobMessageBody {
    pub fn from_sqs_message(message: &SqsMessage) -> anyhow::Result<Option<Self>> {
        let body = match message.body.clone() {
            Some(b) => b,
            None => {
                warn!("Missing message body for: {:?}", &message);
                return Ok(None);
            }
        };
        let queue_message_body =
            match serde_json::from_str::<SenderQueueBlobMessageBody>(&body).ok() {
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

impl SenderQueueBlobEvent {
    pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Self> {
        let mut messages = Vec::new();
        let mut tx_id_to_message_id = HashMap::new();
        for record in event.payload.records {
            let Some(message_body) = SenderQueueBlobMessageBody::from_sqs_message(&record)? else {
                continue;
            };
            let Some(message_id) = record.message_id else {
                continue;
            };
            tx_id_to_message_id.insert(message_body.tx_id.clone(), message_id.clone());
            messages.push(SenderBlobQueueMessage {
                message_id,
                body: message_body,
            });
        }
        Ok(Self {
            messages,
            tx_id_to_message_id,
        })
    }
}
