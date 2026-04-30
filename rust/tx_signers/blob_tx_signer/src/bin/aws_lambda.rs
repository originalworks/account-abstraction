#![recursion_limit = "256"]
#![cfg(feature = "aws")]
use std::env;

use blob_tx_signer::aws_lambda::function_handler;
use lambda_runtime::{run, service_fn, tracing};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();
    let database_url = env::var("DATABASE_URL").unwrap();

    let pool = PgPool::connect(database_url.as_str()).await?;

    run(service_fn(|event| function_handler(event, &pool))).await
}
