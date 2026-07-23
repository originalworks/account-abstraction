use crate::{db::network::AnvilTestNetwork, fixture::E2eTestFixture};
use aws_lambda_events::sqs::SqsEvent;
use db_types::TxStatus;
use std::time::Duration;

const MAX_SCHEDULER_RUNS: u8 = 15;
const SCHEDULER_INTERVAL_SEC: u64 = 1;

// pub async fn get_receipt_poller_with_tx_max_age(
//     e2e_test_fixture: &E2eTestFixture,
//     tx_max_age_sec: i64,
// ) -> anyhow::Result<receipt_poller::orchestrator::aws::AwsLambdaOrchestrator> {
//     e2e_test_fixture
//         .db_repositories
//         .network_repo
//         .set_tx_max_age(tx_max_age_sec, e2e_test_fixture.env_vars.anvil_chain_id)
//         .await?;

//     // Can't use receipt_poller from e2e_test_fixture because it has old network data cached
//     let receipt_poller = get_fresh_receipt_poller(&e2e_test_fixture).await?;
//     Ok(receipt_poller)
// }

// pub async fn get_fresh_receipt_poller(
//     e2e_test_fixture: &E2eTestFixture,
// ) -> anyhow::Result<receipt_poller::orchestrator::aws::AwsLambdaOrchestrator> {
//     Ok(
//         receipt_poller::orchestrator::aws::AwsLambdaOrchestrator::build(
//             &e2e_test_fixture.pool,
//             &e2e_test_fixture.aws_config,
//         )
//         .await?,
//     )
// }

// pub async fn run_receipt_poller_scheduler(
//     e2e_test_fixture: &E2eTestFixture,
//     wait_for_tx_id: &String,
// ) -> anyhow::Result<()> {
//     for _ in 0..MAX_SCHEDULER_RUNS {
//         match e2e_test_fixture
//             .orchestrators
//             .receipt_poller_orchestrator
//             .scheduler_event_handler()
//             .await
//         {
//             Ok(_) => {}
//             Err(err) => {
//                 println!("{err:#?}")
//             }
//         }

//         if e2e_test_fixture
//             .get_tx_status_by_id(&wait_for_tx_id)
//             .await?
//             == TxStatus::EXECUTED
//         {
//             return Ok(());
//         }
//         tokio::time::sleep(Duration::from_secs(SCHEDULER_INTERVAL_SEC)).await;
//     }

//     Ok(())
// }
