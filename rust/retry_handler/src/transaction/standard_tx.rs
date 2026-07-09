use crate::Config;
use crate::constant::BUFFER_DENOMINATOR;
use crate::transaction::{FeeBufferExt, try_option_i64_to_option_u64};
use alloy::eips::eip1559::Eip1559Estimation;
use anyhow::bail;
use db_types::TxExecutionOutcome;
use execution_attempt_db::execution_attempts::ExecutionAttemptRepo;
use execution_attempt_db::execution_attempts::{ExecutionAttempt, NewExecutionAttempt};
use execution_attempt_db::types::ExecutionAttemptWithTxInputs;
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use lambda_runtime::tracing;
use network_db::networks::Network;
use outcome_emitter::emitter::event_bridge::AwsEventBridgeOutcomeEmitter;
use outcome_emitter::outcome::OutcomeEvent;
use receipt_poller_queue::ReceiptPollerQueueMessageBody;
use seoa_contract::contract::sEOA::ExecuteInput;
use seoa_contract::transaction::IntoExecuteInput;
use seoa_contract::{contract::ContractManager, transaction::ExecuteBatchTxContext};
use sqlx::PgPool;
use sqs_queue::message_body::ToJsonString;
use sqs_queue::queue::SqsQueue;
use standard_tx_sender::error::StandardExecutionErrorHandler;
use standard_tx_sender::execution_attempt::ExecutionAttemptFromStandardSuccessful;
use std::sync::Arc;
use tx_input_types::TxInput;
use tx_request_db::repo::TxRequestRepo;
use tx_request_db::types::TxRequestWithInput;
use uuid::Uuid;
use wallet_assignment_db::wallet_assignments::WalletAssignmentRepo;
use wallet_pool::manager::WalletPoolManager;
use wallet_pool::wallet::Wallet;

impl StandardExecutionErrorHandler for StandardTxRetryManager {
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

impl FeeBufferExt for ExecuteBatchTxContext {
    fn apply_fee_buffer(&mut self, network: &Network) -> anyhow::Result<()> {
        let fees = self
            .fees
            .ok_or(anyhow::anyhow!("Can't apply buffer for undefined fees"))?;
        let gas_limit = self.gas_limit.ok_or(anyhow::anyhow!(
            "Can't apply buffer for undefined gas_limit"
        ))?;

        let buffer_ppm = u128::try_from(network.gas_estimation_buffer_ppm)?;

        let fees_with_buffer = Eip1559Estimation {
            max_fee_per_gas: fees.max_fee_per_gas
                + (fees.max_fee_per_gas * buffer_ppm / BUFFER_DENOMINATOR),
            max_priority_fee_per_gas: fees.max_priority_fee_per_gas
                + (fees.max_priority_fee_per_gas * buffer_ppm / BUFFER_DENOMINATOR),
        };

        let gas_limit_with_buffer = gas_limit
            + (gas_limit * u64::try_from(buffer_ppm)? / u64::try_from(BUFFER_DENOMINATOR)?);

        self.fees = Some(fees_with_buffer);
        self.gas_limit = Some(gas_limit_with_buffer);
        Ok(())
    }
}
pub struct StandardTxRetryManager {
    pub contract_manager: Arc<ContractManager>,
    pub wallet_pool_manager: Arc<WalletPoolManager>,
    pub tx_request_repo: TxRequestRepo,
    pub execution_attempt_repo: ExecutionAttemptRepo,
    pub execution_attempt_item_repo: ExecutionAttemptItemRepo,
    pub receipt_poller_queue: SqsQueue,
    pub retry_queue: SqsQueue,
    pub outcome_emitter: AwsEventBridgeOutcomeEmitter,
    pub wallet_assignment_repo: WalletAssignmentRepo,
}

impl StandardTxRetryManager {
    pub fn build(
        contract_manager: &Arc<ContractManager>,
        wallet_pool_manager: &Arc<WalletPoolManager>,
        aws_config: &aws_config::SdkConfig,
        pool: &PgPool,
        config: &Config,
    ) -> anyhow::Result<Self> {
        let execution_attempt_repo = ExecutionAttemptRepo::new(pool.clone());
        let execution_attempt_item_repo = ExecutionAttemptItemRepo::new(pool.clone());
        let tx_request_repo = TxRequestRepo::new(pool.clone());
        let wallet_assignment_repo = WalletAssignmentRepo::new(pool.clone());

        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let retry_queue = SqsQueue::build(
            &sqs_client,
            &config.retry_queue_url,
            &config.retry_queue_message_group_id,
        )?;
        let receipt_poller_queue = SqsQueue::build(
            &sqs_client,
            &config.receipt_poller_queue_url,
            &config.receipt_poller_queue_message_group_id,
        )?;

        let event_bridge_client = aws_sdk_eventbridge::Client::new(&aws_config);
        let outcome_emitter = AwsEventBridgeOutcomeEmitter::build(
            &event_bridge_client,
            &config.outcome_event_bus_name,
        );

        Ok(Self {
            contract_manager: contract_manager.clone(),
            execution_attempt_repo,
            execution_attempt_item_repo,
            tx_request_repo,
            outcome_emitter,
            retry_queue,
            receipt_poller_queue,
            wallet_pool_manager: wallet_pool_manager.clone(),
            wallet_assignment_repo,
        })
    }

