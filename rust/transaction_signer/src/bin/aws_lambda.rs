#![cfg(feature = "aws")]
use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::{LambdaEvent, run, service_fn, tracing};
use ow_wallet_adapter::{OwWalletConfig, wallet::OwWallet};
use sqlx::postgres::PgPool;
use transaction_db::transactions::TransactionRepo;
use transaction_signer::{Config, calldata::parse_calldata, event::SignTxRequest};

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    println!("Cold start");
    tracing::init_default_subscriber();
    run(service_fn(function_handler)).await
}

pub(crate) async fn function_handler(
    event: LambdaEvent<SqsEvent>,
) -> anyhow::Result<(), lambda_runtime::Error> {
    println!("Building...");

    let config = Config::build()?;
    let wallet_config = OwWalletConfig::from(&config)?;
    let wallet = OwWallet::build(&wallet_config).await?;
    let pool = PgPool::connect(&std::env::var(config.database_url)?).await?;
    let transaction_repo = TransactionRepo::new(&pool);

    println!("Signing...");
    let sign_tx_requests = SignTxRequest::from_sqs_event(event)?;

    for sign_tx_request in sign_tx_requests {
        let calldata = parse_calldata(&sign_tx_request.calldata)?;
        let signature = wallet.sign_message(calldata.as_slice()).await?;

        println!("Saving...");
        let insert_tx_input = sign_tx_request.into_db_transaction(signature.to_string())?;

        transaction_repo
            .insert_ignore_conflict(&insert_tx_input)
            .await?;
    }

    Ok(())
}
