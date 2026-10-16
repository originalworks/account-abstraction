use serde_json::from_str;
use standard_sender_queue::StandardSenderQueueMessageBody;
use std::collections::HashSet;
use test_fixture::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use tx_request::standard::StandardTxRequestBody;

pub async fn successful_sign_single_request_sends_sender_message(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!(
        "Verifies that successfully signing a single request sends a message with the expected tx_id to the sender queue"
    );

    // Make sure the queue is empty before starting the test
    e2e_test_fixture
        .test_queue_manager
        .standard_sender_queue
        .clear_queue()
        .await?;

    let tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &tx_request_body.to_string(),
        None,
    )])?;

    e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(tx_request_event)
        .await
        .expect("standard_tx_signer function_handler failed");

    let sender_queue_event = e2e_test_fixture
        .test_queue_manager
        .standard_sender_queue
        .receive_messages(5)
        .await?;

    let messages = sender_queue_event.payload.records;

    assert_eq!(
        messages.len(),
        1,
        "Expected exactly one message in the sender queue"
    );

    let message = &messages[0];
    let message_body: StandardSenderQueueMessageBody =
        from_str(message.body.as_deref().expect("Expected message body"))?;

    assert_eq!(
        message_body.tx_id, tx_request_body.tx_id,
        "Sender queue message should contain the signed request's tx_id"
    );

    println!("{} PASSED", module_path!());
    Ok(())
}

pub async fn successful_sign_multiple_requests_sends_sender_messages(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!(
        "Verifies that successfully signing multiple requests sends one corresponding message per request to the sender queue"
    );

    // Make sure the queue is empty before starting the test
    e2e_test_fixture
        .test_queue_manager
        .standard_sender_queue
        .clear_queue()
        .await?;

    let tx_request_bodies: Vec<StandardTxRequestBody> = (0..3)
        .map(|_| {
            StandardTxRequestBody::test_build(StandardTxRequestBodyOptional::default(
                e2e_test_fixture.env_vars.anvil_chain_id,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let messages = tx_request_bodies
        .iter()
        .map(|body| TestEventMessage::new(&body.to_string(), None))
        .collect();

    let tx_request_event = build_lambda_sqs_event(messages)?;

    e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(tx_request_event)
        .await
        .expect("standard_tx_signer function_handler failed");

    let sender_queue_event = e2e_test_fixture
        .test_queue_manager
        .standard_sender_queue
        .receive_messages(5)
        .await?;

    let received_messages = sender_queue_event.payload.records;

    assert_eq!(
        received_messages.len(),
        tx_request_bodies.len(),
        "Expected one sender queue message for each successfully signed request"
    );

    let received_tx_ids: HashSet<String> = received_messages
        .iter()
        .map(|message| {
            let body: StandardSenderQueueMessageBody =
                from_str(message.body.as_deref().expect("Expected message body"))
                    .expect("Expected sender queue message to contain valid JSON");

            body.tx_id
        })
        .collect();

    let expected_tx_ids: HashSet<String> = tx_request_bodies
        .iter()
        .map(|body| body.tx_id.clone())
        .collect();

    assert_eq!(
        received_tx_ids, expected_tx_ids,
        "Sender queue messages should contain the tx_id of every signed request"
    );

    println!("{} PASSED", module_path!());
    Ok(())
}
