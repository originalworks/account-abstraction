use db_types::TxStatus;
use e2e_test::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use std::time::Duration;
use tx_request::standard::StandardTxRequestBody;

pub async fn happy_path_single_standard_tx(
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

    tokio::time::sleep(Duration::from_secs(1)).await;
    let receipt_poller_queue_event = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    e2e_test_fixture
        .poll_for_receipt_with_scheduler(&standard_tx_input.tx_id, &receipt_poller_queue_event)
        .await?;

    assert_eq!(
        e2e_test_fixture
            .get_tx_status_by_id(&standard_tx_input.tx_id)
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
