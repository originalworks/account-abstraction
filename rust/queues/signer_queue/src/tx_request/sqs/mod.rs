#![cfg(feature = "aws")]
use crate::tx_request::TxRequestBody;
use aws_lambda_events::sqs::SqsEvent;
use db_types::TxType;
use lambda_runtime::{LambdaEvent, tracing::log::warn};

#[cfg(test)]
mod tests;

impl TxRequestBody {
    pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Vec<Self>> {
        let requests: Vec<TxRequestBody> = event
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
                let tx_request_body = match serde_json::from_str::<TxRequestBody>(&body).ok() {
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
