#![cfg(feature = "aws")]
use lambda_runtime::{run, service_fn, tracing};
use sqlx::PgPool;
use transaction_signer::{Config, aws_lambda::function_handler};

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();

    let pool = PgPool::connect(&Config::get_env_var("DATABASE_URL")).await?;

    run(service_fn(|event| function_handler(event, &pool))).await
}
