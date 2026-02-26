#[cfg(test)]
mod tests {
    use crate::event::SignTxRequest;
    use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
    use lambda_runtime::Context;
    use lambda_runtime::LambdaEvent;

    fn build_lambda_event(messages: Vec<SqsMessage>) -> LambdaEvent<SqsEvent> {
        let mut event = SqsEvent::default();
        event.records = messages;

        LambdaEvent::new(event, Context::default())
    }

    fn message_with_body(body: &str) -> SqsMessage {
        let mut message = SqsMessage::default();
        message.body = Some(body.to_string());
        message
    }

    fn message_without_body() -> SqsMessage {
        let mut message = SqsMessage::default();
        message.body = None;
        message
    }

    fn valid_standard_json() -> String {
        serde_json::json!({
            "calldata": "0xdeafbeef",
            "chain_id": 12,
            "tx_id": "abc123",
            "sender_id": "sender-1",
            "tx_type": "STANDARD"
        })
        .to_string()
    }

    fn valid_blob_json() -> String {
        serde_json::json!({
            "calldata": "0xdeafbeef",
            "chain_id": 12,
            "tx_id": "abc123",
            "sender_id": "sender-1",
            "tx_type": "BLOB",
            "blob_file_path": "path/to/file"
        })
        .to_string()
    }

    #[test]
    fn parses_single_valid_blob_type_message() {
        let json = valid_blob_json();
        let event = build_lambda_event(vec![message_with_body(&json)]);
        let result = SignTxRequest::from_sqs_event(event).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn omit_blob_type_without_blob_file_path() {
        let json = r#"{
            "calldata": "0xdeafbeef",
            "chain_id": 12,
            "tx_id": "abc123",
            "sender_id": "sender-1",
            "tx_type": "BLOB",
        }"#;
        let event = build_lambda_event(vec![message_with_body(json)]);
        let result = SignTxRequest::from_sqs_event(event).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn parses_single_valid_standard_type_message() {
        let json = valid_standard_json();
        let event = build_lambda_event(vec![message_with_body(&json)]);
        let result = SignTxRequest::from_sqs_event(event).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parses_multiple_valid_messages() {
        let json1 = valid_standard_json();
        let json2 = valid_blob_json();

        let event = build_lambda_event(vec![message_with_body(&json1), message_with_body(&json2)]);
        let result = SignTxRequest::from_sqs_event(event).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn omits_message_without_body() {
        let json = valid_standard_json();

        let event = build_lambda_event(vec![message_with_body(&json), message_without_body()]);
        let result = SignTxRequest::from_sqs_event(event).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn omits_invalid_json() {
        let valid = valid_blob_json();
        let invalid = r#"not-json"#;

        let event = build_lambda_event(vec![message_with_body(&valid), message_with_body(invalid)]);
        let result = SignTxRequest::from_sqs_event(event).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn returns_empty_vec_if_all_invalid() {
        let event = build_lambda_event(vec![message_without_body(), message_with_body("not-json")]);

        let result = SignTxRequest::from_sqs_event(event).unwrap();

        assert!(result.is_empty());
    }
}
