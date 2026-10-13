use test_fixture::{
    aws::sqs::event::{TestEventMessage, build_lambda_sqs_event},
    fixture::E2eTestFixture,
    tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
};
use tx_request::standard::StandardTxRequestBody;

pub async fn duplicates(e2e_test_fixture: &E2eTestFixture) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!("Verifies that processing the same request twice leaves the database state unchanged",);
    let tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &tx_request_body.to_string(),
        None,
    )])?;

    standard_tx_signer::aws_lambda::function_handler(
        tx_request_event.clone(),
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    let first_tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    let first_standard_tx_input = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    standard_tx_signer::aws_lambda::function_handler(
        tx_request_event,
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    let second_tx_request = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    let second_standard_tx_input = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&tx_request_body.tx_id)
        .await?;

    // The existing records must remain unchanged.
    assert_eq!(second_tx_request.sequence_id, first_tx_request.sequence_id);
    assert_eq!(second_tx_request.tx_id, first_tx_request.tx_id);
    assert_eq!(second_tx_request.tx_status, first_tx_request.tx_status);
    assert_eq!(second_tx_request.attempts, first_tx_request.attempts);
    assert_eq!(second_tx_request.created_at, first_tx_request.created_at);
    assert_eq!(second_tx_request.updated_at, first_tx_request.updated_at);

    assert_eq!(
        second_standard_tx_input.tx_id,
        first_standard_tx_input.tx_id
    );
    assert_eq!(
        second_standard_tx_input.signature,
        first_standard_tx_input.signature
    );
    assert_eq!(
        second_standard_tx_input.calldata,
        first_standard_tx_input.calldata
    );
    assert_eq!(
        second_standard_tx_input.to_address,
        first_standard_tx_input.to_address
    );
    assert_eq!(
        second_standard_tx_input.value_wei,
        first_standard_tx_input.value_wei
    );
    assert_eq!(
        second_standard_tx_input.deadline_timestamp,
        first_standard_tx_input.deadline_timestamp
    );
    assert_eq!(
        second_standard_tx_input.pass_value_from_operator_wallet,
        first_standard_tx_input.pass_value_from_operator_wallet
    );
    assert_eq!(
        second_standard_tx_input.created_at,
        first_standard_tx_input.created_at
    );

    println!("{} PASSED", module_path!());
    Ok(())
}

pub async fn duplicate_tx_id_with_different_body(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<()> {
    println!("Entering component test: {}", module_path!());
    println!(
        "Verifies that reusing a tx_id with a different request body does not overwrite existing records",
    );
    let original_tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;

    let mut fake_tx_request_body = StandardTxRequestBody::test_build(
        StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
    )?;
    fake_tx_request_body.tx_id = original_tx_request_body.tx_id.clone();

    let original_tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &original_tx_request_body.to_string(),
        None,
    )])?;

    let fake_tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
        &fake_tx_request_body.to_string(),
        None,
    )])?;

    standard_tx_signer::aws_lambda::function_handler(
        original_tx_request_event.clone(),
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    let tx_request_before = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&original_tx_request_body.tx_id)
        .await?;

    let tx_input_before = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&original_tx_request_body.tx_id)
        .await?;

    standard_tx_signer::aws_lambda::function_handler(
        fake_tx_request_event,
        &e2e_test_fixture.pool,
        &e2e_test_fixture.aws_config,
    )
    .await
    .unwrap();

    let tx_request_after = e2e_test_fixture
        .db_repositories
        .tx_request_repo
        .find_by_tx_id(&original_tx_request_body.tx_id)
        .await?;

    let tx_input_after = e2e_test_fixture
        .db_repositories
        .standard_tx_input_repo
        .find_by_tx_id(&original_tx_request_body.tx_id)
        .await?;

    assert_eq!(tx_request_after.sequence_id, tx_request_before.sequence_id);
    assert_eq!(tx_request_after.tx_id, tx_request_before.tx_id);
    assert_eq!(tx_request_after.tx_status, tx_request_before.tx_status);
    assert_eq!(tx_request_after.attempts, tx_request_before.attempts);
    assert_eq!(tx_request_after.created_at, tx_request_before.created_at);
    assert_eq!(tx_request_after.updated_at, tx_request_before.updated_at);

    assert_eq!(tx_input_after.tx_id, tx_input_before.tx_id);
    assert_eq!(tx_input_after.signature, tx_input_before.signature);
    assert_eq!(tx_input_after.calldata, tx_input_before.calldata);
    assert_eq!(tx_input_after.to_address, tx_input_before.to_address);
    assert_eq!(tx_input_after.value_wei, tx_input_before.value_wei);
    assert_eq!(
        tx_input_after.deadline_timestamp,
        tx_input_before.deadline_timestamp
    );
    assert_eq!(
        tx_input_after.pass_value_from_operator_wallet,
        tx_input_before.pass_value_from_operator_wallet
    );
    assert_eq!(tx_input_after.created_at, tx_input_before.created_at);

    println!("{} PASSED", module_path!());
    Ok(())
}
