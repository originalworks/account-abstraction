#![cfg(feature = "aws")]
use lambda_runtime::{run, service_fn, tracing};
use sqlx::PgPool;
use sender::{Config, aws_lambda::function_handler};

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();

    let config = Config::build()?;

    let pool = PgPool::connect(&config.database_url).await?;

    run(service_fn(|event| function_handler(event, &pool))).await
}
