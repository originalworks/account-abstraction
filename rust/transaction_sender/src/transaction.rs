use std::{collections::HashMap, str::FromStr};

use alloy::primitives::{Address, Uint, keccak256};
use ow_wallet_adapter::wallet::OwWallet;
use transaction_db::transactions::{Transaction, TransactionRepo};
use transaction_sender_queue::TxSenderQueueMessageBody;
use uuid::Uuid;

use crate::{
    constants::TX_DEADLINE_IN_SEC,
    contract::{SEOA, sEOA::ExecuteInput},
};

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
    pub passed_tx_value: i64,
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

        let mut sorted: HashMap<i64, HashMap<Option<Uuid>, Vec<Transaction>>> =
            HashMap::<i64, HashMap<Option<Uuid>, Vec<Transaction>>>::new();

        for tx in fetched_txs {
            sorted
                .entry(tx.chain_id)
                .or_default()
                .entry(tx.use_operator_wallet_id)
                .or_default()
                .push(tx);
        }

        for (chain_id, specified_operator_wallets) in sorted {
            for (use_operator_wallet_id, transactions) in specified_operator_wallets {
                let execute_batch_input = transactions
                    .iter()
                    .map(|transaction| transaction.into_execute_input().unwrap())
                    .collect::<Vec<ExecuteInput>>();

                let context = ExecuteBatchTxContext {
                    chain_id,
                    use_operator_wallet_id,
                    execute_batch_input,
                    passed_tx_value: 0, // change this!!!!!!!!!!!!!!!!!!11!111!11111111
                };
            }
        }

        Ok(vec![])
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
