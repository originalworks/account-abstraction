#![cfg(feature = "aws")]

use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::{LambdaEvent, tracing::log::warn};
use serde::de::DeserializeOwned;

pub fn tx_requests_from_sqs_event<T>(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let requests: Vec<T> = event
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
            let tx_request_body = match serde_json::from_str::<T>(&body).ok() {
                Some(v) => v,
                None => {
                    warn!("Failed to parse to transaction request body: {:?}", body);
                    return None;
                }
            };

            Some(tx_request_body)
        })
        .collect();

    Ok(requests)
}