    pub async fn split_batch_and_retry(
        &self,
        execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<()> {
        let original_execution_id = execution_attempt.execution_attempt.id;
        let split_execute_batch_context =
            StandardTxRetryManager::split_into_execute_batch_context(execution_attempt)?;

        for mut tx_context in split_execute_batch_context {
            let Some(mut wallet) = self
                .wallet_pool_manager
                .acquire(tx_context.chain_id, None)
                .await?
            else {
                self.tx_request_repo
                    .set_status_for_many(&tx_context.get_tx_ids(), db_types::TxStatus::FAILED)
                    .await?;
                for tx_request in tx_context.tx_requests {
                    self.outcome_emitter()
                        .emit_outcome(&OutcomeEvent {
                            outcome: TxExecutionOutcome::FAILED,
                            tx_request_id: tx_request.tx_id,
                            gas_fee: None,
                            transaction_hash: tx_context.tx_hash.clone(),
                            error: Some(
                                "Failed while retried. Not enough operator wallets".to_string(),
                            ),
                            metadata: tx_request.metadata,
                        })
                        .await?;
                }

                continue;
            };

            self.wallet_assignment_repo
                .new_assignments(&tx_context.get_tx_ids(), wallet.db_record.id)
                .await?;

            match self
                .simulate_retry(
                    &mut tx_context,
                    &mut wallet,
                    &execution_attempt.execution_attempt.id,
                )
                .await
            {
                Ok(_) => {
                    self.send_retry(&mut tx_context, &wallet, &original_execution_id)
                        .await?;
                }
                Err(err) => {
                    tracing::warn!("{err:?}");
                    continue;
                }
            };
        }
        Ok(())
    }

    pub async fn send_retry(
        &self,
        tx_context: &mut ExecuteBatchTxContext,
        wallet: &Wallet,
        original_execution_id: &Uuid,
    ) -> anyhow::Result<()> {
        match self.contract_manager.send_batch(tx_context, &wallet).await {
            Ok(_) => {
                let new_execution_attempt = self
                    .save_successful_tx(&tx_context, &wallet, &original_execution_id)
                    .await?;
                self.send_receipt_poller_queue_message(
                    &tx_context,
                    &new_execution_attempt.id.to_string(),
                )
                .await?;
            }
            Err(err) => {
                tracing::error!("{err:?}");
                self.handle_error(&tx_context, &wallet, err).await?;
            }
        }
        Ok(())
    }

    async fn save_successful_tx(
        &self,
        tx_context: &ExecuteBatchTxContext,
        wallet: &Wallet,
        retried_execution_attempt_id: &Uuid,
    ) -> anyhow::Result<ExecutionAttempt> {
        let execution_attempt_input = NewExecutionAttempt::standard_successful(
            &tx_context,
            wallet.db_record.id,
            Some(retried_execution_attempt_id.clone()),
        )?;

        let new_execution_attempt = self
            .execution_attempt_repo
            .insert(&execution_attempt_input)
            .await?;

        self.execution_attempt_item_repo
            .insert_many(new_execution_attempt.id, &tx_context.get_tx_ids())
            .await?;

        self.tx_request_repo
            .mark_many_as_broadcasted_and_bump_attempts(&tx_context.get_tx_ids())
            .await?;

        Ok(new_execution_attempt)
    }
    async fn send_receipt_poller_queue_message(
        &self,
        tx_context: &ExecuteBatchTxContext,
        execution_attempt_id: &String,
    ) -> anyhow::Result<()> {
        let receipt_poller_queue_message_body = ReceiptPollerQueueMessageBody {
            execution_attempt_id: execution_attempt_id.clone(),
            batch_size: u8::try_from(tx_context.tx_requests.len())?,
        };

        self.receipt_poller_queue
            .send_new(&receipt_poller_queue_message_body.to_json_string()?)
            .await?;
        Ok(())
    }

    pub async fn simulate_retry(
        &self,
        tx_context: &mut ExecuteBatchTxContext,
        wallet: &mut Wallet,
        source_execution_attempt_id: &Uuid,
    ) -> anyhow::Result<()> {
        match self
            .contract_manager
            .simulate_send_batch_tx(tx_context, wallet)
            .await
        {
            Ok(_) => {}
            Err(err) => {
                tracing::error!("{err:?}");
                self.wallet_pool_manager
                    .release_unused(wallet.db_record.id)
                    .await?;
                let failed_execution_attempt = self.handle_error(&tx_context, &wallet, err).await?;
                self.execution_attempt_repo
                    .set_source_execution_attempt_id(
                        &failed_execution_attempt.id,
                        &source_execution_attempt_id,
                    )
                    .await?;
                bail!("Simulation of retry standard tx failed");
            }
        };
        Ok(())
    }

    pub fn split_into_execute_batch_context(
        execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<Vec<ExecuteBatchTxContext>> {
        let mut execute_batch_contexts = Vec::new();

        let use_operator_wallet_id = execution_attempt.tx_requests[0].use_operator_wallet_id;

        let mid = execution_attempt.tx_requests.len().div_ceil(2);

        let (tx_request_batch_a, tx_request_batch_b) = execution_attempt.tx_requests.split_at(mid);

        let context_a = ExecuteBatchTxContext {
            chain_id: execution_attempt.execution_attempt.chain_id,
            use_operator_wallet_id,
            execute_batch_input: tx_request_batch_a
                .iter()
                .map(|tx_request| tx_request.into_execute_input())
                .collect::<anyhow::Result<Vec<ExecuteInput>>>()?,
            batch_tx_value: calculate_batch_tx_value(&tx_request_batch_a.to_vec())?,
            tx_requests: tx_request_batch_a.to_vec(),
            successfully_simulated: false,
            assigned_nonce: None,
            fees: None,
            gas_limit: None,
            tx_hash: None,
        };

        let context_b = ExecuteBatchTxContext {
            chain_id: execution_attempt.execution_attempt.chain_id,
            use_operator_wallet_id,
            execute_batch_input: tx_request_batch_b
                .iter()
                .map(|tx_request| tx_request.into_execute_input())
                .collect::<anyhow::Result<Vec<ExecuteInput>>>()?,
            batch_tx_value: calculate_batch_tx_value(&tx_request_batch_b.to_vec())?,
            tx_requests: tx_request_batch_b.to_vec(),
            successfully_simulated: false,
            assigned_nonce: None,
            fees: None,
            gas_limit: None,
            tx_hash: None,
        };

        execute_batch_contexts.push(context_a);
        execute_batch_contexts.push(context_b);

        Ok(execute_batch_contexts)
    }
}

pub trait IntoExecuteBatchTxContext {
    fn into_execute_batch_context(&self) -> anyhow::Result<ExecuteBatchTxContext>;
}

impl IntoExecuteBatchTxContext for ExecutionAttemptWithTxInputs {
    fn into_execute_batch_context(&self) -> anyhow::Result<ExecuteBatchTxContext> {
        let max_fee_per_gas = u128::try_from(
            self.execution_attempt
                .max_fee_per_gas
                .ok_or(anyhow::anyhow!("Can't parse, missing max_fee_per_gas"))?,
        )?;
        let max_priority_fee_per_gas = u128::try_from(
            self.execution_attempt
                .max_priority_fee
                .ok_or(anyhow::anyhow!(
                    "Can't parse, missing max_priority_fee_per_gas"
                ))?,
        )?;
        let fees = Eip1559Estimation {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        };

        let execute_batch_input = self
            .tx_requests
            .iter()
            .map(|tx_request| tx_request.into_execute_input().unwrap())
            .collect();

        Ok(ExecuteBatchTxContext {
            chain_id: self.execution_attempt.chain_id,
            execute_batch_input,
            use_operator_wallet_id: None,
            batch_tx_value: calculate_batch_tx_value(&self.tx_requests)?,
            tx_requests: self.tx_requests.clone(),
            successfully_simulated: false,
            assigned_nonce: try_option_i64_to_option_u64(self.execution_attempt.nonce_used)?,
            fees: Some(fees),
            gas_limit: try_option_i64_to_option_u64(self.execution_attempt.gas_limit)?,
            tx_hash: self.execution_attempt.tx_hash.clone(),
        })
    }
}

pub fn calculate_batch_tx_value(tx_requests: &Vec<TxRequestWithInput>) -> anyhow::Result<i64> {
    let mut batch_tx_value = 0;

    for tx_request in tx_requests {
        let tx_input = match tx_request.tx_input.clone() {
            TxInput::Blob(_) => bail!("Can't calculate batch tx value for BLOB input"),
            TxInput::Standard(input) => input,
        };
        batch_tx_value += tx_input.value_wei;
    }

    Ok(batch_tx_value)
}
