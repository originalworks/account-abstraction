use crate::{
    Config,
    constant::BUFFER_DENOMINATOR,
    transaction::{FeeBufferExt, try_option_i64_to_option_u64},
};
use alloy::eips::eip1559::Eip1559Estimation;
use anyhow::bail;
use blob_storage::storage::s3::S3BlobStorageManager;
use blob_tx_sender::{
    error::BlobTxExecutionErrorHandler, execution_attempt::ExecutionAttemptFromSuccessfulBlobTx,
};
use db_types::TxExecutionOutcome;
use execution_attempt_db::{
    execution_attempts::{ExecutionAttempt, ExecutionAttemptRepo, NewExecutionAttempt},
    types::ExecutionAttemptWithTxInputs,
};
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use lambda_runtime::tracing;
use network_db::networks::Network;
use outcome_emitter::{emitter::event_bridge::AwsEventBridgeOutcomeEmitter, outcome::OutcomeEvent};
use receipt_poller_queue::ReceiptPollerQueueMessageBody;
use seoa_contract::{
    contract::ContractManager,
    transaction::{BlobBatchInputWithSidecar, BlobBatchTxContext, IntoBlobBatchInput},
};
use sqlx::PgPool;
use sqs_queue::{message_body::ToJsonString, queue::SqsQueue};
use std::sync::Arc;
use tx_input_types::TxInput;
use tx_request_db::repo::TxRequestRepo;
use uuid::Uuid;
use wallet_assignment_db::wallet_assignments::WalletAssignmentRepo;
use wallet_pool::{manager::WalletPoolManager, wallet::Wallet};

impl BlobTxExecutionErrorHandler for BlobTxRetryManager {
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

impl FeeBufferExt for BlobBatchTxContext {
    fn apply_fee_buffer(&mut self, network: &Network) -> anyhow::Result<()> {
        let fees = self
            .fees
            .ok_or(anyhow::anyhow!("Can't apply buffer for undefined fees"))?;
        let gas_limit = self.gas_limit.ok_or(anyhow::anyhow!(
            "Can't apply buffer for undefined gas_limit"
        ))?;
        let max_fee_per_blob_gas = self.max_fee_per_blob_gas.ok_or(anyhow::anyhow!(
            "Can't apply buffer for undefined max_fee_per_blob_gas"
        ))?;
        let gas_buffer_ppm = u128::try_from(network.gas_estimation_buffer_ppm)?;
        let blob_gas_buffer_ppm = u128::try_from(network.blob_gas_estimation_buffer_ppm)?;

        let fees_with_buffer = Eip1559Estimation {
            max_fee_per_gas: fees.max_fee_per_gas
                + (fees.max_fee_per_gas * gas_buffer_ppm / BUFFER_DENOMINATOR),
            max_priority_fee_per_gas: fees.max_priority_fee_per_gas
                + (fees.max_priority_fee_per_gas * gas_buffer_ppm / BUFFER_DENOMINATOR),
        };

        let gas_limit_with_buffer = gas_limit
            + (gas_limit * u64::try_from(gas_buffer_ppm)? / u64::try_from(BUFFER_DENOMINATOR)?);

        let max_fee_per_blob_gas_with_buffer = max_fee_per_blob_gas
            + (max_fee_per_blob_gas * blob_gas_buffer_ppm / BUFFER_DENOMINATOR);

        self.fees = Some(fees_with_buffer);
        self.gas_limit = Some(gas_limit_with_buffer);
        self.max_fee_per_blob_gas = Some(max_fee_per_blob_gas_with_buffer);
        Ok(())
    }
}

pub struct BlobTxRetryManager {
    pub contract_manager: Arc<ContractManager>,
    pub wallet_pool_manager: Arc<WalletPoolManager>,
    pub tx_request_repo: TxRequestRepo,
    pub execution_attempt_repo: ExecutionAttemptRepo,
    pub execution_attempt_item_repo: ExecutionAttemptItemRepo,
    pub receipt_poller_queue: SqsQueue,
    pub retry_queue: SqsQueue,
    pub outcome_emitter: AwsEventBridgeOutcomeEmitter,
    pub blob_storage_manager: S3BlobStorageManager,
    pub wallet_assignment_repo: WalletAssignmentRepo,
}
impl BlobTxRetryManager {
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
        let blob_storage_manager =
            S3BlobStorageManager::build(&aws_config, &config.blob_storage_bucket_name);

        Ok(Self {
            contract_manager: contract_manager.clone(),
            wallet_pool_manager: wallet_pool_manager.clone(),
            execution_attempt_repo,
            execution_attempt_item_repo,
            tx_request_repo,
            retry_queue,
            outcome_emitter,
            receipt_poller_queue,
            blob_storage_manager,
            wallet_assignment_repo,
        })
    }

