use aws_sdk_s3::primitives::ByteStream;
use blob_storage::storage::s3::S3BlobStorageManager;
use std::path::Path;

const TEST_BLOB_FOLDER: &str = "../../local_setup/blob_test_files";
const BLOB_FILE_NAMES: &[&str] = &["blob_1.json", "blob_2.json", "blob_3.json"];

#[allow(async_fn_in_trait)]
pub trait S3BlobStorageManagerTestFeatures {
    async fn prepare_for_test(&self) -> anyhow::Result<&[&str]>;
    async fn create_bucket(&self) -> anyhow::Result<()>;
    async fn upload_test_blobs(&self) -> anyhow::Result<&[&str]>;
}

impl S3BlobStorageManagerTestFeatures for S3BlobStorageManager {
    async fn prepare_for_test(&self) -> anyhow::Result<&[&str]> {
        self.create_bucket().await?;
        let uploaded_blob_s3_keys = self.upload_test_blobs().await?;
        Ok(uploaded_blob_s3_keys)
    }

    async fn create_bucket(&self) -> anyhow::Result<()> {
        self.client
            .create_bucket()
            .bucket(self.bucket_name.clone())
            .send()
            .await?;

        Ok(())
    }
    async fn upload_test_blobs(&self) -> anyhow::Result<&[&str]> {
        for blob_file_name in BLOB_FILE_NAMES {
            let body =
                ByteStream::from_path(Path::new(TEST_BLOB_FOLDER).join(blob_file_name)).await?;
            let response = self
                .client
                .put_object()
                .bucket(self.bucket_name.clone())
                .key(blob_file_name.to_string())
                .body(body)
                .send()
                .await?;
            println!("{response:#?}")
        }

        Ok(BLOB_FILE_NAMES)
    }
}
