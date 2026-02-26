#![cfg(feature = "aws")]
use crate::event::SignTxRequest;
use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::LambdaEvent;
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
                let body = record.body?;
                let sign_tx_request = serde_json::from_str::<SignTxRequest>(&body).ok()?;
                if sign_tx_request.tx_type == TxType::BLOB
                    && sign_tx_request.blob_file_path.is_none()
                {
                    return None;
                }
                Some(sign_tx_request)
            })
            .collect();

        Ok(requests)
    }
}
