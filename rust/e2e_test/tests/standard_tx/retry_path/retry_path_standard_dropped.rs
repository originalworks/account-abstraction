use std::time::Duration;

use crate::standard_tx::retry_path::set_tx_max_age;
use db_types::TxStatus;
use e2e_test::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    db::execution_attempt::FindExecutionByTxId,
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use tx_request::standard::StandardTxRequestBody;

const RANDOM_TX_HASH: &str = "0x27ee575f57248220b3ae9c190b93de171ceec766850fbcf79f8d6db77f13f752";

pub async fn retry_path_standard_dropped(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    let tx_id = uuid::Uuid::new_v4().to_string();
    let chain_id = e2e_test_fixture.env_vars.anvil_chain_id;

    let networks = e2e_test_fixture
        .db_repositories
        .network_repo
        .select_all()
        .await?;

    let default_tx_max_age_sec = networks[0].tx_max_age_sec;

    let receipt_poller = set_tx_max_age(&e2e_test_fixture, 1).await?;

    let mut tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    tx_request_body.tx_id = tx_id.clone();

    let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &tx_request_body.to_string(),
        None,
    )])?;

    // SIGN
    standard_tx_signer::aws_lambda::function_handler(
        tx_request_event,
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    // Simulate sending dropped transaction by saving it to the database without sending it to the network
    let mut execute_batch_context = e2e_test_fixture
        .orchestrators
        .standard_tx_sender_orchestrator
        .tx_context_builder
        .fetch_and_sort_into_batches(&vec![tx_id.clone()])
        .await?
        .pop()
        .unwrap();

    let mut wallet = e2e_test_fixture
        .orchestrators
        .standard_tx_sender_orchestrator
        .wallet_pool_manager
        .acquire(chain_id, None)
        .await?
        .unwrap();

    e2e_test_fixture
        .orchestrators
        .standard_tx_sender_orchestrator
        .wallet_assignment_repo
        .new_assignments(&vec![tx_id.clone()], wallet.db_record.id)
        .await?;

    e2e_test_fixture
        .orchestrators
        .standard_tx_sender_orchestrator
        .contract_manager
        .simulate_send_batch_tx(&mut execute_batch_context, &mut wallet)
        .await?;

    execute_batch_context.tx_hash = Some(RANDOM_TX_HASH.to_string());

    let execution_attempt = e2e_test_fixture
        .orchestrators
        .standard_tx_sender_orchestrator
        .save_successful_execution(&execute_batch_context, &wallet)
        .await?;

    e2e_test_fixture
        .orchestrators
        .standard_tx_sender_orchestrator
        .send_receipt_poller_queue_message(
            &execute_batch_context,
            &execution_attempt.id.to_string(),
        )
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

    set_tx_max_age(&e2e_test_fixture, default_tx_max_age_sec).await?;

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

    Ok(())
}
