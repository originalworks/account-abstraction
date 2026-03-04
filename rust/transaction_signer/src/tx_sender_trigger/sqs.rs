use crate::tx_sender_trigger::TriggerBody;

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

    pub async fn send_new_trigger(&self, trigger_body: &TriggerBody) -> anyhow::Result<()> {
        let sqs_message_body = serde_json::json!(trigger_body).to_string();
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
