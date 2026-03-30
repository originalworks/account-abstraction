#![cfg(feature = "aws")]
use lambda_runtime::{LambdaEvent, run, service_fn, tracing};
use migrator::run_migration;
use serde_json::Value;
use sqlx::PgPool;

async fn function_handler(_event: LambdaEvent<Value>) -> anyhow::Result<(), lambda_runtime::Error> {
    let database_url = &std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(database_url).await?;
    run_migration(&pool).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();
    run(service_fn(function_handler)).await
}
