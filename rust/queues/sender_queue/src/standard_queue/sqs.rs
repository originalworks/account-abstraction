use crate::standard_queue::{
    SenderQueueStandardEvent, SenderQueueStandardMessageBody, SenderStandardQueueMessage,
};
use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
use lambda_runtime::{LambdaEvent, tracing::log::warn};
use std::collections::HashMap;

pub struct SenderStandardSqsQueue {
    client: aws_sdk_sqs::Client,
    transaction_sender_queue_url: String,
    message_group_id: String,
}

impl SenderStandardSqsQueue {
    pub fn build(
        aws_config: &aws_config::SdkConfig,
        transaction_sender_queue_url: &String,
        message_group_id: &String,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            client: aws_sdk_sqs::Client::new(aws_config),
            transaction_sender_queue_url: transaction_sender_queue_url.clone(),
            message_group_id: message_group_id.clone(),
        })
    }

    pub async fn send_new(
        &self,
        message_body: &SenderQueueStandardMessageBody,
    ) -> anyhow::Result<()> {
        let sqs_message_body = serde_json::json!(message_body).to_string();
        let response = self
            .client
            .send_message()
            .queue_url(&self.transaction_sender_queue_url)
            .message_body(sqs_message_body)
            .message_group_id(&self.message_group_id)
            .send()
            .await?;
        println!("{response:?}");
        Ok(())
    }
}

impl SenderQueueStandardMessageBody {
    pub fn from_sqs_message(message: &SqsMessage) -> anyhow::Result<Option<Self>> {
        let body = match message.body.clone() {
            Some(b) => b,
            None => {
                warn!("Missing message body for: {:?}", &message);
                return Ok(None);
            }
        };
        let queue_message_body =
            match serde_json::from_str::<SenderQueueStandardMessageBody>(&body).ok() {
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

impl SenderQueueStandardEvent {
    pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Self> {
        let mut messages = Vec::new();
        let mut tx_id_to_message_id = HashMap::new();
        for record in event.payload.records {
            let Some(message_body) = SenderQueueStandardMessageBody::from_sqs_message(&record)?
            else {
                continue;
            };
            let Some(message_id) = record.message_id else {
                continue;
            };
            tx_id_to_message_id.insert(message_body.tx_id.clone(), message_id.clone());
            messages.push(SenderStandardQueueMessage {
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
