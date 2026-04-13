use std::env;

use crate::{
    aws::sqs::{TestEventMessage, build_lambda_sqs_event},
    constants::RECEIPT_POLLER_QUEUE_NAME,
};
use aws_lambda_events::sqs::SqsEvent;

use aws_sdk_sqs::types::QueueAttributeName;
use lambda_runtime::LambdaEvent;
use receipt_poller_queue::queue::sqs::ReceiptPollerSqsQueue;

#[allow(async_fn_in_trait)]
pub trait ReceiptPollerTestQueue {
    async fn receive_messages(&self, limit: i32) -> anyhow::Result<LambdaEvent<SqsEvent>>;
    async fn create_and_build(
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<ReceiptPollerSqsQueue>;
}

impl ReceiptPollerTestQueue for ReceiptPollerSqsQueue {
    async fn receive_messages(&self, limit: i32) -> anyhow::Result<LambdaEvent<SqsEvent>> {
        let messages: Vec<TestEventMessage> = self
            .client
            .receive_message()
            .queue_url(&self.queue_url)
            .max_number_of_messages(limit)
            .wait_time_seconds(10)
            .visibility_timeout(30)
            .send()
            .await?
            .messages
            .unwrap()
            .iter()
            .map(|message| {
                TestEventMessage::new(
                    &message.body.clone().unwrap(),
                    Some(message.message_id.clone().unwrap()),
                )
            })
            .collect();

        let lambda_sqs_event = build_lambda_sqs_event(messages)?;

        return Ok(lambda_sqs_event);
    }

    async fn create_and_build(
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<ReceiptPollerSqsQueue> {
        let sqs_client = aws_sdk_sqs::Client::new(aws_config);

        let create_queue_response = sqs_client
            .create_queue()
            .queue_name(RECEIPT_POLLER_QUEUE_NAME)
            .attributes(QueueAttributeName::FifoQueue, "true")
            .attributes(QueueAttributeName::ContentBasedDeduplication, "true")
            .send()
            .await?;
        let queue_url = create_queue_response.queue_url.unwrap();

        unsafe {
            env::set_var("RECEIPT_POLLER_QUEUE_URL", &queue_url);
        }

        let message_group_id = env::var("RECEIPT_POLLER_QUEUE_MESSAGE_GROUP_ID")
            .expect(format!("Missing env variable RECEIPT_POLLER_QUEUE_MESSAGE_GROUP_ID").as_str());

        Ok(ReceiptPollerSqsQueue {
            client: sqs_client,
            queue_url,
            message_group_id,
        })
    }
}
