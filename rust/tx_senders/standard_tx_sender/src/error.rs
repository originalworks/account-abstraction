use crate::{
    execution_attempt::ExecutionAttemptFromStandardFailed, orchestrator::aws::AwsLambdaOrchestrator,
};
use db_types::{ExecutionErrorObject, TxExecutionOutcome, TxStatus};
use execution_attempt_db::execution_attempts::{ExecutionAttemptRepo, NewExecutionAttempt};
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use lambda_runtime::tracing;
use outcome_emitter::{emitter::event_bridge::AwsEventBridgeOutcomeEmitter, outcome::OutcomeEvent};
use retry_queue::RetryQueueMessageBody;
use seoa_contract::{contract::SEOA, transaction::ExecuteBatchTxContext};
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use tx_request_db::repo::TxRequestRepo;
use wallet_pool::wallet::Wallet;

#[allow(async_fn_in_trait)]
pub trait ExecutionErrorHandler {
    fn execution_attempt_repo(&self) -> &ExecutionAttemptRepo;
    fn execution_attempt_item_repo(&self) -> &ExecutionAttemptItemRepo;
    fn tx_request_repo(&self) -> &TxRequestRepo;
    fn retry_queue(&self) -> &SqsQueue;
    fn outcome_emitter(&self) -> &AwsEventBridgeOutcomeEmitter;

    async fn handle_error(
        &self,
        execute_batch_context: &ExecuteBatchTxContext,
        wallet: &Wallet,
        error: anyhow::Error,
    ) -> anyhow::Result<()> {
        if let Some(failed_new_execution) =
            build_failed_new_execution(execute_batch_context, wallet, error)?
        {
            let execution_attempt = self
                .execution_attempt_repo()
                .insert(&failed_new_execution)
                .await?;

            self.execution_attempt_item_repo()
                .insert_many(execution_attempt.id, &execute_batch_context.get_tx_ids())
                .await?;

            if let Some(retryable) = failed_new_execution.retryable.clone() {
                if retryable == true {
                    self.tx_request_repo()
                        .set_status_for_many(&execute_batch_context.get_tx_ids(), TxStatus::RETRIED)
                        .await?;
                    let message_body = &RetryQueueMessageBody {
                        execution_attempt_id: execution_attempt.id.to_string(),
                    };
                    let message_body_string = message_body.to_json_string()?;
                    self.retry_queue().send_new(&message_body_string).await?;
                } else {
                    self.tx_request_repo()
                        .set_status_for_many(&execute_batch_context.get_tx_ids(), TxStatus::FAILED)
                        .await?;

                    for tx_request in execute_batch_context.tx_requests.clone() {
                        let outcome = failed_new_execution
                            .outcome
                            .clone()
                            .unwrap_or(TxExecutionOutcome::FAILED);

                        self.outcome_emitter()
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

pub fn build_failed_new_execution(
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
                        let batch_size = execute_batch_context.tx_requests.len();
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

impl ExecutionErrorHandler for AwsLambdaOrchestrator {
    fn execution_attempt_repo(&self) -> &ExecutionAttemptRepo {
        &self.execution_attempt_repo
    }

    fn execution_attempt_item_repo(&self) -> &ExecutionAttemptItemRepo {
        &self.execution_attempt_item_repo
    }

    fn tx_request_repo(&self) -> &TxRequestRepo {
        &self.tx_request_repo
    }

    fn retry_queue(&self) -> &SqsQueue {
        &self.retry_queue
    }

    fn outcome_emitter(&self) -> &AwsEventBridgeOutcomeEmitter {
        &self.outcome_emitter
    }
}
