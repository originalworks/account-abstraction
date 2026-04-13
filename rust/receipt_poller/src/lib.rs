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

    use crate::Config;

    use aws_lambda_events::sqs::SqsEvent;

    use lambda_runtime::LambdaEvent;
    use receipt_poller_queue::queue::ReceiptPollerEvent;

    pub async fn function_handler(
        event: LambdaEvent<SqsEvent>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        println!("Building...");
        let config = Config::build()?;

        let event = ReceiptPollerEvent::from_sqs_event(event)?;

        println!("event received by poller: {event:?}");

        Ok(())
    }
}
