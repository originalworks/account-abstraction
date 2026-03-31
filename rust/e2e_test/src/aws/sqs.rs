use crate::tx_request::CreateTestTxRequestBody;
use crate::{
    constants::{SENDER_BLOB_QUEUE_NAME, SENDER_STANDARD_QUEUE_NAME},
    tx_request::TxRequestBodyOptional,
};
use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
use aws_sdk_sqs::types::QueueAttributeName;
use db_types::TxType;
use lambda_runtime::{Context, LambdaEvent};
use signer_queue::tx_request::TxRequestBody;
use std::env;

pub struct SenderQueueTestHelper {
    sqs_client: aws_sdk_sqs::Client,
    blob_queue_url: String,
    standard_queue_url: String,
}

impl SenderQueueTestHelper {
    pub async fn build(aws_config: aws_config::SdkConfig) -> anyhow::Result<Self> {
        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let create_blob_queue_response = sqs_client
            .create_queue()
            .queue_name(SENDER_BLOB_QUEUE_NAME)
            .attributes(QueueAttributeName::FifoQueue, "true")
            .attributes(QueueAttributeName::ContentBasedDeduplication, "true")
            .send()
            .await?;
        let blob_queue_url = create_blob_queue_response.queue_url.unwrap();

        let create_standard_queue_response = sqs_client
            .create_queue()
            .queue_name(SENDER_STANDARD_QUEUE_NAME)
            .attributes(QueueAttributeName::FifoQueue, "true")
            .attributes(QueueAttributeName::ContentBasedDeduplication, "true")
            .send()
            .await?;
        let standard_queue_url = create_standard_queue_response.queue_url.unwrap();

        unsafe {
            env::set_var("SENDER_STANDARD_QUEUE_URL", &standard_queue_url);
            env::set_var("SENDER_BLOB_QUEUE_URL", &blob_queue_url);
        }

        Ok(Self {
            sqs_client,
            blob_queue_url,
            standard_queue_url,
        })
    }

    pub async fn receive_messages(
        &self,
        queue_tx_type: TxType,
        limit: i32,
    ) -> anyhow::Result<LambdaEvent<SqsEvent>> {
        let mut queue_url = &String::new();
        if queue_tx_type == TxType::BLOB {
            queue_url = &self.blob_queue_url;
        } else {
            queue_url = &self.standard_queue_url;
        }
        let messages: Vec<TestEventMessage> = self
            .sqs_client
            .receive_message()
            .queue_url(queue_url)
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
}

// pub fn build_sqs_event(messages_body: Vec<String>) -> anyhow::Result<LambdaEvent<SqsEvent>> {
//     let mut sqs_event = SqsEvent::default();

//     for message_body in messages_body {
//         let mut sqs_message = SqsMessage::default();
//         sqs_message.body = Some(message_body);
//         // sqs_message.message_id
//         sqs_event.records.push(sqs_message);
//     }

//     let event = LambdaEvent::<SqsEvent>::new(sqs_event, Context::default());

//     Ok(event)
// }

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
