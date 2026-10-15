use db_types::{TxStatus, TxType};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use test_fixture::{
    aws::sqs::event::{TestEventMessage, build_lambda_sqs_event},
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use tx_request::standard::StandardTxRequestBody;

pub async fn database_unavailable(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!("Verifies that signing fails gracefully when the database is unavailable");

    let tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    // Unavailable PostgreSQL endpoint
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

    // The handler must not hang indefinitely.
    let handler_result =
        result.map_err(|_| anyhow::anyhow!("Handler timed out with database unavailable"))?;

    // The handler must report failure instead of success.
    assert!(
        handler_result.is_err(),
        "Expected the handler to return an error when the database is unavailable"
    );

    unavailable_pool.close().await;

    println!("{} PASSED", module_path!());
    Ok(())
}

pub async fn malformed_requests_with_valid_request_in_batch(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!(
        "Cerifies that malformed requests do not prevent a valid request later in the same SQS batch from being processed"
    );

    // Message 1: unsupported chain ID.
    let mut unsupported_chain_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;
    unsupported_chain_body.chain_id = 123456;

    // Message 2: malformed calldata.
    let mut invalid_calldata_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;
    invalid_calldata_body.calldata = "not-hex".to_string();

    // Message 3: valid request, deliberately last in the batch.
    let valid_body = StandardTxRequestBody::test_build(StandardTxRequestBodyOptional::default(
        e2e_test_fixture.env_vars.anvil_chain_id,
    ))?;

    let event = build_lambda_sqs_event(vec![
        TestEventMessage::new(&unsupported_chain_body.to_string(), None),
        TestEventMessage::new(&invalid_calldata_body.to_string(), None),
        TestEventMessage::new(&valid_body.to_string(), None),
    ])?;

    let result = e2e_test_fixture
        .orchestrators
        .standard_tx_signer_orchestrator
        .function_handler(event)
        .await;

    assert!(
        result.is_ok(),
        "Expected the handler to complete batch processing"
    );

    // Invalid requests must not be persisted.
    let unsupported_chain_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&unsupported_chain_body.tx_id)
        .await;
    assert!(
        unsupported_chain_request.is_err(),
        "Request with unsupported chain ID should not be persisted"
    );

    let unsupported_chain_input = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&unsupported_chain_body.tx_id)
        .await;
    assert!(
        unsupported_chain_input.is_err(),
        "Transaction input with unsupported chain ID should not be persisted"
    );

    let invalid_calldata_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&invalid_calldata_body.tx_id)
        .await;
    assert!(
        invalid_calldata_request.is_err(),
        "Request with malformed calldata should not be persisted"
    );

    let invalid_calldata_input = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&invalid_calldata_body.tx_id)
        .await;
    assert!(
        invalid_calldata_input.is_err(),
        "Transaction input with malformed calldata should not be persisted"
    );

    // The valid request must still be processed, despite the earlier invalid messages.
    let valid_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&valid_body.tx_id)
        .await?;
    assert_eq!(valid_request.tx_id, valid_body.tx_id);
    assert_eq!(valid_request.tx_type, TxType::STANDARD);
    assert_eq!(valid_request.tx_status, TxStatus::SIGNED);
    assert_eq!(valid_request.chain_id, valid_body.chain_id);
    assert_eq!(valid_request.attempts, 0);
    assert!(valid_request.sequence_id > 0);

    let valid_input = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&valid_body.tx_id)
        .await?;
    assert_eq!(valid_input.tx_id, valid_body.tx_id);
    assert!(!valid_input.signature.is_empty());
    assert_eq!(
        valid_input.calldata.as_slice(),
        valid_body.calldata.as_bytes()
    );
    assert_eq!(valid_input.to_address, valid_body.to_address);
    assert_eq!(valid_input.value_wei, valid_body.value_wei);
    assert_eq!(
        valid_input.deadline_timestamp,
        valid_body.deadline_timestamp
    );
    assert_eq!(
        valid_input.pass_value_from_operator_wallet,
        valid_body.pass_value_from_operator_wallet
    );

    println!("{} PASSED", module_path!());
    Ok(())
}
