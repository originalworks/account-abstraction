mod blob_tx;
mod standard_tx;

use crate::{
    blob_tx::happy_path::{
        happy_path_single_blob_tx::happy_path_single_blob_tx,
        happy_path_two_blob_tx::happy_path_two_blob_tx,
    },
    standard_tx::{
        happy_path::happy_path_single_standard_tx::happy_path_single_standard_tx,
        retry_path::retry_path_standard_tx_dropped::retry_path_standard_tx_dropped,
    },
};
use e2e_test::fixture::get_e2e_test_fixture;

#[tokio::test]
async fn e2e_tests() -> anyhow::Result<()> {
    let e2e_test_fixture = get_e2e_test_fixture().await;
    happy_path_single_standard_tx(e2e_test_fixture).await?;
    // happy_path_single_blob_tx(e2e_test_fixture).await?;
    // happy_path_two_blob_tx(e2e_test_fixture).await?;
    // retry_path_standard_tx_dropped(e2e_test_fixture).await?;
    Ok(())
}
