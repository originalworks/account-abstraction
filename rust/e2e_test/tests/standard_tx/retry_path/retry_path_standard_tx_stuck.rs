use alloy::providers::{Provider, ProviderBuilder};
use db_types::TxStatus;
use e2e_test::{
    aws::sqs::{
        event::{TestEventMessage, build_lambda_sqs_event},
        test_queue::SqsQueueTester,
    },
    contract::ContractManagerForTests,
    db::network::AddAnvilNetwork,
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use receipt_poller_queue::ReceiptPollerQueueMessageBody;
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use standard_tx_sender::{contract::ContractManager, transaction::TxContextBuilder};
use std::{env, time::Duration};
use tx_request::standard::StandardTxRequestBody;
use wallet_pool::manager::WalletPoolManager;

pub async fn retry_path_standard_tx_stuck(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
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

    let networks = e2e_test_fixture
        .db_repositories
        .network_repo
        .select_all()
        .await?;
    let provider = ProviderBuilder::new().connect_http(networks[0].rpc_url.parse()?);

    provider
        .raw_request::<_, ()>("evm_setAutomine".into(), [false])
        .await?;

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

    let receipt_poller_queue_event = e2e_test_fixture
        .test_queue_manager
        .receipt_poller_queue
        .receive_messages(5)
        .await?;

    let default_tx_max_age_sec = networks[0].tx_max_age_sec;

    e2e_test_fixture
        .db_repositories
        .network_repo
        .set_tx_max_age(1, networks[0].chain_id)
        .await?;

    tokio::time::sleep(Duration::from_millis(3000)).await;

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
    let tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&standard_tx_input.tx_id)
        .await?;

    assert_eq!(tx_request.tx_status, TxStatus::RETRIED);

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
    e2e_test_fixture
        .db_repositories
        .network_repo
        .set_tx_max_age(default_tx_max_age_sec, networks[0].chain_id)
        .await?;

    Ok(())
}
