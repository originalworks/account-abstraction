mod blob_tx;
mod standard_tx;

use e2e_test::{aws::config::build_aws_sdk_config, fixture::get_e2e_test_fixture};

use crate::{
    blob_tx::{blob_happy_path::single_blob_tx_e2e, blob_happy_path_two_tx::two_blob_tx_e2e},
    standard_tx::standard_happy_path::single_standard_tx_e2e,
};

#[tokio::test]
async fn e2e_tests() -> anyhow::Result<()> {
    let aws_config: aws_config::SdkConfig = build_aws_sdk_config().await?;
    let e2e_test_fixture = get_e2e_test_fixture(&aws_config).await;
    single_standard_tx_e2e(e2e_test_fixture).await?;
    single_blob_tx_e2e(e2e_test_fixture).await?;
    two_blob_tx_e2e(e2e_test_fixture).await?;
    Ok(())
}
