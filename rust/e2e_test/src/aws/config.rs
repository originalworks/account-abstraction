use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};

pub async fn build_aws_sdk_config() -> anyhow::Result<aws_config::SdkConfig> {
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let aws_config: aws_config::SdkConfig = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    Ok(aws_config)
}
