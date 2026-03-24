mod constants;
mod contract;
mod transaction;
mod wallet_pool;

use std::env;

pub struct Config {
    pub database_url: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let database_url = Self::get_env_var("DATABASE_URL");

        Ok(Self { database_url })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}

#[cfg(feature = "aws")]
pub mod aws_lambda {
    use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
    use lambda_runtime::LambdaEvent;
    use network_db::networks::NetworkRepo;
    use operator_wallet_db::operator_wallets::OperatorWalletRepo;
    use sender_queue::SenderQueueStandardEvent;
    use tx_request_db::tx_requests::TransactionRepo;
    use wallet_assignment_db::wallet_assignments::WalletAssignmentRepo;

    use crate::{
        contract::ContractManager, transaction::TxContextBuilder, wallet_pool::WalletPoolManager,
    };

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<SqsBatchResponse, lambda_runtime::Error> {
        println!("Building...");

        let transaction_assignment_repo = WalletAssignmentRepo::new(&pool);
        let operator_wallet_repo = OperatorWalletRepo::new(&pool);
        let network_repo = NetworkRepo::new(&pool);
        let transaction_repo = TransactionRepo::new(&pool);
        let networks = network_repo.select_all().await?;

        let wallet_pool_manager = WalletPoolManager::build(operator_wallet_repo, &networks);
        let tx_context_builder = TxContextBuilder::build(&transaction_repo);
        let contract_manager = ContractManager::build(&networks)?;

        let mut sqs_batch_response = SqsBatchResponse::default();

        println!("Reading...");
        let tx_sender_queue_event = SenderQueueStandardEvent::from_sqs_event(event)?;

        let tx_ids = tx_sender_queue_event
            .messages
            .iter()
            .map(|message| message.message_id.clone())
            .collect::<Vec<String>>();

        let execute_batch_context_vec = tx_context_builder
            .fetch_and_sort_into_batches(&tx_ids)
            .await?;

        println!("Executing...");
        for execute_batch_context in execute_batch_context_vec {
            let Some(wallet) = wallet_pool_manager
                .acquire(
                    execute_batch_context.chain_id,
                    execute_batch_context.use_operator_wallet_id,
                )
                .await?
            else {
                transaction_repo
                    .release_many(&execute_batch_context.tx_ids)
                    .await?;
                execute_batch_context.tx_ids.iter().for_each(|tx_id| {
                    if let Some(message_id) = tx_sender_queue_event.tx_id_to_message_id.get(tx_id) {
                        sqs_batch_response.add_failure(message_id);
                    };
                });
                continue;
            };

            // next: create transaction_assignment, send transaction, mark txs in DB, release wallet
            // ...
        }

        Ok(sqs_batch_response)
    }
}
