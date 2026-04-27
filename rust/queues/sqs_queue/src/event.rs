use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::LambdaEvent;
use serde::de::DeserializeOwned;

use crate::parser::parse_sqs_message_body;

pub trait FromSqsRecord<B> {
    fn from_parts(message_id: String, body: B) -> Self;
}

pub fn build_typed_event<B, M>(event: LambdaEvent<SqsEvent>) -> anyhow::Result<Vec<M>>
where
    B: DeserializeOwned,
    M: FromSqsRecord<B>,
{
    let mut messages = Vec::new();

    for record in event.payload.records {
        let Some(body) = parse_sqs_message_body::<B>(&record)? else {
            continue;
        };

        let Some(message_id) = record.message_id else {
            continue;
        };

        messages.push(M::from_parts(message_id, body));
    }

    Ok(messages)
}
