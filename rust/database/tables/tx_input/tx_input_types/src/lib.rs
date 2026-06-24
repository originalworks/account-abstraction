use blob_tx_input_db::blob_tx_inputs::BlobTxInput;
use serde::{Deserialize, Serialize};
use standard_tx_input_db::standard_tx_inputs::StandardTxInput;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum TxInput {
    Blob(BlobTxInput),
    Standard(StandardTxInput),
}
