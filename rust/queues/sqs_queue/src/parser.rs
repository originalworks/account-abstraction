use aws_lambda_events::sqs::SqsMessage;
use lambda_runtime::tracing::log::warn;
use serde::de::DeserializeOwned;

pub fn parse_sqs_message_body<T>(message: &SqsMessage) -> anyhow::Result<Option<T>>
where
    T: DeserializeOwned,
{
    let body = match &message.body {
        Some(b) => b,
        None => {
            warn!("Missing message body: {:?}", message);
            return Ok(None);
        }
    };

    match serde_json::from_str::<T>(body) {
        Ok(v) => Ok(Some(v)),
        Err(_) => {
            warn!("Failed to parse body: {:?}", body);
            Ok(None)
        }
    }
}

pub fn parse_sqs_message_body_vec<T>(messages: &[SqsMessage]) -> anyhow::Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let mut out = Vec::new();
    for msg in messages {
        if let Some(parsed) = parse_sqs_message_body(msg)? {
            out.push(parsed);
        }
    }
    Ok(out)
}
