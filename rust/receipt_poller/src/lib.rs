#[cfg(feature = "aws")]
pub mod orchestrator;
pub mod receipt;

use std::env;

pub struct Config {
    pub database_url: String,
    pub retry_queue_message_group_id: String,
    pub retry_queue_url: String,
    pub outcome_event_bus_name: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let database_url = Self::get_env_var("DATABASE_URL");
        let retry_queue_message_group_id = Self::get_env_var("RETRY_QUEUE_MESSAGE_GROUP_ID");
        let retry_queue_url = Self::get_env_var("RETRY_QUEUE_URL");
        let outcome_event_bus_name = Self::get_env_var("OUTCOME_EVENT_BUS_NAME");

        Ok(Self {
            database_url,
            retry_queue_message_group_id,
            retry_queue_url,
            outcome_event_bus_name,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}
