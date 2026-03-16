use std::{collections::HashMap, str::FromStr};

use alloy::primitives::Address;
use ow_wallet_adapter::wallet::OwWallet;
use transaction_db::transactions::{Transaction, TransactionRepo};
use transaction_sender_queue::TxSenderQueueMessageBody;
use uuid::Uuid;

use crate::contract::{SEOA, sEOA::ExecuteInput};

pub struct ExecuteTxContext {
    pub execute_input: ExecuteInput,
    pub wallet: OwWallet,
    pub use_operator_wallet: Option<Uuid>,
    pub pass_value_from_operator_wallet: bool,
}

pub struct ExecuteBatchTxContext {
    pub chain_id: i64,
    pub execute_input_batch: Vec<ExecuteInput>,
    pub use_operator_wallet: Option<Uuid>,
    pub pass_value_from_operator_wallet: bool,
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
        let ids: Vec<String> = input.iter().map(|message| message.tx_id.clone()).collect();
        let fetched_txs = self.transaction_repo.select_and_lock_many(&ids).await?;

        let mut sorted = HashMap::<i64, HashMap<Option<Uuid>, Vec<Transaction>>>::new();

        for tx in fetched_txs {
            sorted
                .entry(tx.chain_id)
                .or_default()
                .entry(tx.use_operator_wallet_id)
                .or_default()
                .push(tx);
        }

        Ok(vec![])
    }
}
