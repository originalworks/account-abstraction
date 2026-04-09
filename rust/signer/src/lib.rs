pub mod calldata;
pub mod wallet;

use std::env;

pub struct Config {
    pub use_kms: bool,
    pub private_key: Option<String>,
    pub signer_kms_id: Option<String>,
    pub sender_standard_queue_url: String,
    pub sender_blob_queue_url: String,
    pub blob_queue_message_group_id: String,
    pub standard_queue_message_group_id: String,
    pub database_url: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let blob_queue_message_group_id = Self::get_env_var("BLOB_QUEUE_MESSAGE_GROUP_ID");
        let standard_queue_message_group_id = Self::get_env_var("STANDARD_QUEUE_MESSAGE_GROUP_ID");
        let sender_standard_queue_url = Self::get_env_var("SENDER_STANDARD_QUEUE_URL");
        let sender_blob_queue_url = Self::get_env_var("SENDER_BLOB_QUEUE_URL");
        let database_url = Self::get_env_var("DATABASE_URL");
        let mut signer_kms_id = None;
        let mut private_key = None;
        let use_kms = matches!(
            std::env::var("USE_KMS")
                .unwrap_or_else(|_| "false".to_string())
                .as_str(),
            "1" | "true"
        );

        if use_kms {
            signer_kms_id = Some(Self::get_env_var("SIGNER_KMS_ID"));
        } else {
            private_key = Some(Self::get_env_var("PRIVATE_KEY"));
        }

        Ok(Self {
            use_kms,
            private_key,
            signer_kms_id,
            database_url,
            sender_standard_queue_url,
            sender_blob_queue_url,
            blob_queue_message_group_id,
            standard_queue_message_group_id,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}

#[cfg(feature = "aws")]
pub mod aws_lambda {
    use std::collections::HashMap;

    use crate::{Config, calldata::parse_calldata, wallet::WalletManager};
    use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};
    use aws_lambda_events::sqs::SqsEvent;
    use db_types::TxType;
    use lambda_runtime::LambdaEvent;
    use network_db::networks::{Network, NetworkRepo};
    use ow_wallet_adapter::{OwWalletConfig, wallet::OwWallet};
    use sender_queue::{
        blob_queue::{SenderQueueBlobMessageBody, sqs::SenderBlobSqsQueue},
        standard_queue::{SenderQueueStandardMessageBody, sqs::SenderStandardSqsQueue},
    };
    use signer_queue::tx_request::TxRequestBody;
    use tx_request_db::tx_requests::TxRequestRepo;

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        println!("Building...");
        let config = Config::build()?;

        let transaction_repo = TxRequestRepo::new(&pool);
        let network_repo = NetworkRepo::new(&pool);
        let networks = network_repo.select_all().await?;
        // let mut networks_by_chain_id = HashMap::<i64, Network>::new();
        // for network in networks {
        //     networks_by_chain_id.insert(network.chain_id, network.clone());
        // }

        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        let tx_sender_standard_queue = SenderStandardSqsQueue::build(
            &aws_config,
            &config.sender_standard_queue_url,
            &config.standard_queue_message_group_id,
        )?;

        let tx_sender_blob_queue = SenderBlobSqsQueue::build(
            &aws_config,
            &config.sender_blob_queue_url,
            &config.blob_queue_message_group_id,
        )?;

        let tx_request_body_vec = TxRequestBody::from_sqs_event(event)?;
        let mut wallet_manager = WalletManager::build(&networks, &config)?;

        for tx_request_body in tx_request_body_vec {
            println!("Signing: {tx_request_body:?}");

            let wallet = wallet_manager.get_wallet(tx_request_body.chain_id).await?;

            let calldata = parse_calldata(&tx_request_body.calldata)?;

            let signature = wallet.sign_message(calldata.as_slice()).await?;

            println!("Saving...");
            let insert_tx_input = tx_request_body.into_db_input(signature.as_bytes().to_vec())?;
            transaction_repo
                .insert_ignore_conflict(&insert_tx_input)
                .await?;
            if tx_request_body.tx_type == TxType::STANDARD {
                let trigger_body = SenderQueueStandardMessageBody {
                    tx_id: insert_tx_input.tx_id,
                };

                tx_sender_standard_queue.send_new(&trigger_body).await?;
            } else {
                let trigger_body = SenderQueueBlobMessageBody {
                    tx_id: insert_tx_input.tx_id,
                };

                tx_sender_blob_queue.send_new(&trigger_body).await?;
            }
        }

        Ok(())
    }
}
