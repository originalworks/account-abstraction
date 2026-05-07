#[cfg(test)]
mod tests {
    use alloy::{primitives::Address, signers::local::PrivateKeySigner};
    use blob_storage::storage::s3::S3BlobStorageManager;
    use blob_tx_input_db::blob_tx_inputs::BlobTxInputRepo;
    use db_types::TxStatus;
    use e2e_test::aws::s3::S3BlobStorageManagerTestFeatures;
    use e2e_test::aws::sqs::TestQueueManager;
    use e2e_test::aws::sqs::event::TestEventMessage;
    use e2e_test::aws::sqs::event::build_lambda_sqs_event;
    use e2e_test::aws::sqs::test_queue::SqsQueueTester;
    use e2e_test::db::network::AddAnvilNetwork;
    use e2e_test::db::operator_wallet::InsertFromMnemonic;
    use e2e_test::tx_request::{BlobTxRequestBodyForTest, BlobTxRequestBodyOptional};
    use e2e_test::{
        aws::config::build_aws_sdk_config,
        db::{drop_and_migrate, get_pool},
    };
    use network_db::networks::NetworkRepo;
    use operator_wallet_db::operator_wallets::OperatorWalletRepo;
    use tx_request::blob_tx::BlobTxRequestBody;
    use tx_request_db::tx_requests::TxRequestRepo;

    pub fn get_seoa_address() -> anyhow::Result<Address> {
        let seoa_private_key = std::env::var("PRIVATE_KEY").unwrap();
        let pk_signer: PrivateKeySigner = seoa_private_key.parse().unwrap();

        Ok(pk_signer.address())
    }

    // #[ignore]
    #[tokio::test]
    async fn single_blob_tx_e2e() -> anyhow::Result<()> {
        let anvil_chain_id = std::env::var("ANVIL_CHAIN_ID").unwrap().parse()?;
        let blob_storage_bucket_name = std::env::var("BLOB_STORAGE_BUCKET_NAME").unwrap();

        let anvil_mnemonic = std::env::var("ANVIL_MNEMONIC").unwrap();

        let pool = get_pool().await?;
        // drop_and_migrate(&pool).await?;
        let network_repo = NetworkRepo::new(&pool);
        let tx_request_repo = TxRequestRepo::new(&pool);
        let blob_tx_input_repo = BlobTxInputRepo::new(&pool);
        let operator_wallet_repo = OperatorWalletRepo::new(&pool);
        let seoa_address = get_seoa_address()?;
        network_repo
            .add_anvil(seoa_address.to_string(), anvil_chain_id)
            .await?;
        operator_wallet_repo
            .insert_from_mnemonic(anvil_mnemonic, anvil_chain_id)
            .await?;

        let aws_config: aws_config::SdkConfig = build_aws_sdk_config().await?;
        let test_queue_manager = TestQueueManager::build(&aws_config).await?;

        let blob_storage_manager =
            S3BlobStorageManager::build(&aws_config, &blob_storage_bucket_name);

        let blob_json_file_names = blob_storage_manager.prepare_for_test().await?;

        let tx_request_body = BlobTxRequestBody::test_build(BlobTxRequestBodyOptional::default(
            anvil_chain_id,
            blob_json_file_names[1].to_string(),
        ))?;

        let tx_request_body_2 = BlobTxRequestBody::test_build(BlobTxRequestBodyOptional::default(
            anvil_chain_id,
            blob_json_file_names[2].to_string(),
        ))?;

        let tx_request_event = build_lambda_sqs_event(vec![
            TestEventMessage::new(&tx_request_body.to_string(), None),
            TestEventMessage::new(&tx_request_body_2.to_string(), None),
        ])?;

        blob_tx_signer::aws_lambda::function_handler(tx_request_event, &pool)
            .await
            .unwrap();

        let blob_tx_input = blob_tx_input_repo
            .find_by_tx_id(&tx_request_body.tx_id)
            .await?;

        assert!(blob_tx_input.signature.is_empty() == false);

        let blob_tx_input_2 = blob_tx_input_repo
            .find_by_tx_id(&tx_request_body_2.tx_id)
            .await?;

        assert!(blob_tx_input_2.signature.is_empty() == false);
        // Transaction Request was signed and is ready to be sent

        let blob_sender_queue_event = test_queue_manager
            .blob_sender_queue
            .receive_messages(5)
            .await?;

        match blob_tx_sender::aws_lambda::function_handler(blob_sender_queue_event, &pool).await {
            Ok(_) => {}
            Err(err) => {
                println!("{err:#?}")
            }
        }

        let receipt_poller_queue_event = test_queue_manager
            .receipt_poller_queue
            .receive_messages(5)
            .await?;

        let mut receipt_found = false;

        while receipt_found == false {
            match receipt_poller::aws_lambda::function_handler(
                receipt_poller_queue_event.clone(),
                &pool,
            )
            .await
            {
                Ok(_) => {}
                Err(err) => {
                    println!("{err:#?}")
                }
            }
            let tx_request = tx_request_repo.find_by_tx_id(&blob_tx_input.tx_id).await?;
            if tx_request.tx_status == TxStatus::EXECUTED {
                receipt_found = true;
            }
        }

        // let retry_queue_event = retry_queue.receive_messages(5).await?;

        // match retry_handler::aws_lambda::function_handler(retry_queue_event, &pool).await {
        //     Ok(_) => {}
        //     Err(err) => {
        //         println!("{err:#?}")
        //     }
        // }

        Ok(())
    }
}
