#![cfg(feature = "aws")]
use lambda_runtime::{LambdaEvent, run, service_fn, tracing};
use serde_json::Value;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPool;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

async fn function_handler(_event: LambdaEvent<Value>) -> anyhow::Result<(), lambda_runtime::Error> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    MIGRATOR.run(&pool).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();
    run(service_fn(function_handler)).await
}
