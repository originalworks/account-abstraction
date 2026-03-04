#![cfg(feature = "aws")]
use crate::event::SignTxRequest;
use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::{LambdaEvent, tracing::log::warn};
use transaction_db::transactions::TxType;

#[cfg(test)]
mod tests;

impl SignTxRequest {
    pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Vec<Self>> {
        let requests: Vec<SignTxRequest> = event
            .payload
            .records
            .into_iter()
            .filter_map(|record| {
                let body = record.body.clone()?;
                let sign_tx_request = serde_json::from_str::<SignTxRequest>(&body).ok()?;
                if sign_tx_request.tx_type == TxType::BLOB
                    && sign_tx_request.blob_file_path.is_none()
                {
                    warn!("Failed to parse event: {:?}", record);
                    return None;
                }
                Some(sign_tx_request)
            })
            .collect();

        Ok(requests)
    }
}

pub struct TxSenderSqsTrigger {
    client: aws_sdk_sqs::Client,
    transaction_sender_queue_url: String,
}

impl TxSenderSqsTrigger {
    pub fn build(
        aws_config: &aws_config::SdkConfig,
        transaction_sender_queue_url: &String,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            client: aws_sdk_sqs::Client::new(aws_config),
            transaction_sender_queue_url: transaction_sender_queue_url.clone(),
        })
    }

    pub async fn trigger_tx_sender(&self) -> anyhow::Result<()> {
        let response = self
            .client
            .send_message()
            .queue_url(&self.transaction_sender_queue_url)
            .message_body("{}")
            .send()
            .await?;
        println!("{response:?}");
        Ok(())
    }
}
