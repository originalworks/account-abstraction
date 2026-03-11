use aws_lambda_events::sqs::SqsMessage;
use lambda_runtime::tracing::log::warn;

use crate::TxSenderQueueMessageBody;

pub struct TxSenderSqsQueue {
    client: aws_sdk_sqs::Client,
    transaction_sender_queue_url: String,
}

impl TxSenderSqsQueue {
    pub fn build(
        aws_config: &aws_config::SdkConfig,
        transaction_sender_queue_url: &String,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            client: aws_sdk_sqs::Client::new(aws_config),
            transaction_sender_queue_url: transaction_sender_queue_url.clone(),
        })
    }

    pub async fn send_new_trigger(
        &self,
        message_body: &TxSenderQueueMessageBody,
    ) -> anyhow::Result<()> {
        let sqs_message_body = serde_json::json!(message_body).to_string();
        let response = self
            .client
            .send_message()
            .queue_url(&self.transaction_sender_queue_url)
            .message_body(sqs_message_body)
            .send()
            .await?;
        println!("{response:?}");
        Ok(())
    }
}

impl TxSenderQueueMessageBody {
    pub fn from_sqs_message(message: &SqsMessage) -> anyhow::Result<Option<Self>> {
        let body = match message.body.clone() {
            Some(b) => b,
            None => {
                warn!("Missing message body for: {:?}", &message);
                return Ok(None);
            }
        };
        let queue_message_body = match serde_json::from_str::<TxSenderQueueMessageBody>(&body).ok()
        {
            Some(v) => v,
            None => {
                warn!("Failed to parse queue message body body: {:?}", body);
                return Ok(None);
            }
        };

        Ok(Some(queue_message_body))
    }
}
