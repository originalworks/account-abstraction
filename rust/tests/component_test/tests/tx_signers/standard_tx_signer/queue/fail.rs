use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use test_fixture::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use tx_request::standard::StandardTxRequestBody;

pub async fn database_unavailable(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!("Verifies that database unavailability does not produce sender queue messages");

    let sender_queue = &e2e_test_fixture.test_queue_manager.standard_sender_queue;

    sender_queue.clear_queue().await?;

    let unavailable_pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy("postgres://postgres:postgres@127.0.0.1:1/standard_tx_signer_test")?;

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        standard_tx_signer::orchestrator::aws::AwsLambdaOrchestrator::build(
            &unavailable_pool,
            &e2e_test_fixture.aws_config,
        ),
    )
    .await;

    let build_result = result
        .map_err(|_| anyhow::anyhow!("Orchestrator build timed out with database unavailable"))?;

    assert!(
        build_result.is_err(),
        "Expected orchestrator construction to fail when the database is unavailable"
    );

    unavailable_pool.close().await;

    // No sender message should have been produced.
    assert!(
        sender_queue.receive_messages(5).await.is_err(),
        "Expected no sender queue messages when the database is unavailable"
    );

    println!("{} PASSED", module_path!());
    Ok(())
}

pub async fn malformed_requests_with_valid_request_in_batch(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!("Verifies that only valid requests produce sender queue messages");

    let sender_queue = &e2e_test_fixture.test_queue_manager.standard_sender_queue;

    sender_queue.clear_queue().await?;

    // Message 1: unsupported chain ID.
    let mut unsupported_chain_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;
    unsupported_chain_body.chain_id = 123456;

    // Message 2: invalid calldata.
    let mut invalid_calldata_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;
    invalid_calldata_body.calldata = "not-hex".to_string();

    // Message 3: valid request.
    let valid_body = StandardTxRequestBody::test_build(StandardTxRequestBodyOptional::default(
        e2e_test_fixture.env_vars.anvil_chain_id,
    ))?;

    let event = build_lambda_sqs_event(vec![
        TestEventMessage::new(&unsupported_chain_body.to_string(), None),
        TestEventMessage::new(&invalid_calldata_body.to_string(), None),
        TestEventMessage::new(&valid_body.to_string(), None),
    ])?;

    e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(event)
        .await
        .expect("standard_tx_signer function_handler failed");

    let sender_queue_event = sender_queue.receive_messages(5).await?;
    let records = sender_queue_event.payload.records;

    assert_eq!(
        records.len(),
        1,
        "Expected exactly one sender message for the valid request"
    );

    let message_body: serde_json::Value = serde_json::from_str(
        records[0]
            .body
            .as_deref()
            .expect("Expected sender message body"),
    )?;

    assert_eq!(
        message_body["tx_id"].as_str(),
        Some(valid_body.tx_id.as_str()),
        "The sender message should contain the valid request's tx_id"
    );

    println!("{} PASSED", module_path!());
    Ok(())
}

pub async fn malformed_request_structures_in_batch(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!(
        "Verifies that a batch containing only structurally malformed requests produces no sender messages"
    );

    let sender_queue = &e2e_test_fixture.test_queue_manager.standard_sender_queue;

    sender_queue.clear_queue().await?;

    let default_options =
        || StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id);

    // Message 1: missing calldata.
    let body_missing_calldata = StandardTxRequestBody::test_build(default_options())?;

    let mut json_missing_calldata: serde_json::Value =
        serde_json::from_str(&body_missing_calldata.to_string())?;

    json_missing_calldata
        .as_object_mut()
        .expect("Expected a JSON object")
        .remove("calldata");

    // Message 2: missing tx_id.
    let body_missing_tx_id = StandardTxRequestBody::test_build(default_options())?;

    let mut json_missing_tx_id: serde_json::Value =
        serde_json::from_str(&body_missing_tx_id.to_string())?;

    json_missing_tx_id
        .as_object_mut()
        .expect("Expected a JSON object")
        .remove("tx_id");

    // Message 3: wrong type for chain_id.
    let body_wrong_chain_id = StandardTxRequestBody::test_build(default_options())?;

    let mut json_wrong_chain_id: serde_json::Value =
        serde_json::from_str(&body_wrong_chain_id.to_string())?;

    json_wrong_chain_id
        .as_object_mut()
        .expect("Expected a JSON object")
        .insert(
            "chain_id".to_string(),
            serde_json::Value::String("not-a-number".to_string()),
        );

    let event = build_lambda_sqs_event(vec![
        TestEventMessage::new(&json_missing_calldata.to_string(), None),
        TestEventMessage::new(&json_missing_tx_id.to_string(), None),
        TestEventMessage::new(&json_wrong_chain_id.to_string(), None),
    ])?;

    let result = e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(event)
        .await;

    assert!(
        result.is_ok(),
        "Expected the handler to complete when all messages are malformed"
    );

    // The queue should contain no sender messages.
    assert!(
        sender_queue.receive_messages(5).await.is_err(),
        "Expected no sender messages when every request is structurally malformed"
    );

    println!("{} PASSED", module_path!());
    Ok(())
}
