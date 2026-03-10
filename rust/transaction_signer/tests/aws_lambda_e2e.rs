#[cfg(test)]
mod tests {
    const CHAIN_ID: i64 = 10;

    use aws_config::{BehaviorVersion, meta::region::RegionProviderChain};
    use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
    use lambda_runtime::{Context, LambdaEvent};
    use network_db::networks::{InsertNetworkInput, NetworkRepo};
    use serde_json::json;
    use sqlx::PgPool;
    use transaction_db::transactions::TransactionRepo;
    use transaction_signer::{Config, aws_lambda::function_handler};

    async fn create_transaction_sender_queue() -> anyhow::Result<()> {
        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        let client = aws_sdk_sqs::Client::new(&aws_config);

        client
            .create_queue()
            .queue_name("transaction-sender-queue")
            .send()
            .await?;
        Ok(())
    }

    async fn add_network() -> anyhow::Result<()> {
        let pool = PgPool::connect(&Config::get_env_var("DATABASE_URL")).await?;
        let network_repo = NetworkRepo::new(&pool);
        network_repo
            .insert_new_network(&InsertNetworkInput {
                rpc_url: "http://localhost:8545".to_string(),
                chain_id: CHAIN_ID,
                contract_address: "0x0123".to_string(),
                chain_name: "hardhat".to_string(),
            })
            .await?;
        Ok(())
    }

    // Do not run this test directly!
    // Instead run it with: ./aws_lambda_e2e.sh
    #[ignore]
    #[tokio::test]
    async fn test_function_handler_e2e() -> anyhow::Result<()> {
        create_transaction_sender_queue().await?;
        add_network().await?;
        let tx_id = "abc123";
        let sqs_message_body = json!({
            "tx_id": "abc123",
            "requester_id": "requester-1",
            "tx_type": "STANDARD",
            "calldata": "0xdeafbeef",
            "to_address": "0x00112233",
            "value_wei": 123,
            "chain_id": CHAIN_ID
        });

        let mut sqs_message = SqsMessage::default();
        sqs_message.body = Some(sqs_message_body.to_string());

        let mut sqs_event = SqsEvent::default();
        sqs_event.records = vec![sqs_message];

        let lambda_event = LambdaEvent::new(sqs_event, Context::default());
        let pool = PgPool::connect(&Config::get_env_var("DATABASE_URL")).await?;
        match function_handler(lambda_event, &pool).await {
            Ok(value) => value,
            Err(err) => {
                println!("{err:?}");
                panic!("function_handler error");
            }
        };

        let transaction_repo = TransactionRepo::new(&pool);
        let inserted_transaction = transaction_repo.find_by_tx_id(tx_id.to_string()).await?;

        assert!(inserted_transaction.signature.is_empty() == false);
        Ok(())
    }
}
