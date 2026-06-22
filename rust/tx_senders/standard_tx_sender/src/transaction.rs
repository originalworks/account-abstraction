use seoa_contract::transaction::{ExecuteBatchTxContext, IntoExecuteInput};
use std::collections::HashMap;
use tx_request_db::tx_requests::{StandardTxRequestRaw, TxRequestRepo};
use uuid::Uuid;

pub struct TxContextBuilder {
    transaction_repo: TxRequestRepo,
}

impl TxContextBuilder {
    pub fn build(transaction_repo: &TxRequestRepo) -> Self {
        Self {
            transaction_repo: transaction_repo.clone(),
        }
    }

    pub async fn fetch_and_sort_into_batches(
        &self,
        tx_ids: &Vec<String>,
    ) -> anyhow::Result<Vec<ExecuteBatchTxContext>> {
        let fetched_txs = self
            .transaction_repo
            .select_and_lock_many_standard(tx_ids)
            .await?;

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
        transactions: Vec<StandardTxRequestRaw>,
    ) -> Option<ExecuteBatchTxContext> {
        let mut execute_batch_input = Vec::new();
        let mut batch_tx_value = 0;

        for transaction in transactions.clone() {
            match transaction.clone().into_execute_input() {
                Ok(execute_input) => {
                    if transaction.pass_value_from_operator_wallet && transaction.value_wei > 0 {
                        batch_tx_value += transaction.value_wei;
                    }

                    execute_batch_input.push(execute_input.clone())
                }
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
            raw_tx_requests: transactions,
            successfully_simulated: false,
            assigned_nonce: None,
            fees: None,
            gas_limit: None,
            tx_hash: None,
        })
    }

    fn group_by_chain_and_wallet(
        transactions: Vec<StandardTxRequestRaw>,
    ) -> HashMap<i64, HashMap<Option<Uuid>, Vec<StandardTxRequestRaw>>> {
        let mut grouped: HashMap<i64, HashMap<Option<Uuid>, Vec<StandardTxRequestRaw>>> =
            HashMap::new();

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
