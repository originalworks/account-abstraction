#[cfg(test)]
mod tests {
    use alloy::{primitives::Address, signers::local::PrivateKeySigner};
    use e2e_test::aws::sqs::SenderQueueTestHelper;
    use e2e_test::aws::sqs::TestEventMessage;
    use e2e_test::aws::sqs::build_lambda_sqs_event;
    use e2e_test::db::network::AddAnvilNetwork;
    use e2e_test::db::operator_wallet::InsertFromMnemonic;
    use e2e_test::tx_request::CreateTestTxRequestBody;
    use e2e_test::tx_request::TxRequestBodyOptional;
    use e2e_test::{
        aws::config::build_aws_sdk_config,
        db::{drop_and_migrate, get_pool},
    };
    use network_db::networks::NetworkRepo;
    use operator_wallet_db::operator_wallets::OperatorWalletRepo;
    use signer_queue::tx_request::TxRequestBody;
    use tx_request_db::tx_requests::TxRequestRepo;

    pub fn get_seoa_address() -> anyhow::Result<Address> {
        let seoa_private_key = std::env::var("PRIVATE_KEY").unwrap();
        let pk_signer: PrivateKeySigner = seoa_private_key.parse().unwrap();

        Ok(pk_signer.address())
    }

    #[tokio::test]
    async fn single_standard_tx_e2e() -> anyhow::Result<()> {
        let anvil_chain_id = std::env::var("ANVIL_CHAIN_ID").unwrap().parse()?;
        let anvil_mnemonic = std::env::var("ANVIL_MNEMONIC").unwrap();

        let pool = get_pool().await?;
        drop_and_migrate(&pool).await?;
        let network_repo = NetworkRepo::new(&pool);
        let tx_request_repo = TxRequestRepo::new(&pool);
        let operator_wallet_repo = OperatorWalletRepo::new(&pool);
        let seoa_address = get_seoa_address()?;
        network_repo.add_anvil(seoa_address.to_string()).await?;
        operator_wallet_repo
            .insert_from_mnemonic(anvil_mnemonic, anvil_chain_id, 5)
            .await?;

        let aws_config: aws_config::SdkConfig = build_aws_sdk_config().await?;
        let sender_queue_test_helper = SenderQueueTestHelper::build(aws_config).await?;

        let tx_request_body = TxRequestBody::test_build(TxRequestBodyOptional::default(
            db_types::TxType::STANDARD,
            anvil_chain_id,
        ))?;

        let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
            &tx_request_body.to_string(),
            None,
        )])?;

        signer::aws_lambda::function_handler(tx_request_event, &pool)
            .await
            .unwrap();

        let tx_request_db_record = tx_request_repo
            .find_by_tx_id(&tx_request_body.tx_id)
            .await?;

        assert!(tx_request_db_record.signature.is_empty() == false);

        // Transaction Request was signed and is ready to be sent

        let sender_queue_event = sender_queue_test_helper
            .receive_messages(db_types::TxType::STANDARD, 5)
            .await?;

        println!("sender_queue_event in happy path: {sender_queue_event:#?}");

        match sender::aws_lambda::function_handler(sender_queue_event, &pool).await {
            Ok(_) => {}
            Err(err) => {
                println!("{err:#?}")
            }
        }

        Ok(())
    }
}
