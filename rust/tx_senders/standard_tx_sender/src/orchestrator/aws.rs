#![cfg(feature = "aws")]

use crate::{
    Config,
    contract::{ContractManager, SEOA},
    execution_attempt::NewStandardExecutionAttemptBuilder,
    transaction::{ExecuteBatchTxContext, TxContextBuilder},
};
use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent};
use db_types::{ExecutionErrorObject, TxExecutionOutcome, TxStatus};
use execution_attempt_db::execution_attempts::{ExecutionAttemptRepo, NewExecutionAttempt};
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use lambda_runtime::{LambdaEvent, tracing};
use network_db::networks::NetworkRepo;
use operator_wallet_db::operator_wallets::OperatorWalletRepo;
use outcome_emitter::{emitter::event_bridge::AwsEventBridgeOutcomeEmitter, outcome::OutcomeEvent};
use receipt_poller_queue::ReceiptPollerQueueMessageBody;
use retry_queue::RetryQueueMessageBody;
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use standard_sender_queue::StandardSenderQueueEvent;
use tx_request_db::tx_requests::TxRequestRepo;
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
                Ok(new_execution_attempt) => {
                    let execution_attempt = self
                        .execution_attempt_repo
                        .insert(&new_execution_attempt)
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

                    let receipt_poller_queue_message_body = ReceiptPollerQueueMessageBody {
                        execution_attempt_id: execution_attempt.id.to_string(),
                        batch_size: u8::try_from(execute_batch_context.raw_tx_requests.len())?,
                    };

                    self.receipt_poller_queue
                        .send_new(&receipt_poller_queue_message_body.to_json_string()?)
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

    fn build_failed_new_execution(
        execute_batch_context: &ExecuteBatchTxContext,
        wallet: &Wallet,
        error: anyhow::Error,
    ) -> anyhow::Result<Option<NewExecutionAttempt>> {
        let mut failed_new_execution = NewExecutionAttempt::default_standard(
            execute_batch_context.chain_id,
            wallet.db_record.id,
            execute_batch_context.batch_tx_value,
        );

        match error.downcast::<alloy::contract::Error>() {
            Ok(alloy_error) => {
                match alloy_error.try_decode_into_interface_error::<SEOA::SEOAErrors>() {
                    Ok(decoded) => match decoded {
                        SEOA::SEOAErrors::Expired(_) => {
                            failed_new_execution = NewExecutionAttempt::standard_failed(
                                execute_batch_context,
                                wallet.db_record.id,
                                ExecutionErrorObject {
                                    error_type: "Expired".to_string(),
                                    error_body: None,
                                },
                                false,
                            )
                            .expect("error parsing failed");
                        }
                        SEOA::SEOAErrors::InvalidSignature(_) => {
                            failed_new_execution = NewExecutionAttempt::standard_failed(
                                execute_batch_context,
                                wallet.db_record.id,
                                ExecutionErrorObject {
                                    error_type: "InvalidSignature".to_string(),
                                    error_body: Some(execute_batch_context.to_json_string()?),
                                },
                                false,
                            )
                            .expect("error parsing failed");
                        }
                        SEOA::SEOAErrors::ExecutionFailed(_) => {
                            let batch_size = execute_batch_context.raw_tx_requests.len();
                            let retryable = if batch_size > 1 { true } else { false };
                            failed_new_execution = NewExecutionAttempt::standard_failed(
                                execute_batch_context,
                                wallet.db_record.id,
                                ExecutionErrorObject {
                                    error_type: "ExecutionFailed".to_string(),
                                    error_body: Some(execute_batch_context.to_json_string()?),
                                },
                                retryable,
                            )
                            .expect("error parsing failed");
                        }
                        SEOA::SEOAErrors::AlreadyUsed(_) => {
                            tracing::warn!(
                                "Tried to send transaction that was already used: {execute_batch_context:?}"
                            );
                            return Ok(None);
                        }
                        _ => {
                            failed_new_execution = NewExecutionAttempt::standard_failed(
                                execute_batch_context,
                                wallet.db_record.id,
                                ExecutionErrorObject {
                                    error_type: "Unknown".to_string(),
                                    error_body: Some(execute_batch_context.to_json_string()?),
                                },
                                false,
                            )
                            .expect("error parsing failed");
                        }
                    },
                    Err(encoded_error) => {
                        failed_new_execution = NewExecutionAttempt::standard_failed(
                            execute_batch_context,
                            wallet.db_record.id,
                            ExecutionErrorObject {
                                error_type: "Generic alloy error".to_string(),
                                error_body: Some(encoded_error.to_string()),
                            },
                            false,
                        )
                        .expect("error parsing failed");
                    }
                };
            }
            Err(generic_error) => {
                failed_new_execution = NewExecutionAttempt::standard_failed(
                    execute_batch_context,
                    wallet.db_record.id,
                    ExecutionErrorObject {
                        error_type: "Generic error".to_string(),
                        error_body: Some(generic_error.to_string()),
                    },
                    false,
                )
                .expect("error parsing failed");
            }
        };
        Ok(Some(failed_new_execution))
    }

    async fn handle_error(
        &self,
        execute_batch_context: &ExecuteBatchTxContext,
        wallet: &Wallet,
        error: anyhow::Error,
    ) -> anyhow::Result<()> {
        if let Some(failed_new_execution) =
            Self::build_failed_new_execution(execute_batch_context, wallet, error)?
        {
            let execution_attempt = self
                .execution_attempt_repo
                .insert(&failed_new_execution)
                .await?;

            self.execution_attempt_item_repo
                .insert_many(execution_attempt.id, &execute_batch_context.get_tx_ids())
                .await?;

            if let Some(retryable) = failed_new_execution.retryable.clone() {
                if retryable == true {
                    self.tx_request_repo
                        .set_status_for_many(&execute_batch_context.get_tx_ids(), TxStatus::RETRIED)
                        .await?;
                    let message_body = &RetryQueueMessageBody {
                        execution_attempt_id: execution_attempt.id.to_string(),
                    };
                    let message_body_string = message_body.to_json_string()?;
                    self.retry_queue.send_new(&message_body_string).await?;
                } else {
                    self.tx_request_repo
                        .set_status_for_many(&execute_batch_context.get_tx_ids(), TxStatus::FAILED)
                        .await?;

                    for tx_request in execute_batch_context.raw_tx_requests.clone() {
                        let outcome = failed_new_execution
                            .outcome
                            .clone()
                            .unwrap_or(TxExecutionOutcome::FAILED);

                        self.outcome_emitter
                            .emit_outcome(&OutcomeEvent {
                                outcome,
                                tx_request_id: tx_request.tx_id,
                                gas_fee: failed_new_execution.used_gas,
                                transaction_hash: failed_new_execution.tx_hash.clone(),
                                error: failed_new_execution.error_object.clone(),
                                metadata: tx_request.metadata,
                            })
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }
}
