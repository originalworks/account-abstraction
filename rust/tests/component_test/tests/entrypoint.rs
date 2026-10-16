#![recursion_limit = "256"]
pub mod tx_signers;

use crate::tx_signers::standard_tx_signer;
use test_fixture::{db::get_pool, fixture::E2eTestFixture};

#[tokio::test]
async fn standard_tx_signer_component_database_tests() -> anyhow::Result<()> {
    let pool = get_pool().await?;

    let e2e_test_fixture = E2eTestFixture::build(&pool).await?;

    standard_tx_signer::database::idempotency::duplicate_tx_id_with_different_body(
        &e2e_test_fixture,
    )
    .await?;
    standard_tx_signer::database::idempotency::duplicates(&e2e_test_fixture).await?;
    standard_tx_signer::database::successful_sign::successful_sign_single_request(
        &e2e_test_fixture,
    )
    .await?;
    standard_tx_signer::database::successful_sign::successful_sign_multiple_request(
        &e2e_test_fixture,
    )
    .await?;

    standard_tx_signer::database::fail::database_unavailable(&e2e_test_fixture).await?;
    standard_tx_signer::database::fail::malformed_requests_with_valid_request_in_batch(
        &e2e_test_fixture,
    )
    .await?;
    standard_tx_signer::database::fail::malformed_request_structures_in_batch(&e2e_test_fixture)
        .await?;

    Ok(())
}

#[tokio::test]
async fn standard_tx_signer_component_queue_tests() -> anyhow::Result<()> {
    let pool = get_pool().await?;

    let e2e_test_fixture = E2eTestFixture::build(&pool).await?;

    standard_tx_signer::queue::successful_sign::successful_sign_single_request_sends_sender_message(
        &e2e_test_fixture,
    )
    .await?;
    standard_tx_signer::queue::successful_sign::successful_sign_multiple_requests_sends_sender_messages(
        &e2e_test_fixture,
    )
    .await?;

    standard_tx_signer::queue::idempotency::duplicates(&e2e_test_fixture).await?;
    standard_tx_signer::queue::idempotency::duplicate_tx_id_with_different_body(&e2e_test_fixture)
        .await?;

    standard_tx_signer::queue::fail::database_unavailable(&e2e_test_fixture).await?;
    standard_tx_signer::queue::fail::malformed_requests_with_valid_request_in_batch(
        &e2e_test_fixture,
    )
    .await?;
    standard_tx_signer::queue::fail::malformed_request_structures_in_batch(&e2e_test_fixture)
        .await?;
    Ok(())
}
