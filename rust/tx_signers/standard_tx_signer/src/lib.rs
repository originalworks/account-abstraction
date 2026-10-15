pub mod calldata;
pub mod orchestrator;
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
    pub outcome_event_bus_name: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let standard_sender_queue_message_group_id =
            Self::get_env_var("STANDARD_SENDER_QUEUE_MESSAGE_GROUP_ID");
        let sender_standard_queue_url = Self::get_env_var("STANDARD_SENDER_QUEUE_URL");
        let database_url = Self::get_env_var("DATABASE_URL");
        let outcome_event_bus_name = Self::get_env_var("OUTCOME_EVENT_BUS_NAME");
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
            outcome_event_bus_name,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}
