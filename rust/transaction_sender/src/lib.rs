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
    use aws_lambda_events::sqs::SqsEvent;
    use lambda_runtime::LambdaEvent;
    use network_db::networks::NetworkRepo;
    use operator_wallet_db::operator_wallets::OperatorWalletRepo;
    use transaction_assignment_db::transaction_assignments::TransactionAssignmentRepo;
    use transaction_db::transactions::TransactionRepo;
    use transaction_sender_queue::TxSenderQueueMessageBody;

    use crate::{
        contract::ContractManager, transaction::TxContextBuilder, wallet_pool::WalletPoolManager,
    };

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        println!("Building...");

        let transaction_assignment_repo = TransactionAssignmentRepo::new(&pool);
        let operator_wallet_repo = OperatorWalletRepo::new(&pool);
        let network_repo = NetworkRepo::new(&pool);
        let transaction_repo = TransactionRepo::new(&pool);
        let networks = network_repo.select_all().await?;

        let wallet_pool_manager = WalletPoolManager::build(operator_wallet_repo, &networks);
        let tx_context_builder = TxContextBuilder::build(&transaction_repo);
        let contract_manager = ContractManager::build(&networks)?;

        println!("Reading...");
        let queue_messages =
            TxSenderQueueMessageBody::from_sqs_message_vec(&event.payload.records)?;
        let execute_batch_context_vec = tx_context_builder
            .fetch_and_sort_into_batches(queue_messages)
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
                continue;
            };

            // next: create transaction_assignment, send transaction, mark txs in DB, release wallet
            // ...
        }

        Ok(())
    }
}
