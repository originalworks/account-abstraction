pub struct SqsQueue {
    pub client: aws_sdk_sqs::Client,
    pub queue_url: String,
    pub message_group_id: String,
}

impl SqsQueue {
    pub fn build(
        aws_config: &aws_config::SdkConfig,
        queue_url: &str,
        message_group_id: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            client: aws_sdk_sqs::Client::new(aws_config),
            queue_url: queue_url.to_string(),
            message_group_id: message_group_id.to_string(),
        })
    }

    pub async fn send_new(&self, message_body_string: &String) -> anyhow::Result<()> {
        let response = self
            .client
            .send_message()
            .queue_url(&self.queue_url)
            .message_body(message_body_string)
            .message_group_id(&self.message_group_id)
            .send()
            .await?;

        println!("{response:?}");
        Ok(())
    }
}
