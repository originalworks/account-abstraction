#[cfg(test)]
mod tests {
    use std::env;

    use alloy::{primitives::Address, signers::local::PrivateKeySigner};
    use e2e_test::aws::sqs::event::TestEventMessage;
    use e2e_test::aws::sqs::event::build_lambda_sqs_event;
    use e2e_test::aws::sqs::test_queue::TestQueue;
    use e2e_test::constants::RECEIPT_POLLER_QUEUE_NAME;
    use e2e_test::constants::RETRY_QUEUE_NAME;
    use e2e_test::constants::SENDER_BLOB_QUEUE_NAME;
    use e2e_test::constants::SENDER_STANDARD_QUEUE_NAME;
    use e2e_test::db::network::AddAnvilNetwork;
    use e2e_test::db::operator_wallet::InsertFromMnemonic;
    use e2e_test::tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional};
    use e2e_test::{
        aws::config::build_aws_sdk_config,
        db::{drop_and_migrate, get_pool},
    };
    use network_db::networks::NetworkRepo;
    use operator_wallet_db::operator_wallets::OperatorWalletRepo;
    use sqs_queue::queue::SqsQueue;
    use standard_tx_input_db::standard_tx_inputs::StandardTxInputRepo;
    use tx_request::standard::StandardTxRequestBody;
    use tx_request_db::tx_requests::TxRequestRepo;

    pub fn get_seoa_address() -> anyhow::Result<Address> {
        let seoa_private_key = std::env::var("PRIVATE_KEY").unwrap();
        let pk_signer: PrivateKeySigner = seoa_private_key.parse().unwrap();

        Ok(pk_signer.address())
    }

    #[ignore]
    #[tokio::test]
    async fn single_standard_tx_e2e() -> anyhow::Result<()> {
        let anvil_chain_id = std::env::var("ANVIL_CHAIN_ID").unwrap().parse()?;
        let anvil_mnemonic = std::env::var("ANVIL_MNEMONIC").unwrap();

        let blob_sender_queue_message_group_id =
            env::var("BLOB_SENDER_QUEUE_MESSAGE_GROUP_ID").unwrap();
        let standard_sender_queue_message_group_id =
            env::var("STANDARD_SENDER_QUEUE_MESSAGE_GROUP_ID").unwrap();
        let receipt_poller_queue_message_group_id =
            env::var("RECEIPT_POLLER_QUEUE_MESSAGE_GROUP_ID").unwrap();
        let retry_queue_message_group_id = env::var("RETRY_QUEUE_MESSAGE_GROUP_ID").unwrap();

        let pool = get_pool().await?;
        drop_and_migrate(&pool).await?;
        let network_repo = NetworkRepo::new(&pool);
        let tx_request_repo = TxRequestRepo::new(&pool);
        let standard_tx_input_repo = StandardTxInputRepo::new(&pool);
        let operator_wallet_repo = OperatorWalletRepo::new(&pool);
        let seoa_address = get_seoa_address()?;
        network_repo.add_anvil(seoa_address.to_string()).await?;
        operator_wallet_repo
            .insert_from_mnemonic(anvil_mnemonic, anvil_chain_id, 1)
            .await?;

        let aws_config: aws_config::SdkConfig = build_aws_sdk_config().await?;
        let standard_sender_queue = SqsQueue::create_and_build(
            &aws_config,
            SENDER_STANDARD_QUEUE_NAME.to_string(),
            standard_sender_queue_message_group_id,
        )
        .await?;

        let blob_sender_queue = SqsQueue::create_and_build(
            &aws_config,
            SENDER_BLOB_QUEUE_NAME.to_string(),
            blob_sender_queue_message_group_id,
        )
        .await?;
        let receipt_poller_queue = SqsQueue::create_and_build(
            &aws_config,
            RECEIPT_POLLER_QUEUE_NAME.to_string(),
            receipt_poller_queue_message_group_id,
        )
        .await?;
        let retry_queue = SqsQueue::create_and_build(
            &aws_config,
            RETRY_QUEUE_NAME.to_string(),
            retry_queue_message_group_id,
        )
        .await?;

        unsafe {
            env::set_var(
                "SENDER_STANDARD_QUEUE_URL",
                &standard_sender_queue.queue_url,
            );
            env::set_var("SENDER_BLOB_QUEUE_URL", &blob_sender_queue.queue_url);
            env::set_var("RECEIPT_POLLER_QUEUE_URL", &receipt_poller_queue.queue_url);
            env::set_var("RETRY_QUEUE_URL", &retry_queue.queue_url);
        }

        let tx_request_body = StandardTxRequestBody::test_build(
            StandardTxRequestBodyOptional::default(anvil_chain_id),
        )?;

        let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
            &tx_request_body.to_string(),
            None,
        )])?;

        standard_tx_signer::aws_lambda::function_handler(tx_request_event, &pool)
            .await
            .unwrap();

        let standard_tx_input = standard_tx_input_repo
            .find_by_tx_id(&tx_request_body.tx_id)
            .await?;

        assert!(standard_tx_input.signature.is_empty() == false);

        // Transaction Request was signed and is ready to be sent

        let sender_queue_event = standard_sender_queue.receive_messages(5).await?;

        match standard_tx_sender::aws_lambda::function_handler(sender_queue_event, &pool).await {
            Ok(_) => {}
            Err(err) => {
                println!("{err:#?}")
            }
        }

        let receipt_poller_queue_event = receipt_poller_queue.receive_messages(5).await?;

        match receipt_poller::aws_lambda::function_handler(receipt_poller_queue_event, &pool).await
        {
            Ok(_) => {}
            Err(err) => {
                println!("{err:#?}")
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
