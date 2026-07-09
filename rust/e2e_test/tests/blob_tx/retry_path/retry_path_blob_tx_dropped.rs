use db_types::TxStatus;
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
use std::time::Duration;
use tx_request::blob_tx::BlobTxRequestBody;

use crate::common::retry::get_receipt_poller_with_tx_max_age;

const RANDOM_TX_HASH: &str = "0xf92145c95eb1bbda1237ab8dfbe87bb35136e58c9b2133caee84faae8df91b58";

pub async fn retry_path_blob_tx_dropped(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    let tx_id = uuid::Uuid::new_v4().to_string();
    let chain_id = e2e_test_fixture.env_vars.anvil_chain_id;

    let networks = e2e_test_fixture
        .db_repositories
        .network_repo
        .select_all()
        .await?;

    let default_tx_max_age_sec = networks[0].tx_max_age_sec;

    let mut receipt_poller = get_receipt_poller_with_tx_max_age(&e2e_test_fixture, 1).await?;

    let mut tx_request_body = BlobTxRequestBody::test_build(BlobTxRequestBodyOptional::default(
        e2e_test_fixture.env_vars.anvil_chain_id,
        BLOB_JSON_TEST_FILES[1].to_string(),
    ))?;
    tx_request_body.tx_id = tx_id.clone();

    let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &tx_request_body.to_string(),
        None,
    )])?;

    blob_tx_signer::aws_lambda::function_handler(tx_request_event, &e2e_test_fixture.pool)
        .await
        .unwrap();

    // receive message to clear the queue
    e2e_test_fixture
        .test_queue_manager
        .blob_sender_queue
        .receive_messages(1)
        .await?;

    // Simulate sending dropped transaction by saving it to the database without sending it to the network
    let mut blob_batch_context = e2e_test_fixture
        .orchestrators
        .blob_tx_sender_orchestrator
        .tx_context_builder
        .fetch_and_sort_into_batches(&vec![tx_id.clone()])
        .await?
        .pop()
        .unwrap();

    let mut wallet = e2e_test_fixture
        .orchestrators
        .blob_tx_sender_orchestrator
        .wallet_pool_manager
        .acquire(chain_id, None)
        .await?
        .unwrap();

    e2e_test_fixture
        .orchestrators
        .blob_tx_sender_orchestrator
        .wallet_assignment_repo
        .new_assignments(&vec![tx_id.clone()], wallet.db_record.id)
        .await?;

    e2e_test_fixture
        .orchestrators
        .blob_tx_sender_orchestrator
        .contract_manager
        .simulate_send_blob_batch(&mut blob_batch_context, &mut wallet)
        .await?;

    blob_batch_context.tx_hash = Some(RANDOM_TX_HASH.to_string());

    let execution_attempt = e2e_test_fixture
        .orchestrators
        .blob_tx_sender_orchestrator
        .save_successful_execution(&blob_batch_context, &wallet)
        .await?;

    e2e_test_fixture
        .orchestrators
        .blob_tx_sender_orchestrator
        .send_receipt_poller_queue_message(&blob_batch_context, &execution_attempt.id.to_string())
        .await?;

    tokio::time::sleep(Duration::from_millis(3000)).await;

    // POLL FOR RECEIPT
    let receipt_poller_queue_event = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    match receipt_poller
        .sqs_event_handler(receipt_poller_queue_event.clone().payload)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            println!("{err:#?}")
        }
    }

    let mut tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_id)
        .await?;

    assert_eq!(tx_request.tx_status, TxStatus::RETRIED);
    receipt_poller =
        get_receipt_poller_with_tx_max_age(&e2e_test_fixture, default_tx_max_age_sec).await?;

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

    tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_id)
        .await?;

    assert_eq!(tx_request.tx_status, TxStatus::BROADCASTED);

    // POLL FOR RECEIPT AGAIN
    let receipt_poller_queue_event_2 = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    match receipt_poller
        .sqs_event_handler(receipt_poller_queue_event_2.clone().payload)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            println!("{err:#?}")
        }
    }

    tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_id)
        .await?;

    assert_eq!(tx_request.tx_status, TxStatus::EXECUTED);
    assert_eq!(tx_request.attempts, 2);

    let execution_attempts = e2e_test_fixture
        .db_repositories
        .execution_attempt_repo
        .find_by_tx_id(&tx_id)
        .await?;

    assert_eq!(execution_attempts.len(), 2);
    assert_eq!(
        execution_attempts[0].nonce_used,
        execution_attempts[1].nonce_used
    );

    println!("retry_path_blob_tx_dropped PASSED");

    Ok(())
}
