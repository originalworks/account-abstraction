#![cfg(feature = "aws")]
use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};
use aws_secrets::AwsSecretsManager;
use lambda_runtime::{LambdaEvent, run, service_fn, tracing};
use migrator::run_migration;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

#[derive(Debug, Deserialize)]
pub struct MigrationEvent {
    pub source: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MigrationResponse {
    pub success: bool,
    pub message: String,
    pub error: Option<String>,
}

async fn function_handler(
    _event: LambdaEvent<MigrationEvent>,
) -> anyhow::Result<MigrationResponse, lambda_runtime::Error> {
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;

    let aws_secrets_manager = AwsSecretsManager::build(&aws_config)?;
    let database_url = aws_secrets_manager.read_database_url().await?;

    // WIP, testing
    println!("Success! Database url is: {}", database_url);
    // let pool = PgPool::connect(&database_url).await?;
    // run_migration(&pool).await?;
    Ok(MigrationResponse {
        success: true,
        message: "All good".to_string(),
        error: None,
    })
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();
    run(service_fn(function_handler)).await
}
