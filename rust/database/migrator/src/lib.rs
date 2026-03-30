use sqlx::migrate::Migrator;

static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

pub async fn run_migration(pool: &sqlx::Pool<sqlx::Postgres>) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}
