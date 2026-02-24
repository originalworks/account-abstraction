#![cfg(feature = "aws")]
use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::LambdaEvent;

use crate::event::SignTxRequest;

impl SignTxRequest {
    pub fn from_sqs_event(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Vec<Self>> {
        let requests: Vec<SignTxRequest> = event
            .payload
            .records
            .iter()
            .map(|sqs_message| {
                let body = sqs_message
                    .body
                    .clone()
                    .expect("body not found for sqs record");
                let sign_tx_request: SignTxRequest =
                    serde_json::from_str(&body).expect("failed to parse sqs message body");
                sign_tx_request
            })
            .collect();

        Ok(requests)
    }
}
