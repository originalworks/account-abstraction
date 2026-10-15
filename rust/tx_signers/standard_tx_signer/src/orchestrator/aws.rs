// use std::sync::Mutex;

use crate::{Config, signature::sign_tx_request};
use alloy::signers::Signature;
use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
use db_types::TxExecutionOutcome;
use lambda_runtime::{LambdaEvent, tracing};
use network_db::networks::NetworkRepo;
use outcome_emitter::{emitter::event_bridge::AwsEventBridgeOutcomeEmitter, outcome::OutcomeEvent};
use signer_wallet::{IntoSignerWalletConfig, manager::SignerWalletManager};
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use standard_sender_queue::StandardSenderQueueMessageBody;
use tokio::sync::Mutex;
use tx_request::{sqs_parser::tx_requests_from_sqs_event, standard::StandardTxRequestBody};
use tx_request_db::repo::{InsertTxRequestResult, TxRequestRepo};

pub struct AwsLambdaOrchestrator {
    tx_sender_standard_queue: SqsQueue,
    transaction_repo: TxRequestRepo,
    outcome_emitter: AwsEventBridgeOutcomeEmitter,
    signer_wallet_manager: Mutex<SignerWalletManager>,
}

impl AwsLambdaOrchestrator {
    pub async fn build(
        pool: &sqlx::Pool<sqlx::Postgres>,
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<Self> {
        let config = Config::build()?;
        let transaction_repo = TxRequestRepo::new(pool.clone());
        let network_repo = NetworkRepo::new(pool.clone());
        let networks = network_repo.select_all().await?;

        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let event_bridge_client = aws_sdk_eventbridge::Client::new(&aws_config);

        let tx_sender_standard_queue = SqsQueue::build(
            &sqs_client,
            &config.sender_standard_queue_url,
            &config.standard_sender_queue_message_group_id,
        )?;

        let signer_wallet_manager = Mutex::new(SignerWalletManager::build(
            &networks,
            &config.into_signer_wallet_config(),
        )?);

        let outcome_emitter = AwsEventBridgeOutcomeEmitter::build(
            &event_bridge_client,
            &config.outcome_event_bus_name,
        );

        Ok(Self {
            transaction_repo,
            tx_sender_standard_queue,
            signer_wallet_manager,
            outcome_emitter,
        })
    }

    pub async fn function_handler(
        &self,
        event: LambdaEvent<SqsEvent>,
    ) -> anyhow::Result<(), lambda_runtime::Error> {
        tracing::info!("Building standard_tx_signer...");
        let tx_request_body_vec = tx_requests_from_sqs_event::<StandardTxRequestBody>(event)?;

        for tx_request_body in tx_request_body_vec {
            tracing::info!("Signing: {tx_request_body:?}");
            let signature = match self.sign_with_wallet(&tx_request_body).await {
                Ok(sig) => sig,
                Err(err) => {
                    self.outcome_emitter
                        .emit_outcome(&OutcomeEvent {
                            outcome: TxExecutionOutcome::FAILED,
                            tx_request_id: tx_request_body.tx_id,
                            error: Some(err.to_string()),
                            metadata: tx_request_body.metadata,
                            gas_fee: None,
                            transaction_hash: None,
                        })
                        .await?;
                    continue;
                }
            };

            tracing::info!("Saving...");
            let insert_tx_input = tx_request_body.into_db_input(signature.as_bytes().to_vec())?;
            match self
                .transaction_repo
                .insert_tx_request_with_tx_input(&insert_tx_input)
                .await?
            {
                InsertTxRequestResult::SAVED => {
                    let trigger_body = StandardSenderQueueMessageBody {
                        tx_id: insert_tx_input.new_tx_request.tx_id,
                    };

                    self.tx_sender_standard_queue
                        .send_new(&trigger_body.to_json_string()?)
                        .await?;
                }
                InsertTxRequestResult::SKIPPED => {
                    tracing::warn!(
                        "Warning! This transaction request was already signed! Skipping: {:?}",
                        tx_request_body
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn sign_with_wallet(
        &self,
        tx_request_body: &StandardTxRequestBody,
    ) -> anyhow::Result<Signature> {
        let signature;
        {
            let mut signer_wallet_manager = self.signer_wallet_manager.lock().await;
            let wallet = signer_wallet_manager
                .get_wallet(tx_request_body.chain_id)
                .await?;
            signature = sign_tx_request(&tx_request_body, wallet).await?;
        }
        Ok(signature)
    }
}
