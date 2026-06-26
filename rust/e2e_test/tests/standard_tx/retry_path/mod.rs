pub mod retry_path_standard_dropped;
pub mod retry_path_standard_tx_stuck;

use e2e_test::{db::network::AnvilTestNetwork, fixture::E2eTestFixture};

pub async fn set_tx_max_age(
    e2e_test_fixture: &E2eTestFixture,
    tx_max_age_sec: i64,
) -> anyhow::Result<receipt_poller::orchestrator::aws::AwsLambdaOrchestrator> {
    e2e_test_fixture
        .db_repositories
        .network_repo
        .set_tx_max_age(tx_max_age_sec, e2e_test_fixture.env_vars.anvil_chain_id)
        .await?;

    // Can't use receipt_poller from e2e_test_fixture because it has old network data cached
    let receipt_poller = get_receipt_poller(&e2e_test_fixture).await?;
    Ok(receipt_poller)
}

async fn get_receipt_poller(
    e2e_test_fixture: &E2eTestFixture,
) -> anyhow::Result<receipt_poller::orchestrator::aws::AwsLambdaOrchestrator> {
    Ok(
        receipt_poller::orchestrator::aws::AwsLambdaOrchestrator::build(
            &e2e_test_fixture.pool,
            &e2e_test_fixture.aws_config,
        )
        .await?,
    )
}
