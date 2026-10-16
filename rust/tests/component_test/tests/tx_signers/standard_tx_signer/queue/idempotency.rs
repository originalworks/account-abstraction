use serde_json::from_str;
use standard_sender_queue::StandardSenderQueueMessageBody;
use test_fixture::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use tx_request::standard::StandardTxRequestBody;

pub async fn duplicates(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!(
        "Verifies that processing the same request twice does not produce duplicate sender queue messages"
    );

    let sender_queue = &e2e_test_fixture.test_queue_manager.standard_sender_queue;

    // Ensure the queue contains no messages from previous tests.
    sender_queue.clear_queue().await?;

    let tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &tx_request_body.to_string(),
        None,
    )])?;

    // First processing should send a sender message.
    e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(tx_request_event.clone())
        .await
        .expect("standard_tx_signer function_handler failed");

    let first_queue_event = sender_queue.receive_messages(5).await?;
    let first_messages = first_queue_event.payload.records;

    assert_eq!(
        first_messages.len(),
        1,
        "Expected one sender queue message after the first processing"
    );

    let first_message_body: StandardSenderQueueMessageBody = from_str(
        first_messages[0]
            .body
            .as_deref()
            .expect("Expected sender queue message body"),
    )?;

    assert_eq!(first_message_body.tx_id, tx_request_body.tx_id);

    // Process the exact same request again.
    e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(tx_request_event)
        .await
        .expect("standard_tx_signer function_handler failed");

    // The queue should remain empty because the duplicate should be ignored.
    sender_queue.clear_queue().await?;

    let remaining_messages = sender_queue.receive_messages(5).await;
    assert!(
        remaining_messages.is_err(),
        "Expected no additional sender queue message after processing a duplicate request"
    );

    println!("{} PASSED", module_path!());
    Ok(())
}

pub async fn duplicate_tx_id_with_different_body(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!(
        "Verifies that reusing a tx_id with a different request body does not produce an additional sender queue message"
    );

    let sender_queue = &e2e_test_fixture.test_queue_manager.standard_sender_queue;

    // Ensure the queue contains no messages from previous tests.
    sender_queue.clear_queue().await?;

    let original_tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    let mut fake_tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    // Reuse the original tx_id but retain different values for the other fields.
    fake_tx_request_body.tx_id = original_tx_request_body.tx_id.clone();

    let original_tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &original_tx_request_body.to_string(),
        None,
    )])?;

    let fake_tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &fake_tx_request_body.to_string(),
        None,
    )])?;

    // Process the original request.
    e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(original_tx_request_event)
        .await
        .expect("standard_tx_signer function_handler failed");

    let first_queue_event = sender_queue.receive_messages(5).await?;
    let first_messages = first_queue_event.payload.records;

    assert_eq!(
        first_messages.len(),
        1,
        "Expected one sender queue message for the original request"
    );

    let original_message_body: StandardSenderQueueMessageBody = from_str(
        first_messages[0]
            .body
            .as_deref()
            .expect("Expected sender queue message body"),
    )?;

    assert_eq!(original_message_body.tx_id, original_tx_request_body.tx_id);

    // Process a different body with the same tx_id.
    e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(fake_tx_request_event)
        .await
        .expect("standard_tx_signer function_handler failed");

    // No additional message should be produced.
    sender_queue.clear_queue().await?;

    let remaining_messages = sender_queue.receive_messages(5).await;
    assert!(
        remaining_messages.is_err(),
        "Expected no additional sender queue message when reusing a tx_id with a different body"
    );

    println!("{} PASSED", module_path!());
    Ok(())
}
