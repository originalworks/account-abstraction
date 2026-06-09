pub mod event;
pub mod test_queue;

use crate::constants::{
    BLOB_SENDER_QUEUE_NAME, RECEIPT_POLLER_QUEUE_NAME, RETRY_QUEUE_NAME,
    STANDARD_SENDER_QUEUE_NAME, TX_OUTCOME_QUEUE_NAME,
};
use sqs_queue::queue::SqsQueue;
use std::env;
use test_queue::SqsQueueTester;

pub struct TestQueueManager {
    pub sqs_client: aws_sdk_sqs::Client,
    pub blob_sender_queue: SqsQueue,
    pub standard_sender_queue: SqsQueue,
    pub receipt_poller_queue: SqsQueue,
    pub retry_queue: SqsQueue,
    pub tx_outcome_queue: SqsQueue,
}

impl TestQueueManager {
    pub async fn build(aws_config: &aws_config::SdkConfig) -> anyhow::Result<Self> {
        let blob_sender_queue_message_group_id =
            env::var("BLOB_SENDER_QUEUE_MESSAGE_GROUP_ID").unwrap();
        let receipt_poller_queue_message_group_id =
            env::var("RECEIPT_POLLER_QUEUE_MESSAGE_GROUP_ID").unwrap();
        let retry_queue_message_group_id = env::var("RETRY_QUEUE_MESSAGE_GROUP_ID").unwrap();
        let standard_sender_queue_message_group_id =
            env::var("STANDARD_SENDER_QUEUE_MESSAGE_GROUP_ID").unwrap();

        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);

        let blob_sender_queue = SqsQueue::create_if_not_exist(
            &sqs_client,
            BLOB_SENDER_QUEUE_NAME.to_string(),
            blob_sender_queue_message_group_id,
        )
        .await?;

        let standard_sender_queue = SqsQueue::create_if_not_exist(
            &sqs_client,
            STANDARD_SENDER_QUEUE_NAME.to_string(),
            standard_sender_queue_message_group_id,
        )
        .await?;

        let receipt_poller_queue = SqsQueue::create_if_not_exist(
            &sqs_client,
            RECEIPT_POLLER_QUEUE_NAME.to_string(),
            receipt_poller_queue_message_group_id,
        )
        .await?;

        let retry_queue = SqsQueue::create_if_not_exist(
            &sqs_client,
            RETRY_QUEUE_NAME.to_string(),
            retry_queue_message_group_id,
        )
        .await?;

        let tx_outcome_queue = SqsQueue::create_outcome_queue_if_not_exist(&sqs_client).await?;

        unsafe {
            env::set_var(
                "STANDARD_SENDER_QUEUE_URL",
                &standard_sender_queue.queue_url,
            );
            env::set_var("BLOB_SENDER_QUEUE_URL", &blob_sender_queue.queue_url);
            env::set_var("RECEIPT_POLLER_QUEUE_URL", &receipt_poller_queue.queue_url);
            env::set_var("RETRY_QUEUE_URL", &retry_queue.queue_url);
        }

        Ok(Self {
            sqs_client: sqs_client.clone(),
            retry_queue,
            receipt_poller_queue,
            blob_sender_queue,
            standard_sender_queue,
            tx_outcome_queue,
        })
    }
}
