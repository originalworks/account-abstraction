#![cfg(feature = "aws")]
use lambda_runtime::{LambdaEvent, run, service_fn, tracing};
use migrator::run_migration;
use serde::{Deserialize, Serialize};
use std::env;
// use serde_json::Value;
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
    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPool::connect(&database_url).await?;
    match run_migration(&pool).await {
        Ok(_) => {
            return Ok(MigrationResponse {
                success: true,
                message: "Migration completed".to_string(),
                error: None,
            });
        }
        Err(err) => {
            tracing::error!(%err);
            return Ok(MigrationResponse {
                success: false,
                message: "Migration failed".to_string(),
                error: Some(err.to_string()),
            });
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();
    run(service_fn(function_handler)).await
}
