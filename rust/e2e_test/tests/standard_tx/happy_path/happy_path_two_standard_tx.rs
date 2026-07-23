use db_types::TxStatus;
use e2e_test::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    db::execution_attempt::ExecutionAttemptTestExt,
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use std::time::Duration;
use tx_request::standard::StandardTxRequestBody;

pub async fn happy_path_two_standard_tx(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    println!("Entering test: {}", module_path!());
    let tx_request_body_1 = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    let tx_request_body_2 = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    let tx_request_event = build_lambda_sqs_event(vec![
        TestEventMessage::new(&tx_request_body_1.to_string(), None),
        TestEventMessage::new(&tx_request_body_2.to_string(), None),
    ])?;

    standard_tx_signer::aws_lambda::function_handler(
        tx_request_event,
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    // Transaction Request was signed and is ready to be sent
    let sender_queue_event = e2e_test_fixture
        .test_queue_manager
        .standard_sender_queue
        .receive_messages(5)
        .await?;

    match e2e_test_fixture
        .orchestrators
        .standard_tx_sender_orchestrator
        .function_handler(sender_queue_event)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            println!("{err:#?}")
        }
    }

    let tx_1_execution = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_tx_id(&tx_request_body_1.tx_id)
        .await?
        .pop()
        .unwrap();

    let tx_2_execution = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_tx_id(&tx_request_body_2.tx_id)
        .await?
        .pop()
        .unwrap();

    // ensure both tx were included in one batch
    assert_eq!(tx_1_execution.id, tx_2_execution.id);

    assert_eq!(
        e2e_test_fixture
            .get_tx_status_by_id(&tx_request_body_1.tx_id)
            .await?,
        TxStatus::BROADCASTED
    );
    assert_eq!(
        e2e_test_fixture
            .get_tx_status_by_id(&tx_request_body_2.tx_id)
            .await?,
        TxStatus::BROADCASTED
    );

    // Both tx requests were signed and broadcasted in the same execution attempt
    // Now poll for the receipt
    tokio::time::sleep(Duration::from_secs(1)).await;
    let receipt_poller_queue_event = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    e2e_test_fixture
        .poll_for_receipt_with_scheduler(&tx_request_body_1.tx_id, &receipt_poller_queue_event)
        .await?;

    assert_eq!(
        e2e_test_fixture
            .get_tx_status_by_id(&tx_request_body_1.tx_id)
            .await?,
        TxStatus::EXECUTED
    );

    assert_eq!(
        e2e_test_fixture
            .get_tx_status_by_id(&tx_request_body_2.tx_id)
            .await?,
        TxStatus::EXECUTED
    );

    let _outcome_queue_event = e2e_test_fixture
        .test_queue_manager
        .tx_outcome_queue
        .receive_messages(1)
        .await?;
    println!("{} PASSED", module_path!());
    Ok(())
}
