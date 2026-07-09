use db_types::{TxExecutionOutcome, TxStatus};
use e2e_test::{
    aws::{
        s3::BLOB_JSON_TEST_FILES,
        sqs::{
            event::{TestEventMessage, build_lambda_sqs_event},
            test_queue::SqsQueueTester,
        },
    },
    db::execution_attempt::ExecutionAttemptTestExt,
    fixture::E2eTestFixture,
    tx_request::{BlobTxRequestBodyForTest, BlobTxRequestBodyOptional},
};
use std::time::{SystemTime, UNIX_EPOCH};
use tx_request::blob_tx::BlobTxRequestBody;

pub async fn expired_blob_tx(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut tx_request_body_optional = BlobTxRequestBodyOptional::default(
        e2e_test_fixture.env_vars.anvil_chain_id,
        BLOB_JSON_TEST_FILES.first().unwrap().to_string(),
    );
    tx_request_body_optional.deadline_timestamp =
        Some(i64::try_from(current_timestamp).unwrap() - 3600);

    let tx_request_body = BlobTxRequestBody::test_build(tx_request_body_optional)?;

    let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &tx_request_body.to_string(),
        None,
    )])?;

    blob_tx_signer::aws_lambda::function_handler(
        tx_request_event,
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    let blob_tx_input = e2e_test_fixture
        .db_repositories
        .blob_tx_input_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    assert!(blob_tx_input.signature.is_empty() == false);

    // Transaction Request was signed and is ready to be sent

    let sender_queue_event = e2e_test_fixture
        .test_queue_manager
        .blob_sender_queue
        .receive_messages(5)
        .await?;

    match e2e_test_fixture
        .orchestrators
        .blob_tx_sender_orchestrator
        .function_handler(sender_queue_event)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            println!("{err:#?}")
        }
    }

    let tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;
    let execution_attempt_vec = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_tx_id(&tx_request.tx_id)
        .await?;

    let execution_attempt = execution_attempt_vec.first().unwrap();

    assert_eq!(tx_request.tx_status, TxStatus::FAILED);
    assert_eq!(
        execution_attempt.outcome,
        Some(TxExecutionOutcome::REVERTED)
    );
    assert_eq!(execution_attempt.retryable, Some(false));

    Ok(())
}
