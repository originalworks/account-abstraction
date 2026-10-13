use db_types::{TxStatus, TxType};
use test_fixture::{
    aws::sqs::event::{TestEventMessage, build_lambda_sqs_event},
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use tx_request::standard::StandardTxRequestBody;

pub async fn successful_sign_single_request(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!("Verifies successful signing and correct database persistence for a single request",);
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

    let tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    let standard_tx_input = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    // Verify StandardTxInput was correctly persisted.
    assert!(standard_tx_input.signature.is_empty() == false);
    assert_eq!(standard_tx_input.tx_id, tx_request_body.tx_id);
    assert_eq!(
        standard_tx_input.calldata,
        tx_request_body.calldata.as_bytes()
    );
    assert_eq!(standard_tx_input.to_address, tx_request_body.to_address);
    assert_eq!(standard_tx_input.value_wei, tx_request_body.value_wei);
    assert_eq!(
        standard_tx_input.deadline_timestamp,
        tx_request_body.deadline_timestamp
    );
    assert_eq!(
        standard_tx_input.pass_value_from_operator_wallet,
        tx_request_body.pass_value_from_operator_wallet
    );

    // Verify TxRequest was correctly persisted.
    assert_eq!(tx_request.tx_id, tx_request_body.tx_id);
    assert_eq!(tx_request.requester_id, tx_request_body.requester_id);
    assert_eq!(tx_request.tx_type, TxType::STANDARD);
    assert_eq!(tx_request.tx_status, TxStatus::SIGNED);
    assert_eq!(tx_request.chain_id, tx_request_body.chain_id);
    assert_eq!(
        tx_request.use_operator_wallet_id,
        tx_request_body.use_operator_wallet_id
    );
    assert_eq!(tx_request.attempts, 0);
    assert_eq!(tx_request.metadata, None);

    // Verify database-generated fields.
    assert!(tx_request.sequence_id > 0);

    println!("{} PASSED", module_path!());
    Ok(())
}

pub async fn successful_sign_multiple_request(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!(
        "Verifies successful processing and correct database persistence for multiple requests",
    );
    let tx_request_bodies: Vec<StandardTxRequestBody> = (0..3)
        .map(|_| {
            StandardTxRequestBody::test_build(StandardTxRequestBodyOptional::default(
                e2e_test_fixture.env_vars.anvil_chain_id,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let messages = tx_request_bodies
        .iter()
        .map(|body| TestEventMessage::new(&body.to_string(), None))
        .collect();

    let tx_request_event = build_lambda_sqs_event(messages)?;

    standard_tx_signer::aws_lambda::function_handler(
        tx_request_event,
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    for body in &tx_request_bodies {
        let tx_request = e2e_test_fixture
            .db_repositories
            .tx_request_repo
            .find_by_tx_id(&body.tx_id)
            .await?;

        let tx_input = e2e_test_fixture
            .db_repositories
            .standard_tx_input_repo
            .find_by_tx_id(&body.tx_id)
            .await?;

        assert_eq!(tx_request.tx_id, body.tx_id);
        assert_eq!(tx_request.tx_type, TxType::STANDARD);
        assert_eq!(tx_request.tx_status, TxStatus::SIGNED);
        assert_eq!(tx_request.chain_id, body.chain_id);
        assert_eq!(tx_request.attempts, 0);
        assert!(tx_request.sequence_id > 0);

        assert_eq!(tx_input.tx_id, body.tx_id);
        assert!(!tx_input.signature.is_empty());
        assert_eq!(tx_input.calldata.as_slice(), body.calldata.as_bytes());
        assert_eq!(tx_input.to_address, body.to_address);
        assert_eq!(tx_input.value_wei, body.value_wei);
        assert_eq!(tx_input.deadline_timestamp, body.deadline_timestamp);
        assert_eq!(
            tx_input.pass_value_from_operator_wallet,
            body.pass_value_from_operator_wallet
        );
    }

    println!("{} PASSED", module_path!());
    Ok(())
}
