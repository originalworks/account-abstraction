use transaction_db::run_migration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = &std::env::var("DATABASE_URL")?;
    run_migration(database_url).await?;
    Ok(())
}
