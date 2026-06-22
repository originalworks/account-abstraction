#![recursion_limit = "256"]

pub mod execution_attempt;
pub mod orchestrator;
pub mod transaction;

use std::env;

pub struct Config {
    pub database_url: String,
    pub receipt_poller_queue_url: String,
    pub receipt_poller_queue_message_group_id: String,
    pub retry_queue_message_group_id: String,
    pub retry_queue_url: String,
    pub outcome_event_bus_name: String,
}

impl Config {
    pub fn build() -> anyhow::Result<Self> {
        let database_url = Self::get_env_var("DATABASE_URL");
        let receipt_poller_queue_message_group_id =
            Self::get_env_var("RECEIPT_POLLER_QUEUE_MESSAGE_GROUP_ID");
        let receipt_poller_queue_url = Self::get_env_var("RECEIPT_POLLER_QUEUE_URL");
        let retry_queue_message_group_id = Self::get_env_var("RETRY_QUEUE_MESSAGE_GROUP_ID");
        let retry_queue_url = Self::get_env_var("RETRY_QUEUE_URL");
        let outcome_event_bus_name = Self::get_env_var("OUTCOME_EVENT_BUS_NAME");

        Ok(Self {
            database_url,
            receipt_poller_queue_message_group_id,
            receipt_poller_queue_url,
            retry_queue_message_group_id,
            retry_queue_url,
            outcome_event_bus_name,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }
}
