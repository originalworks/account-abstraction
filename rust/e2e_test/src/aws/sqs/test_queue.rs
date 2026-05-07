use crate::aws::sqs::event::{TestEventMessage, build_lambda_sqs_event};
use anyhow::bail;
use aws_lambda_events::sqs::SqsEvent;
use aws_sdk_sqs::types::QueueAttributeName;
use lambda_runtime::LambdaEvent;

use sqs_queue::queue::SqsQueue;

#[allow(async_fn_in_trait)]
pub trait SqsQueueTester {
    async fn receive_messages(&self, limit: i32) -> anyhow::Result<LambdaEvent<SqsEvent>>;
    async fn create_if_not_exist(
        sqs_client: &aws_sdk_sqs::Client,
        queue_name: String,
        message_group_id: String,
    ) -> anyhow::Result<SqsQueue>;
}

impl SqsQueueTester for SqsQueue {
    async fn receive_messages(&self, limit: i32) -> anyhow::Result<LambdaEvent<SqsEvent>> {
        let response = self
            .client
            .receive_message()
            .queue_url(&self.queue_url)
            .max_number_of_messages(limit)
            .wait_time_seconds(10)
            .visibility_timeout(30)
            .send()
            .await?;

        if let Some(messages) = response.messages.clone() {
            let test_event_messages: Vec<TestEventMessage> = response
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

            let lambda_sqs_event = build_lambda_sqs_event(test_event_messages)?;
            for msg in messages {
                if let Some(receipt_handle) = msg.receipt_handle {
                    self.client
                        .delete_message()
                        .queue_url(self.queue_url.clone())
                        .receipt_handle(receipt_handle)
                        .send()
                        .await?;
                }
            }

            return Ok(lambda_sqs_event);
        } else {
            bail!("No messages received");
        }
    }

    async fn create_if_not_exist(
        sqs_client: &aws_sdk_sqs::Client,
        queue_name: String,
        message_group_id: String,
    ) -> anyhow::Result<SqsQueue> {
        match sqs_client
            .get_queue_url()
            .queue_name(queue_name.clone())
            .send()
            .await
        {
            Ok(resp) => {
                return Ok(SqsQueue {
                    queue_url: resp.queue_url.unwrap(),
                    client: sqs_client.clone(),
                    message_group_id,
                });
            }

            Err(_) => {
                let create_queue_response = sqs_client
                    .create_queue()
                    .queue_name(queue_name)
                    .attributes(QueueAttributeName::FifoQueue, "true")
                    .attributes(QueueAttributeName::ContentBasedDeduplication, "true")
                    .send()
                    .await?;
                let queue_url = create_queue_response.queue_url.unwrap();

                return Ok(SqsQueue {
                    queue_url,
                    client: sqs_client.clone(),
                    message_group_id,
                });
            }
        }
    }
}
