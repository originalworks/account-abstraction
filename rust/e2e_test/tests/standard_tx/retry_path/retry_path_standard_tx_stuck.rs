use alloy::providers::{Provider, ProviderBuilder};
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

use crate::standard_tx::retry_path::set_tx_max_age;

pub async fn retry_path_standard_tx_stuck(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    let tx_id = uuid::Uuid::new_v4().to_string();

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

    let networks = e2e_test_fixture
        .db_repositories
        .network_repo
        .select_all()
        .await?;
    let provider = ProviderBuilder::new().connect_http(networks[0].rpc_url.parse()?);

    // Disable automine to simulate a stuck transaction.
    // Transaction will not be mined into a block, and will remain in pool.
    provider
        .raw_request::<_, ()>("evm_setAutomine".into(), [false])
        .await?;

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

    // POLL FOR RECEIPT
    let receipt_poller_queue_event = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    let default_tx_max_age_sec = networks[0].tx_max_age_sec;

    tokio::time::sleep(Duration::from_millis(3000)).await;

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

    // Re-enable automine to allow the retried transaction to be mined into a block.
    provider
        .raw_request::<_, ()>("evm_setAutomine".into(), [true])
        .await?;
    set_tx_max_age(&e2e_test_fixture, default_tx_max_age_sec).await?;

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
