#![cfg(feature = "aws")]
use aws_config::SdkConfig;
use aws_sdk_s3::Client;
use tokio::io::AsyncReadExt;
use tx_request::blob_tx::BlobInputJsonFile;

pub struct S3BlobStorageManager {
    pub client: Client,
    pub bucket_name: String,
}

impl S3BlobStorageManager {
    pub fn build(aws_config: &SdkConfig, bucket_name: &String) -> Self {
        let mut builder = aws_sdk_s3::config::Builder::from(aws_config);
        if Self::is_local() {
            builder = builder.force_path_style(true);
        }
        let config = builder.build();
        let client = Client::from_conf(config);

        Self {
            client,
            bucket_name: bucket_name.clone(),
        }
    }

    fn is_local() -> bool {
        matches!(
            std::env::var("IS_LOCAL")
                .unwrap_or_else(|_| "false".to_string())
                .as_str(),
            "1" | "true"
        )
    }

    pub async fn read_json_file(&self, file_path: String) -> anyhow::Result<BlobInputJsonFile> {
        let resp = self
            .client
            .get_object()
            .bucket(self.bucket_name.clone())
            .key(file_path)
            .send()
            .await?;

        let mut body = resp.body.into_async_read();
        let mut contents = String::new();
        body.read_to_string(&mut contents).await?;
        let blob_input: BlobInputJsonFile = serde_json::from_str(&contents).unwrap();

        Ok(blob_input)
    }
}
