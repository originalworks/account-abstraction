use sqlx::migrate::Migrator;
use sqlx::postgres::PgPool;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    MIGRATOR.run(&pool).await?;
    Ok(())
}
