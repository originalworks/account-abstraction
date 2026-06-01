#![recursion_limit = "256"]
#![cfg(feature = "aws")]
use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};
use lambda_runtime::{run, service_fn, tracing};
use receipt_poller::{Config, orchestrator::aws::AwsLambdaOrchestrator};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();

    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;

    let database_url = Config::get_env_var("DATABASE_URL");
    let pool = PgPool::connect(&database_url).await?;

    let aws_lambda_orchestrator = AwsLambdaOrchestrator::build(&pool, &aws_config).await?;

    run(service_fn(|event| {
        aws_lambda_orchestrator.function_handler(event)
    }))
    .await
}
