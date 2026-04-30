pub mod blob_storage;
pub mod signature;

use std::env;

use signer_wallet::IntoSignerWalletConfig;

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
    pub blob_storage_bucket_name: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let blob_storage_bucket_name = Self::get_env_var("BLOB_STORAGE_BUCKET_NAME");
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
            blob_storage_bucket_name,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}

#[cfg(feature = "aws")]
pub mod aws_lambda {

    use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};
    use aws_lambda_events::sqs::SqsEvent;
    use db_types::BlobStorageType;
    use lambda_runtime::LambdaEvent;
    use network_db::networks::NetworkRepo;
    use signer_wallet::{IntoSignerWalletConfig, manager::SignerWalletManager};
    use tx_request::{blob_tx::BlobTxRequestBody, sqs_parser::tx_requests_from_sqs_event};
    use tx_request_db::tx_requests::TxRequestRepo;

    use crate::{Config, blob_storage::s3::S3BlobStorageManager, signature::sign_tx_request};

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        println!("Building...");

        let config = Config::build()?;
        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        let transaction_repo = TxRequestRepo::new(&pool);
        let network_repo = NetworkRepo::new(&pool);
        let networks = network_repo.select_all().await?;

        let s3_blob_storage_manager =
            S3BlobStorageManager::build(&aws_config, &config.blob_storage_bucket_name);
        let tx_request_body_vec = tx_requests_from_sqs_event::<BlobTxRequestBody>(event)?;
        let mut signer_wallet_manager =
            SignerWalletManager::build(&networks, &config.into_signer_wallet_config())?;

        for tx_request_body in tx_request_body_vec {
            let blob_input_json_file = match tx_request_body.storage_type {
                BlobStorageType::S3 => {
                    s3_blob_storage_manager
                        .read_json_file(tx_request_body.source_file_path.clone())
                        .await?
                }
            };
            let wallet = signer_wallet_manager
                .get_wallet(tx_request_body.chain_id)
                .await?;

            let signature =
                sign_tx_request(&tx_request_body, &blob_input_json_file, wallet).await?;

            let insert_tx_input = tx_request_body
                .into_db_input(&blob_input_json_file, signature.as_bytes().to_vec())?;

            transaction_repo
                .insert_tx_request_with_tx_input(&insert_tx_input)
                .await?;
        }
        Ok(())
    }
}
