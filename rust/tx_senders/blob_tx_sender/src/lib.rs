#![recursion_limit = "256"]
mod contract;
mod transaction;
use std::env;

pub struct Config {
    pub database_url: String,
    pub receipt_poller_queue_url: String,
    pub receipt_poller_queue_message_group_id: String,
    pub blob_storage_bucket_name: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let database_url = Self::get_env_var("DATABASE_URL");
        let receipt_poller_queue_message_group_id =
            Self::get_env_var("RECEIPT_POLLER_QUEUE_MESSAGE_GROUP_ID");
        let receipt_poller_queue_url = Self::get_env_var("RECEIPT_POLLER_QUEUE_URL");
        let blob_storage_bucket_name = Self::get_env_var("BLOB_STORAGE_BUCKET_NAME");

        Ok(Self {
            database_url,
            receipt_poller_queue_message_group_id,
            receipt_poller_queue_url,
            blob_storage_bucket_name,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}

#[cfg(feature = "aws")]
pub mod aws_lambda {
    use crate::{Config, contract::ContractManager, transaction::BlobTxContextBuilder};
    use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};
    use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
    use blob_sender_queue::BlobSenderQueueEvent;
    use blob_storage::storage::s3::S3BlobStorageManager;
    use execution_attempt_db::execution_attempts::ExecutionAttemptRepo;
    use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
    use lambda_runtime::LambdaEvent;
    use network_db::networks::NetworkRepo;
    use operator_wallet_db::operator_wallets::OperatorWalletRepo;
    use receipt_poller_queue::ReceiptPollerQueueMessageBody;
    use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
    use tx_request_db::tx_requests::TxRequestRepo;
    use wallet_assignment_db::wallet_assignments::WalletAssignmentRepo;
    use wallet_pool::manager::WalletPoolManager;

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<SqsBatchResponse, lambda_runtime::Error> {
        println!("Building...");

        let config = Config::build()?;

        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        let wallet_assignment_repo = WalletAssignmentRepo::new(&pool);
        let operator_wallet_repo = OperatorWalletRepo::new(&pool);
        let network_repo = NetworkRepo::new(&pool);
        let tx_request_repo = TxRequestRepo::new(&pool);
        let execution_attempt_repo = ExecutionAttemptRepo::new(&pool);
        let execution_attempt_item_repo = ExecutionAttemptItemRepo::new(&pool);
        let networks = network_repo.select_all().await?;
        let blob_storage_manager =
            S3BlobStorageManager::build(&aws_config, &config.blob_storage_bucket_name);

        let wallet_pool_manager = WalletPoolManager::build(operator_wallet_repo, &networks);
        let tx_context_builder =
            BlobTxContextBuilder::build(&tx_request_repo, blob_storage_manager);
        let contract_manager = ContractManager::build(&networks).await?;
        let receipt_poller_queue = SqsQueue::build(
            &aws_config,
            &config.receipt_poller_queue_url,
            &config.receipt_poller_queue_message_group_id,
        )?;

        let mut sqs_batch_response = SqsBatchResponse::default();

        println!("Reading...");
        let tx_sender_queue_event = BlobSenderQueueEvent::from_sqs_event(event)?;

        let tx_ids = tx_sender_queue_event
            .messages
            .iter()
            .map(|message| message.body.tx_id.clone())
            .collect::<Vec<String>>();

        let blob_batch_context_vec = tx_context_builder
            .fetch_and_sort_into_batches(&tx_ids)
            .await?;

        println!("Executing...");
        for blob_batch_context in blob_batch_context_vec {
            let Some(wallet) = wallet_pool_manager
                .acquire(
                    blob_batch_context.chain_id,
                    blob_batch_context.use_operator_wallet_id,
                )
                .await?
            else {
                tx_request_repo
                    .release_many(&blob_batch_context.tx_ids)
                    .await?;
                blob_batch_context.tx_ids.iter().for_each(|tx_id| {
                    if let Some(message_id) = tx_sender_queue_event.tx_id_to_message_id.get(tx_id) {
                        sqs_batch_response.add_failure(message_id);
                    };
                });
                continue;
            };

            let assignment_ids = wallet_assignment_repo
                .new_assignments(&blob_batch_context.tx_ids, wallet.db_record.id)
                .await?;

            let new_execution_attempt = contract_manager
                .send_blob_batch(&blob_batch_context, wallet)
                .await?;

            let execution_attempt = execution_attempt_repo.insert(new_execution_attempt).await?;

            execution_attempt_item_repo
                .insert_many(execution_attempt.id, &blob_batch_context.tx_ids)
                .await?;

            tx_request_repo
                .mark_many_as_broadcasted(&blob_batch_context.tx_ids)
                .await?;

            let receipt_poller_queue_message_body = ReceiptPollerQueueMessageBody {
                execution_attempt_id: execution_attempt.id.to_string(),
            };

            receipt_poller_queue
                .send_new(&receipt_poller_queue_message_body.to_json_string()?)
                .await?;
        }

        Ok(sqs_batch_response)
    }
}
