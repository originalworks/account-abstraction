pub mod calldata;
pub mod signature;

use signer_wallet::IntoSignerWalletConfig;
use std::env;

impl IntoSignerWalletConfig for Config {
    fn into_signer_wallet_config(&self) -> signer_wallet::Config {
        signer_wallet::Config {
            use_kms: self.use_kms,
            private_key: self.private_key.clone(),
            signer_kms_id: self.signer_kms_id.clone(),
        }
    }
}

pub struct Config {
    pub use_kms: bool,
    pub private_key: Option<String>,
    pub signer_kms_id: Option<String>,
    pub sender_standard_queue_url: String,
    pub standard_sender_queue_message_group_id: String,
    pub database_url: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let standard_sender_queue_message_group_id =
            Self::get_env_var("STANDARD_SENDER_QUEUE_MESSAGE_GROUP_ID");
        let sender_standard_queue_url = Self::get_env_var("SENDER_STANDARD_QUEUE_URL");
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
            standard_sender_queue_message_group_id,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}

#[cfg(feature = "aws")]
pub mod aws_lambda {

    use crate::{Config, signature::sign_tx_request};
    use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};
    use aws_lambda_events::sqs::SqsEvent;
    use lambda_runtime::LambdaEvent;
    use network_db::networks::NetworkRepo;
    use signer_wallet::{IntoSignerWalletConfig, manager::SignerWalletManager};
    use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
    use standard_sender_queue::StandardSenderQueueMessageBody;
    use tx_request::{sqs_parser::tx_requests_from_sqs_event, standard::StandardTxRequestBody};
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

        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        let tx_sender_standard_queue = SqsQueue::build(
            &aws_config,
            &config.sender_standard_queue_url,
            &config.standard_sender_queue_message_group_id,
        )?;

        let tx_request_body_vec = tx_requests_from_sqs_event::<StandardTxRequestBody>(event)?;
        let mut wallet_manager =
            SignerWalletManager::build(&networks, &config.into_signer_wallet_config())?;

        for tx_request_body in tx_request_body_vec {
            println!("Signing: {tx_request_body:?}");

            let wallet = wallet_manager.get_wallet(tx_request_body.chain_id).await?;
            let signature = sign_tx_request(&tx_request_body, wallet).await?;

            println!("Saving...");
            let insert_tx_input = tx_request_body.into_db_input(signature.as_bytes().to_vec())?;
            transaction_repo
                .insert_tx_request_with_tx_input(&insert_tx_input)
                .await?;
            let trigger_body = StandardSenderQueueMessageBody {
                tx_id: insert_tx_input.new_tx_request.tx_id,
            };

            tx_sender_standard_queue
                .send_new(&trigger_body.to_json_string()?)
                .await?;
        }

        Ok(())
    }
}
