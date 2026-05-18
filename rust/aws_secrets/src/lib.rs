use aws_config::SdkConfig;
use serde::{Deserialize, Serialize};
use std::env;

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct DatabaseSecrets {
    pub password: String,
    pub dbname: String,
    pub engine: String,
    pub port: u64,
    pub dbInstanceIdentifier: String,
    pub host: String,
    pub username: String,
}

pub struct AwsSecretsManager {
    client: aws_sdk_secretsmanager::Client,
    aws_database_secrets_name: String,
    database_name: String,
}

impl AwsSecretsManager {
    pub fn build(aws_config: &SdkConfig) -> anyhow::Result<Self> {
        let client = aws_sdk_secretsmanager::Client::new(&aws_config);
        let aws_database_secrets_name = Self::get_env_var("AWS_DATABASE_SECRETS_NAME");
        let database_name = Self::get_env_var("DATABASE_NAME");
        Ok(Self {
            client,
            aws_database_secrets_name,
            database_name,
        })
    }

    pub fn get_env_var(key: &str) -> String {
        env::var(key).expect(format!("Missing env variable: {key}").as_str())
    }

    pub async fn read_database_secrets(&self) -> anyhow::Result<DatabaseSecrets> {
        let response = &self
            .client
            .get_secret_value()
            .secret_id(&self.aws_database_secrets_name)
            .send()
            .await?;

        let database_secrets_json_string = response
            .secret_string()
            .expect("Could not retrieve secret string from AWS SM");

        let database_secrets: DatabaseSecrets = serde_json::from_str(database_secrets_json_string)?;
        Ok(database_secrets)
    }

    pub async fn read_database_url(&self) -> anyhow::Result<String> {
        let database_secrets = self.read_database_secrets().await?;
        Ok(format!(
            "{}://{}:{}@{}:{}/{}",
            database_secrets.engine,
            database_secrets.username,
            database_secrets.password,
            database_secrets.host,
            database_secrets.port,
            self.database_name
        ))
    }
}
