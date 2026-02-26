#[cfg(test)]
mod tests {
    use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
    use lambda_runtime::{Context, LambdaEvent};
    use serde_json::json;
    use sqlx::PgPool;
    use transaction_db::transactions::TransactionRepo;
    use transaction_signer::{Config, aws_lambda::function_handler};

    // Do not run this test directly!
    // Instead run it with: ./aws_lambda_e2e.sh
    #[ignore]
    #[tokio::test]
    async fn test_function_handler_e2e() {
        let tx_id = "abc123";
        let sqs_message_body = json!({
            "calldata": "0xdeafbeef",
            "chain_id": 12,
            "tx_id": tx_id,
            "sender_id": "test_sender",
            "tx_type": "STANDARD"
        });

        let mut sqs_message = SqsMessage::default();
        sqs_message.body = Some(sqs_message_body.to_string());

        let mut sqs_event = SqsEvent::default();
        sqs_event.records = vec![sqs_message];

        let lambda_event = LambdaEvent::new(sqs_event, Context::default());
        let pool = PgPool::connect(&Config::get_env_var("DATABASE_URL"))
            .await
            .unwrap();

        let result = function_handler(lambda_event, &pool).await;

        assert!(result.is_ok());

        let transaction_repo = TransactionRepo::new(&pool);
        let inserted_transaction = transaction_repo
            .find_by_tx_id(tx_id.to_string())
            .await
            .unwrap();

        assert!(inserted_transaction.signature.is_empty() == false);
    }
}
