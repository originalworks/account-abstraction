use crate::constants::{SENDER_BLOB_QUEUE_NAME, SENDER_STANDARD_QUEUE_NAME};
use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
use lambda_runtime::{Context, LambdaEvent};
use signer_queue::tx_request::TxRequestBody;
use std::env;

async fn create_queue(
    aws_client: &aws_sdk_sqs::Client,
    queue_name: &String,
) -> anyhow::Result<String> {
    let create_queue_output = aws_client
        .create_queue()
        .queue_name(queue_name)
        .send()
        .await?;
    let queue_url = create_queue_output.queue_url.unwrap();

    Ok(queue_url)
}

pub async fn create_sender_queues(sqs_client: aws_sdk_sqs::Client) -> anyhow::Result<()> {
    let sender_standard_queue_url =
        create_queue(&sqs_client, &SENDER_STANDARD_QUEUE_NAME.to_string()).await?;
    let sender_blob_queue_url =
        create_queue(&sqs_client, &SENDER_BLOB_QUEUE_NAME.to_string()).await?;

    unsafe {
        env::set_var("SENDER_STANDARD_QUEUE_URL", &sender_standard_queue_url);
        env::set_var("SENDER_BLOB_QUEUE_URL", &sender_blob_queue_url);
    }

    Ok(())
}

pub fn build_transfer_tx_request_event(
    tx_request_body: TxRequestBody,
) -> anyhow::Result<LambdaEvent<SqsEvent>> {
    let sqs_message_body = serde_json::json!(tx_request_body).to_string();
    let mut sqs_message = SqsMessage::default();
    sqs_message.body = Some(sqs_message_body);

    let mut sqs_event = SqsEvent::default();
    sqs_event.records.push(sqs_message);

    let event = LambdaEvent::<SqsEvent>::new(sqs_event, Context::default());

    Ok(event)
}
