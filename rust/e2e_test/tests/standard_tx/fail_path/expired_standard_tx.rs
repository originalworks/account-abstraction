use db_types::TxStatus;
use e2e_test::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tx_request::standard::StandardTxRequestBody;

pub async fn expired_standard_tx(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut tx_request_body_optional =
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id);
    tx_request_body_optional.deadline_timestamp =
        Some(i64::try_from(current_timestamp).unwrap() - 3600);

    let mut tx_request_body = StandardTxRequestBody::test_build(tx_request_body_optional)?;

    let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &tx_request_body.to_string(),
        None,
    )])?;

    standard_tx_signer::aws_lambda::function_handler(
        tx_request_event,
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    let standard_tx_input = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    assert!(standard_tx_input.signature.is_empty() == false);

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

    // let receipt_poller_queue_event = e2e_test_fixture
    //     .test_queue_manager
    //     .receipt_poller_queue
    //     .receive_messages(5)
    //     .await?;

    // let mut receipt_found = false;

    // while receipt_found == false {
    //     match receipt_poller::aws_lambda::function_handler(
    //         receipt_poller_queue_event.clone(),
    //         &e2e_test_fixture.pool,
    //     )
    //     .await
    //     {
    //         Ok(_) => {}
    //         Err(err) => {
    //             println!("{err:#?}")
    //         }
    //     }
    //     let tx_request = e2e_test_fixture
    //         .db_repositories
    //         .tx_request_repo
    //         .find_by_tx_id(&standard_tx_input.tx_id)
    //         .await?;
    //     if tx_request.tx_status == TxStatus::EXECUTED {
    //         receipt_found = true;
    //     }
    //     tokio::time::sleep(Duration::from_millis(1000)).await;
    // }
    // assert!(receipt_found);

    // match e2e_test_fixture
    //     .orchestrators
    //     .standard_tx_sender_orchestrator
    //     .function_handler(sender_queue_event)
    //     .await
    // {
    //     Ok(_) => {}
    //     Err(err) => {
    //         println!("{err:#?}")
    //     }
    // }

    Ok(())
}
