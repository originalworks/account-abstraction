use db_types::TxStatus;
use e2e_test::aws::s3::BLOB_JSON_TEST_FILES;
use e2e_test::aws::sqs::event::TestEventMessage;
use e2e_test::aws::sqs::event::build_lambda_sqs_event;
use e2e_test::aws::sqs::test_queue::SqsQueueTester;
use e2e_test::fixture::E2eTestFixture;
use e2e_test::tx_request::{BlobTxRequestBodyForTest, BlobTxRequestBodyOptional};
use std::time::Duration;
use tx_request::blob_tx::BlobTxRequestBody;

pub async fn happy_path_two_blob_tx(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    let tx_request_body = BlobTxRequestBody::test_build(BlobTxRequestBodyOptional::default(
        e2e_test_fixture.env_vars.anvil_chain_id,
        BLOB_JSON_TEST_FILES[1].to_string(),
    ))?;

    let tx_request_body_2 = BlobTxRequestBody::test_build(BlobTxRequestBodyOptional::default(
        e2e_test_fixture.env_vars.anvil_chain_id,
        BLOB_JSON_TEST_FILES[2].to_string(),
    ))?;

    let tx_request_event = build_lambda_sqs_event(vec![
        TestEventMessage::new(&tx_request_body.to_string(), None),
        TestEventMessage::new(&tx_request_body_2.to_string(), None),
    ])?;

    blob_tx_signer::aws_lambda::function_handler(tx_request_event, &e2e_test_fixture.pool)
        .await
        .unwrap();

    let blob_tx_input = e2e_test_fixture
        .db_repositories
        .blob_tx_input_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    assert!(blob_tx_input.signature.is_empty() == false);

    let blob_tx_input_2 = e2e_test_fixture
        .db_repositories
        .blob_tx_input_repo
        .find_by_tx_id(&tx_request_body_2.tx_id)
        .await?;

    assert!(blob_tx_input_2.signature.is_empty() == false);
    // Transaction Request was signed and is ready to be sent

    let blob_sender_queue_event = e2e_test_fixture
        .test_queue_manager
        .blob_sender_queue
        .receive_messages(5)
        .await?;

    match blob_tx_sender::aws_lambda::function_handler(
        blob_sender_queue_event,
        &e2e_test_fixture.pool,
    )
    .await
    {
        Ok(_) => {}
        Err(err) => {
            println!("{err:#?}")
        }
    }

    let receipt_poller_queue_event = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    let mut receipt_found = false;

    while receipt_found == false {
        match e2e_test_fixture
            .orchestrators
            .receipt_poller_orchestrator
            .function_handler(receipt_poller_queue_event.clone())
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
            .find_by_tx_id(&blob_tx_input.tx_id)
            .await?;
        if tx_request.tx_status == TxStatus::EXECUTED {
            receipt_found = true;
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
    assert!(receipt_found);

    Ok(())
}
