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
use std::time::Duration;
use tokio::join;
use tx_request::standard::StandardTxRequestBody;

pub async fn concurrent_standard_tx_workers(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering test: {}", module_path!());
    let tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &tx_request_body.to_string(),
        None,
    )])?;

    // SIGN
    join!(
        standard_tx_signer::aws_lambda::function_handler(
            tx_request_event.clone(),
            &e2e_test_fixture.pool,
            &e2e_test_fixture.aws_config,
        ),
        standard_tx_signer::aws_lambda::function_handler(
            tx_request_event.clone(),
            &e2e_test_fixture.pool,
            &e2e_test_fixture.aws_config,
        ),
        standard_tx_signer::aws_lambda::function_handler(
            tx_request_event.clone(),
            &e2e_test_fixture.pool,
            &e2e_test_fixture.aws_config,
        )
    );

    let mut tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    assert_eq!(tx_request.tx_status, TxStatus::SIGNED);
    assert_eq!(tx_request.created_at, tx_request.updated_at);

    let sender_queue_event = e2e_test_fixture
        .test_queue_manager
        .standard_sender_queue
        .receive_messages(5)
        .await?;

    // SEND
    join!(
        e2e_test_fixture
            .orchestrators
            .standard_tx_sender_orchestrator
            .function_handler(sender_queue_event.clone()),
        e2e_test_fixture
            .orchestrators
            .standard_tx_sender_orchestrator
            .function_handler(sender_queue_event.clone()),
        e2e_test_fixture
            .orchestrators
            .standard_tx_sender_orchestrator
            .function_handler(sender_queue_event.clone())
    );

    tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    let mut execution_attempt = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    assert_eq!(tx_request.tx_status, TxStatus::BROADCASTED);
    assert_eq!(tx_request.attempts, 1);
    assert_eq!(execution_attempt.len(), 1);

    tokio::time::sleep(Duration::from_secs(3)).await;
    let receipt_poller_queue_event = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    // POLL FOR RECEIPT
    join!(
        e2e_test_fixture
            .orchestrators
            .receipt_poller_orchestrator
            .sqs_event_handler(receipt_poller_queue_event.clone().payload),
        e2e_test_fixture
            .orchestrators
            .receipt_poller_orchestrator
            .sqs_event_handler(receipt_poller_queue_event.clone().payload),
        e2e_test_fixture
            .orchestrators
            .receipt_poller_orchestrator
            .sqs_event_handler(receipt_poller_queue_event.clone().payload)
    );

    join!(
        e2e_test_fixture
            .orchestrators
            .receipt_poller_orchestrator
            .scheduler_event_handler(),
        e2e_test_fixture
            .orchestrators
            .receipt_poller_orchestrator
            .scheduler_event_handler(),
        e2e_test_fixture
            .orchestrators
            .receipt_poller_orchestrator
            .scheduler_event_handler()
    );

    tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    execution_attempt = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    assert_eq!(tx_request.tx_status, TxStatus::EXECUTED);
    assert_eq!(tx_request.attempts, 1);
    assert_eq!(execution_attempt.len(), 1);
    assert_eq!(
        execution_attempt.first().unwrap().outcome.clone().unwrap(),
        TxExecutionOutcome::SUCCEED
    );
    println!("{} PASSED", module_path!());
    Ok(())
}
