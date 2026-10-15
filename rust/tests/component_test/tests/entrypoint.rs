#![recursion_limit = "256"]
pub mod tx_signers;

use crate::tx_signers::standard_tx_signer;
use test_fixture::{db::get_pool, fixture::E2eTestFixture};

#[tokio::test]
async fn e2e_blob_tx_tests() -> anyhow::Result<()> {
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

    Ok(())
}
