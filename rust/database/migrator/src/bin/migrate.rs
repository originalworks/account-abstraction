use migrator::run_migration;
use sqlx::PgPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = &std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(database_url).await?;
    run_migration(&pool).await?;
    Ok(())
}
