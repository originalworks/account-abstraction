mod blob_tx;
mod standard_tx;

use crate::{
    blob_tx::{
        fail_path::expired_blob_tx::expired_blob_tx,
        happy_path::{
            happy_path_single_blob_tx::happy_path_single_blob_tx,
            happy_path_two_blob_tx::happy_path_two_blob_tx,
        },
        idempotency::{
            concurrent_blob_tx_workers::concurrent_blob_tx_workers,
            concurrent_retry_workers_in_blob_tx::concurrent_retry_workers_in_blob_tx,
        },
        retry_path::{
            retry_path_blob_tx_dropped::retry_path_blob_tx_dropped,
            retry_path_blob_tx_stuck::retry_path_blob_tx_stuck,
        },
    },
    standard_tx::{
        fail_path::expired_standard_tx::expired_standard_tx,
        happy_path::{
            happy_path_single_standard_tx::happy_path_single_standard_tx,
            happy_path_two_standard_tx::happy_path_two_standard_tx,
        },
        idempotency::{
            concurrent_standard_tx_retry_workers::concurrent_standard_tx_retry_workers,
            concurrent_standard_tx_workers::concurrent_standard_tx_workers,
        },
        retry_path::{
            retry_path_standard_dropped::retry_path_standard_dropped,
            retry_path_standard_reverted::retry_path_standard_reverted,
            retry_path_standard_tx_stuck::retry_path_standard_tx_stuck,
        },
    },
};
use e2e_test::{db::get_pool, fixture::E2eTestFixture};
// use e2e_test::{db::get_pool, fixture::get_e2e_test_fixture};

#[tokio::test]
async fn e2e_blob_tx_tests() -> anyhow::Result<()> {
    let pool = get_pool().await?;

    let e2e_test_fixture = E2eTestFixture::build(&pool).await?;

    // //
    // // BLOB TXS
    expired_blob_tx(&e2e_test_fixture).await?;

    retry_path_blob_tx_dropped(&e2e_test_fixture).await?;
    retry_path_blob_tx_stuck(&e2e_test_fixture).await?;

    happy_path_single_blob_tx(&e2e_test_fixture).await?;
    happy_path_two_blob_tx(&e2e_test_fixture).await?;

    concurrent_blob_tx_workers(&e2e_test_fixture).await?;
    concurrent_retry_workers_in_blob_tx(&e2e_test_fixture).await?;

    Ok(())
}

#[tokio::test]
async fn e2e_standard_tx_tests() -> anyhow::Result<()> {
    let pool = get_pool().await?;

    // let e2e_test_fixture = get_e2e_test_fixture(pool).await;
    let e2e_test_fixture = E2eTestFixture::build(&pool).await?;
    //
    // STANDARD TXS
    expired_standard_tx(&e2e_test_fixture).await?;

    happy_path_single_standard_tx(&e2e_test_fixture).await?;
    happy_path_single_standard_tx(&e2e_test_fixture).await?;
    happy_path_two_standard_tx(&e2e_test_fixture).await?;

    concurrent_standard_tx_workers(&e2e_test_fixture).await?;
    concurrent_standard_tx_retry_workers(&e2e_test_fixture).await?;

    retry_path_standard_tx_stuck(&e2e_test_fixture).await?;
    retry_path_standard_dropped(&e2e_test_fixture).await?;
    retry_path_standard_reverted(&e2e_test_fixture).await?;

    Ok(())
}
