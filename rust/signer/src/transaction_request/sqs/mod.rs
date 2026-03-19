#![cfg(feature = "aws")]
use crate::transaction_request::RequestBody;
use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::{LambdaEvent, tracing::log::warn};
use transaction_db::transactions::TxType;

#[cfg(test)]
mod tests;

impl RequestBody {
    pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Vec<Self>> {
        let requests: Vec<RequestBody> = event
            .payload
            .records
            .into_iter()
            .filter_map(|record| {
                let body = match record.body.clone() {
                    Some(b) => b,
                    None => {
                        warn!("Missing message body for: {:?}", &record);
                        return None;
                    }
                };
                let tx_request_body = match serde_json::from_str::<RequestBody>(&body).ok() {
                    Some(v) => v,
                    None => {
                        warn!("Failed to parse to transaction request body: {:?}", body);
                        return None;
                    }
                };
                if tx_request_body.tx_type == TxType::BLOB
                    && tx_request_body.blob_file_path.is_none()
                {
                    warn!(
                        "Invalid BLOB type transaction request: {:?}",
                        tx_request_body
                    );
                    return None;
                }
                Some(tx_request_body)
            })
            .collect();

        Ok(requests)
    }
}