    pub fn split_into_blob_batch_context(
        source_tx_context: &BlobBatchTxContext,
    ) -> anyhow::Result<Vec<BlobBatchTxContext>> {
        let mut blob_batch_contexts = Vec::new();
        // let use_operator_wallet_id = execution_attempt.tx_requests[0].use_operator_wallet_id;
        let mid = source_tx_context
            .blob_batch_with_sidecar_vec
            .len()
            .div_ceil(2);

        let (input_with_sidecar_a, input_with_sidecar_b) =
            source_tx_context.blob_batch_with_sidecar_vec.split_at(mid);

        let (tx_requests_a, tx_requests_b) = source_tx_context.tx_requests.split_at(mid);

        let context_a = BlobBatchTxContext {
            chain_id: source_tx_context.chain_id,
            use_operator_wallet_id: source_tx_context.use_operator_wallet_id,
            blob_batch_with_sidecar_vec: input_with_sidecar_a.to_vec(),
            tx_requests: tx_requests_a.to_vec(),
            max_fee_per_blob_gas: source_tx_context.max_fee_per_blob_gas,
            successfully_simulated: false,
            assigned_nonce: None,
            fees: None,
            gas_limit: None,
            tx_hash: None,
        };

        let context_b = BlobBatchTxContext {
            chain_id: source_tx_context.chain_id,
            use_operator_wallet_id: source_tx_context.use_operator_wallet_id,
            blob_batch_with_sidecar_vec: input_with_sidecar_b.to_vec(),
            tx_requests: tx_requests_b.to_vec(),
            max_fee_per_blob_gas: source_tx_context.max_fee_per_blob_gas,
            successfully_simulated: false,
            assigned_nonce: None,
            fees: None,
            gas_limit: None,
            tx_hash: None,
        };

        blob_batch_contexts.push(context_a);
        blob_batch_contexts.push(context_b);

        Ok(blob_batch_contexts)
    }

    pub async fn split_blob_batch_and_retry(
        &self,
        execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<()> {
        let source_tx_context = self.recreate_blob_batch_context(execution_attempt).await?;
        let original_execution_id = execution_attempt.execution_attempt.id;
        let split_blob_batch_context =
            BlobTxRetryManager::split_into_blob_batch_context(&source_tx_context)?;

        for mut tx_context in split_blob_batch_context {
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
                                "Failed while retring. Not enough operator wallets".to_string(),
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

    pub async fn recreate_blob_batch_context(
        &self,
        execution_attempt: &ExecutionAttemptWithTxInputs,
    ) -> anyhow::Result<BlobBatchTxContext> {
        let max_fee_per_gas = u128::try_from(
            execution_attempt
                .execution_attempt
                .max_fee_per_gas
                .ok_or(anyhow::anyhow!("Can't parse, missing max_fee_per_gas"))?,
        )?;
        let max_priority_fee_per_gas =
            u128::try_from(execution_attempt.execution_attempt.max_priority_fee.ok_or(
                anyhow::anyhow!("Can't parse, missing max_priority_fee_per_gas"),
            )?)?;

        let max_fee_per_blob_gas = u128::try_from(
            execution_attempt
                .execution_attempt
                .max_fee_per_blob_gas
                .ok_or(anyhow::anyhow!("Can't parse, missing max_fee_per_blob_gas"))?,
        )?;
        let fees = Eip1559Estimation {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        };

        let mut blob_batch_with_sidecar_vec = Vec::new();

        for tx_request in execution_attempt.tx_requests.clone() {
            let TxInput::Blob(blob_tx_input) = &tx_request.tx_input else {
                anyhow::bail!("expected Blob transaction");
            };
            let blob_batch_input = tx_request.into_blob_batch_input()?;

            let blob_input_json_file = self
                .blob_storage_manager
                .read_json_file(&blob_tx_input.source_file_path)
                .await?;
            blob_batch_with_sidecar_vec.push(BlobBatchInputWithSidecar {
                blob_batch_input: blob_batch_input,
                sidecar: blob_input_json_file.blob_sidecar,
            })
        }

        Ok(BlobBatchTxContext {
            chain_id: execution_attempt.execution_attempt.chain_id,
            blob_batch_with_sidecar_vec,
            use_operator_wallet_id: None,
            max_fee_per_blob_gas: Some(max_fee_per_blob_gas),
            tx_requests: execution_attempt.tx_requests.clone(),
            successfully_simulated: false,
            assigned_nonce: try_option_i64_to_option_u64(
                execution_attempt.execution_attempt.nonce_used,
            )?,
            fees: Some(fees),
            gas_limit: try_option_i64_to_option_u64(execution_attempt.execution_attempt.gas_limit)?,
            tx_hash: execution_attempt.execution_attempt.tx_hash.clone(),
        })
    }

    pub async fn simulate_retry(
        &self,
        tx_context: &mut BlobBatchTxContext,
        wallet: &mut Wallet,
        source_execution_attempt_id: &Uuid,
    ) -> anyhow::Result<()> {
        match self
            .contract_manager
            .simulate_send_blob_batch(tx_context, wallet)
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
                bail!("Simulation of retry blob tx failed");
            }
        };
        Ok(())
    }

    pub async fn send_retry(
        &self,
        tx_context: &mut BlobBatchTxContext,
        wallet: &Wallet,
        original_execution_id: &Uuid,
    ) -> anyhow::Result<()> {
        match self
            .contract_manager
            .send_blob_batch(tx_context, wallet)
            .await
        {
            Ok(_) => {
                let execution_attempt = self
                    .save_successful_execution(&tx_context, &wallet, &original_execution_id)
                    .await?;

                self.send_receipt_poller_queue_message(
                    &tx_context,
                    &execution_attempt.id.to_string(),
                )
                .await?;
            }
            Err(err) => {
                tracing::error!("{err:?}");
                self.handle_error(&tx_context, &wallet, err).await?;
            }
        };
        Ok(())
    }
    pub async fn send_receipt_poller_queue_message(
        &self,
        tx_context: &BlobBatchTxContext,
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

    pub async fn save_successful_execution(
        &self,
        tx_context: &BlobBatchTxContext,
        wallet: &Wallet,
        retried_execution_attempt_id: &Uuid,
    ) -> anyhow::Result<ExecutionAttempt> {
        let execution_attempt_input = NewExecutionAttempt::blob_tx_successful(
            tx_context,
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
}
