#![cfg(feature = "aws")]

use crate::{
    Config, error::ExecutionErrorHandler,
    execution_attempt::ExecutionAttemptFromStandardSuccessful, transaction::TxContextBuilder,
};
use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
use execution_attempt_db::execution_attempts::{
    ExecutionAttempt, ExecutionAttemptRepo, NewExecutionAttempt,
};
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use lambda_runtime::{LambdaEvent, tracing};
use network_db::networks::NetworkRepo;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use outcome_emitter::emitter::event_bridge::AwsEventBridgeOutcomeEmitter;
use receipt_poller_queue::ReceiptPollerQueueMessageBody;
use seoa_contract::{contract::ContractManager, transaction::ExecuteBatchTxContext};
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use standard_sender_queue::StandardSenderQueueEvent;
use tx_request_db::repo::TxRequestRepo;
use wallet_assignment_db::wallet_assignments::WalletAssignmentRepo;
use wallet_pool::{manager::WalletPoolManager, wallet::Wallet};

pub struct AwsLambdaOrchestrator {
    pub wallet_assignment_repo: WalletAssignmentRepo,
    pub tx_request_repo: TxRequestRepo,
    pub execution_attempt_repo: ExecutionAttemptRepo,
    pub execution_attempt_item_repo: ExecutionAttemptItemRepo,
    pub wallet_pool_manager: WalletPoolManager,
    pub tx_context_builder: TxContextBuilder,
    pub contract_manager: ContractManager,
    pub receipt_poller_queue: SqsQueue,
    pub retry_queue: SqsQueue,
    pub outcome_emitter: AwsEventBridgeOutcomeEmitter,
}

impl AwsLambdaOrchestrator {
    pub async fn build(
        pool: &sqlx::Pool<sqlx::Postgres>,
        aws_config: &aws_config::SdkConfig,
    ) -> anyhow::Result<Self> {
        tracing::info!("Building standard_tx_sender...");

        let config = Config::build()?;

        let wallet_assignment_repo = WalletAssignmentRepo::new(pool.clone());
        let operator_wallet_repo = OperatorWalletRepo::new(pool.clone());
        let network_repo = NetworkRepo::new(pool.clone());
        let tx_request_repo = TxRequestRepo::new(pool.clone());
        let execution_attempt_repo = ExecutionAttemptRepo::new(pool.clone());
        let execution_attempt_item_repo = ExecutionAttemptItemRepo::new(pool.clone());
        let networks = network_repo.select_all().await?;

        let wallet_pool_manager = WalletPoolManager::build(operator_wallet_repo.clone(), &networks);
        let tx_context_builder = TxContextBuilder::build(&tx_request_repo);
        let contract_manager = ContractManager::build(&networks).await?;
        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let receipt_poller_queue = SqsQueue::build(
            &sqs_client,
            &config.receipt_poller_queue_url,
            &config.receipt_poller_queue_message_group_id,
        )?;
        let event_bridge_client = aws_sdk_eventbridge::Client::new(&aws_config);
        let outcome_emitter = AwsEventBridgeOutcomeEmitter::build(
            &event_bridge_client,
            config.outcome_event_bus_name,
        );

        let retry_queue = SqsQueue::build(
            &sqs_client,
            &config.retry_queue_url,
            &config.retry_queue_message_group_id,
        )?;
        Ok(Self {
            wallet_assignment_repo,
            tx_request_repo,
            execution_attempt_repo,
            execution_attempt_item_repo,
            wallet_pool_manager,
            tx_context_builder,
            contract_manager,
            receipt_poller_queue,
            retry_queue,
            outcome_emitter,
        })
    }

    pub async fn function_handler(
        &self,
        event: LambdaEvent<SqsEvent>,
    ) -> anyhow::Result<SqsBatchResponse, lambda_runtime::Error> {
        let mut sqs_batch_response = SqsBatchResponse::default();
        tracing::info!("Reading...");
        let tx_sender_queue_event = StandardSenderQueueEvent::from_sqs_lambda_event(event)?;

        tracing::info!("{tx_sender_queue_event:?}");

        let tx_ids = tx_sender_queue_event
            .messages
            .iter()
            .map(|message| message.body.tx_id.clone())
            .collect::<Vec<String>>();

        let execute_batch_context_vec = self
            .tx_context_builder
            .fetch_and_sort_into_batches(&tx_ids)
            .await?;

        tracing::info!("Executing...");
        for mut execute_batch_context in execute_batch_context_vec {
            let Some(mut wallet) = self
                .wallet_pool_manager
                .acquire(
                    execute_batch_context.chain_id,
                    execute_batch_context.use_operator_wallet_id,
                )
                .await?
            else {
                self.tx_request_repo
                    .release_many(&execute_batch_context.get_tx_ids())
                    .await?;
                execute_batch_context.get_tx_ids().iter().for_each(|tx_id| {
                    if let Some(message_id) = tx_sender_queue_event.tx_id_to_message_id.get(tx_id) {
                        sqs_batch_response.add_failure(message_id);
                    };
                });
                continue;
            };

            let wallet_assignment_ids = self
                .wallet_assignment_repo
                .new_assignments(&execute_batch_context.get_tx_ids(), wallet.db_record.id)
                .await?;

            match self
                .contract_manager
                .simulate_send_batch_tx(&mut execute_batch_context, &mut wallet)
                .await
            {
                Ok(_) => {}
                Err(err) => {
                    tracing::error!("{err:?}");
                    self.wallet_pool_manager
                        .release_unused(wallet.db_record.id)
                        .await?;
                    self.handle_error(&execute_batch_context, &wallet, err)
                        .await?;
                    continue;
                }
            };

            match self
                .contract_manager
                .send_batch(&mut execute_batch_context, &wallet)
                .await
            {
                Ok(_) => {
                    let execution_attempt = self
                        .save_successful_execution(&execute_batch_context, &wallet)
                        .await?;

                    self.send_receipt_poller_queue_message(
                        &execute_batch_context,
                        &execution_attempt.id.to_string(),
                    )
                    .await?;
                }

                Err(err) => {
                    tracing::error!("{err:?}");
                    self.handle_error(&execute_batch_context, &wallet, err)
                        .await?;
                }
            };
        }

        Ok(sqs_batch_response)
    }

    pub async fn save_successful_execution(
        &self,
        execute_batch_context: &ExecuteBatchTxContext,
        wallet: &Wallet,
    ) -> anyhow::Result<ExecutionAttempt> {
        let execution_attempt_input = NewExecutionAttempt::standard_successful(
            execute_batch_context,
            wallet.db_record.id,
            None,
        )?;
        let execution_attempt = self
            .execution_attempt_repo
            .insert(&execution_attempt_input)
            .await?;

        self.execution_attempt_item_repo
            .insert_many(execution_attempt.id, &execute_batch_context.get_tx_ids())
            .await?;

        self.tx_request_repo
            .set_status_for_many(
                &execute_batch_context.get_tx_ids(),
                db_types::TxStatus::BROADCASTED,
            )
            .await?;

        Ok(execution_attempt)
    }

    pub async fn send_receipt_poller_queue_message(
        &self,
        execute_batch_context: &ExecuteBatchTxContext,
        execution_attempt_id: &String,
    ) -> anyhow::Result<()> {
        let receipt_poller_queue_message_body = ReceiptPollerQueueMessageBody {
            execution_attempt_id: execution_attempt_id.clone(),
            batch_size: u8::try_from(execute_batch_context.tx_requests.len())?,
        };

        self.receipt_poller_queue
            .send_new(&receipt_poller_queue_message_body.to_json_string()?)
            .await?;

        Ok(())
    }
}
