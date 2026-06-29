use db_types::{TxExecutionOutcome, TxStatus};
use e2e_test::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    db::execution_attempt::ExecutionAttemptTestExt,
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use execution_attempt_db::execution_attempts::ExecutionAttemptRepo;
use tx_request::standard::StandardTxRequestBody;
use tx_request_db::repo::TxRequestRepo;
use uuid::Uuid;

pub async fn retry_path_standard_reverted(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    let network = e2e_test_fixture
        .db_repositories
        .network_repo
        .select_all()
        .await?;
    let mut valid_tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;
    let mut invalid_tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;
    valid_tx_request_body.tx_id = uuid::Uuid::new_v4().to_string();
    invalid_tx_request_body.tx_id = uuid::Uuid::new_v4().to_string();
    invalid_tx_request_body.calldata = "aabbccddeeff112233445566778899".to_string(); // Invalid calldata to cause transaction to revert
    invalid_tx_request_body.to_address = network[0].clone().contract_address;

    let tx_request_event = build_lambda_sqs_event(vec![
        TestEventMessage::new(&valid_tx_request_body.to_string(), None),
        TestEventMessage::new(&invalid_tx_request_body.to_string(), None),
    ])?;

    // SIGN
    standard_tx_signer::aws_lambda::function_handler(
        tx_request_event,
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    // SEND
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

    let valid_tx_execution = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_tx_id(&valid_tx_request_body.tx_id)
        .await?
        .pop()
        .unwrap();
    let invalid_tx_execution = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_tx_id(&invalid_tx_request_body.tx_id)
        .await?
        .pop()
        .unwrap();

    // both tx were included in the same batch
    assert_eq!(invalid_tx_execution.id, valid_tx_execution.id);
    assert_eq!(
        invalid_tx_execution.outcome.unwrap(),
        TxExecutionOutcome::REVERTED
    );

    // RETRY
    let retry_queue_event = e2e_test_fixture
        .test_queue_manager
        .retry_queue
        .receive_messages(5)
        .await?;

    match e2e_test_fixture
        .orchestrators
        .retry_handler_orchestrator
        .function_handler(retry_queue_event.clone())
        .await
    {
        Ok(_) => {}
        Err(err) => {
            println!("{err:#?}")
        }
    }

    let new_execution_attempt = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_source_execution_attempt_id(&invalid_tx_execution.id)
        .await?;

    let invalid_tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&invalid_tx_request_body.tx_id)
        .await?;
    let mut valid_tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&valid_tx_request_body.tx_id)
        .await?;

    // original execution was split into two
    assert_eq!(new_execution_attempt.len(), 2);
    assert_eq!(valid_tx_request.tx_status, TxStatus::BROADCASTED);
    assert_eq!(valid_tx_request.attempts, 2);
    assert_eq!(invalid_tx_request.tx_status, TxStatus::FAILED);

    // POLL FOR RECEIPT
    let receipt_poller_queue_event = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    match e2e_test_fixture
        .orchestrators
        .receipt_poller_orchestrator
        .sqs_event_handler(receipt_poller_queue_event.clone().payload)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            println!("{err:#?}")
        }
    }

    valid_tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&valid_tx_request_body.tx_id)
        .await?;
    assert_eq!(valid_tx_request.tx_status, TxStatus::EXECUTED);

    Ok(())
}
