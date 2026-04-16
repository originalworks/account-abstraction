#![recursion_limit = "256"]
#![cfg(feature = "aws")]
use lambda_runtime::{run, service_fn, tracing};
use retry_handler::{Config, aws_lambda::function_handler};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();

    let pool = PgPool::connect(&Config::get_env_var("DATABASE_URL")).await?;

    run(service_fn(|event| function_handler(event, &pool))).await
}
