use alloy::{
    // consensus::BlobTransactionSidecarEip7594,
    primitives::{FixedBytes, Uint, keccak256},
};
use blob_storage::storage::s3::S3BlobStorageManager;
use seoa_contract::{
    contract::sEOA::BlobBatchInput,
    transaction::{BlobBatchInputWithSidecar, BlobBatchTxContext},
};
use std::collections::HashMap;
use tx_request_db::{
    repo::TxRequestRepo,
    types::{BlobTxRequestRaw, IntoTxRequestWithInput},
};
use uuid::Uuid;

// #[derive(Debug)]
// pub struct BlobBatchInputWithSidecar {
//     pub blob_batch_input: BlobBatchInput,
//     pub sidecar: BlobTransactionSidecarEip7594,
// }

// #[derive(Debug)]
// pub struct BlobBatchTxContext {
//     pub chain_id: i64,
//     pub blob_batch_with_sidecar_vec: Vec<BlobBatchInputWithSidecar>,
//     pub use_operator_wallet_id: Option<Uuid>,
//     pub tx_ids: Vec<String>,
// }

pub struct BlobTxContextBuilder {
    transaction_repo: TxRequestRepo,
    blob_storage_manager: S3BlobStorageManager,
}

impl BlobTxContextBuilder {
    pub fn build(
        transaction_repo: TxRequestRepo,
        blob_storage_manager: S3BlobStorageManager,
    ) -> Self {
        Self {
            transaction_repo,
            blob_storage_manager,
        }
    }

    pub async fn fetch_and_sort_into_batches(
        &self,
        tx_ids: &Vec<String>,
    ) -> anyhow::Result<Vec<BlobBatchTxContext>> {
        let fetched_txs = self
            .transaction_repo
            .select_and_lock_many_blob(tx_ids)
            .await?;

        let sorted = Self::group_by_chain_and_wallet(fetched_txs);

        let mut batch_contexts = Vec::new();
        for (chain_id, wallet_map) in sorted {
            for (use_operator_wallet_id, transactions) in wallet_map {
                let context = self
                    .build_batch_context(chain_id, use_operator_wallet_id, transactions)
                    .await?;
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
        transactions: Vec<BlobTxRequestRaw>,
    ) -> anyhow::Result<Option<BlobBatchTxContext>> {
        let mut blob_batch_with_sidecar_vec: Vec<BlobBatchInputWithSidecar> = Vec::new();

        let mut tx_requests = Vec::new();

        for transaction in transactions.clone() {
            match transaction.clone().into_blob_batch_input() {
                Ok(blob_batch_input) => {
                    let blob_input_json_file = self
                        .blob_storage_manager
                        .read_json_file(&transaction.source_file_path)
                        .await?;
                    blob_batch_with_sidecar_vec.push(BlobBatchInputWithSidecar {
                        blob_batch_input: blob_batch_input.clone(),
                        sidecar: blob_input_json_file.blob_sidecar,
                    })
                }
                Err(_) => {
                    self.transaction_repo
                        .mark_as_invalid(&transaction.tx_id)
                        .await
                        .ok();
                    continue;
                }
            }
            let tx_request = transaction.into_tx_request_with_input()?;
            tx_requests.push(tx_request);
        }

        if blob_batch_with_sidecar_vec.is_empty() {
            return Ok(None);
        }

        Ok(Some(BlobBatchTxContext {
            chain_id,
            use_operator_wallet_id,
            blob_batch_with_sidecar_vec,
            tx_requests,
            successfully_simulated: false,
            assigned_nonce: None,
            fees: None,
            gas_limit: None,
            tx_hash: None,
            max_fee_per_blob_gas: None,
        }))
    }

    fn group_by_chain_and_wallet(
        transactions: Vec<BlobTxRequestRaw>,
    ) -> HashMap<i64, HashMap<Option<Uuid>, Vec<BlobTxRequestRaw>>> {
        let mut grouped: HashMap<i64, HashMap<Option<Uuid>, Vec<BlobTxRequestRaw>>> =
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

trait IntoBlobBatchInput {
    fn into_blob_batch_input(&self) -> anyhow::Result<BlobBatchInput>;
}

impl IntoBlobBatchInput for BlobTxRequestRaw {
    fn into_blob_batch_input(&self) -> anyhow::Result<BlobBatchInput> {
        let image_id_array: [u8; 32] =
            self.image_id.clone().try_into().map_err(|v: Vec<u8>| {
                anyhow::anyhow!("image_id must be 32 bytes, got {}", v.len())
            })?;
        let blob_sha2_array: [u8; 32] =
            self.blob_sha2.clone().try_into().map_err(|v: Vec<u8>| {
                anyhow::anyhow!("blob_sha2 must be 32 bytes, got {}", v.len())
            })?;

        Ok(BlobBatchInput {
            imageId: FixedBytes::<32>::try_from(image_id_array)?,
            commitment: self.commitment.clone().try_into()?,
            blobSha2: FixedBytes::<32>::try_from(blob_sha2_array)?,
            salt: keccak256(self.tx_id.clone().into_bytes()),
            deadline: Uint::<256, 4>::from(self.deadline_timestamp as u64),
            signature: self.signature.clone().into(),
        })
    }
}
