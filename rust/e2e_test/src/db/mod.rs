pub mod network;

use sqlx::PgPool;
use std::env;

pub async fn drop_and_migrate(pool: &sqlx::Pool<sqlx::Postgres>) -> anyhow::Result<()> {
    drop_table(&pool).await?;
    migrator::run_migration(&pool).await?;
    Ok(())
}

pub async fn drop_table(pool: &sqlx::Pool<sqlx::Postgres>) -> anyhow::Result<()> {
    sqlx::query!("DROP SCHEMA public CASCADE;")
        .execute(pool)
        .await?;

    sqlx::query!("CREATE SCHEMA public;").execute(pool).await?;
    Ok(())
}

pub async fn get_pool() -> anyhow::Result<sqlx::Pool<sqlx::Postgres>> {
    let database_url = env::var("DATABASE_URL").unwrap();
    let pool: sqlx::Pool<sqlx::Postgres> = PgPool::connect(&database_url).await?;
    Ok(pool)
}
