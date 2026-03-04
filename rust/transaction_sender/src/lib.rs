use ow_wallet_adapter::HasOwWalletFields;
use std::env;

impl HasOwWalletFields for Config {
    fn use_kms(&self) -> bool {
        self.use_kms
    }
    fn rpc_url(&self) -> String {
        self.rpc_url.clone()
    }
    fn private_key(&self) -> Option<String> {
        self.private_key.clone()
    }
    fn signer_kms_id(&self) -> Option<String> {
        self.signer_kms_id.clone()
    }
}

pub struct Config {
    pub use_kms: bool,
    pub rpc_url: String,
    pub private_key: Option<String>,
    pub signer_kms_id: Option<String>,
    pub transaction_sender_queue_url: Option<String>,
    pub database_url: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let transaction_sender_queue_url = env::var("TRANSACTION_SENDER_QUEUE_URL").ok();
        let rpc_url = Self::get_env_var("RPC_URL");
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
            rpc_url,
            private_key,
            signer_kms_id,
            database_url,
            transaction_sender_queue_url,
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
    use lambda_runtime::LambdaEvent;
    use ow_wallet_adapter::{OwWalletConfig, wallet::OwWallet};
    use transaction_db::transactions::TransactionRepo;

    use crate::Config;

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        println!("Building...");

        let config = Config::build()?;
        let wallet_config = OwWalletConfig::from(&config)?;
        let wallet = OwWallet::build(&wallet_config).await?;
        let transaction_repo = TransactionRepo::new(&pool);
        println!("TODO");
        Ok(())
    }
}
