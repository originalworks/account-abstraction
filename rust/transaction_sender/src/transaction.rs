use alloy::primitives::{Address, Uint, keccak256};
use ow_wallet_adapter::wallet::OwWallet;
use std::{collections::HashMap, str::FromStr};
use transaction_db::transactions::{Transaction, TransactionRepo};
use transaction_sender_queue::TxSenderQueueMessageBody;
use uuid::Uuid;

use crate::{constants::TX_DEADLINE_IN_SEC, contract::sEOA::ExecuteInput};

pub struct ExecuteTxContext {
    pub execute_input: ExecuteInput,
    pub wallet: OwWallet,
    pub use_operator_wallet_id: Option<Uuid>,
    pub pass_value_from_operator_wallet: bool,
}

pub struct ExecuteBatchTxContext {
    pub chain_id: i64,
    pub execute_batch_input: Vec<ExecuteInput>,
    pub use_operator_wallet_id: Option<Uuid>,
    pub batch_tx_value: i64,
    pub tx_ids: Vec<String>,
}

pub struct TxContextBuilder<'a> {
    transaction_repo: &'a TransactionRepo<'a>,
}

impl<'a> TxContextBuilder<'a> {
    pub fn build(transaction_repo: &'a TransactionRepo) -> Self {
        Self { transaction_repo }
    }

    pub async fn fetch_and_sort_into_batches(
        &self,
        input: Vec<TxSenderQueueMessageBody>,
    ) -> anyhow::Result<Vec<ExecuteBatchTxContext>> {
        let ids = input.iter().map(|message| message.tx_id.clone()).collect();
        let fetched_txs = self.transaction_repo.select_and_lock_many(&ids).await?;

        let sorted = Self::group_by_chain_and_wallet(fetched_txs);

        let mut batch_contexts = Vec::new();
        for (chain_id, wallet_map) in sorted {
            for (use_operator_wallet_id, transactions) in wallet_map {
                let context = self
                    .build_batch_context(chain_id, use_operator_wallet_id, transactions)
                    .await;
                if let Some(ctx) = context {
                    batch_contexts.push(ctx);
                }
            }
        }

        Ok(batch_contexts)
    }

    async fn build_batch_context(
        &self,
        chain_id: i64,
        use_operator_wallet_id: Option<Uuid>,
        transactions: Vec<Transaction>,
    ) -> Option<ExecuteBatchTxContext> {
        let mut execute_batch_input = Vec::new();
        let mut batch_tx_value = 0;
        let mut tx_ids = Vec::new();

        for transaction in &transactions {
            if transaction.pass_value_from_operator_wallet && transaction.value_wei > 0 {
                batch_tx_value += transaction.value_wei;
            }

            tx_ids.push(transaction.tx_id.clone());

            match transaction.clone().into_execute_input() {
                Ok(execute_input) => execute_batch_input.push(execute_input),
                Err(_) => {
                    self.transaction_repo
                        .mark_as_invalid(&transaction.tx_id)
                        .await
                        .ok();
                }
            }
        }

        if execute_batch_input.is_empty() {
            return None;
        }

        Some(ExecuteBatchTxContext {
            chain_id,
            use_operator_wallet_id,
            execute_batch_input,
            batch_tx_value,
            tx_ids,
        })
    }

    fn group_by_chain_and_wallet(
        transactions: Vec<Transaction>,
    ) -> HashMap<i64, HashMap<Option<Uuid>, Vec<Transaction>>> {
        let mut grouped: HashMap<i64, HashMap<Option<Uuid>, Vec<Transaction>>> = HashMap::new();

        for tx in transactions {
            grouped
                .entry(tx.chain_id)
                .or_default()
                .entry(tx.use_operator_wallet_id)
                .or_default()
                .push(tx);
        }

        grouped
    }
}

trait ToExecuteInput {
    fn into_execute_input(self) -> anyhow::Result<ExecuteInput>;
}

impl ToExecuteInput for Transaction {
    fn into_execute_input(self) -> anyhow::Result<ExecuteInput> {
        Ok(ExecuteInput {
            target: Address::from_str(self.to_address.as_str())?,
            payload: self.calldata.into(),
            value: Uint::<256, 4>::from(self.value_wei as u64),
            salt: keccak256(self.tx_id.into_bytes()),
            deadline: Uint::<256, 4>::from(TX_DEADLINE_IN_SEC as u64),
            signature: self.signature.into(),
        })
    }
}
