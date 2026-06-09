use serde::{Deserialize, Serialize};
use sqlx::Type;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "text")]
pub enum TxType {
    STANDARD,
    BLOB,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "text")]
pub enum TxStatus {
    SIGNED,
    LOCKED,
    BROADCASTED,
    EXECUTED,
    INVALID,
    RETRIED,
    FAILED,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "text")]
pub enum BlobStorageType {
    S3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionErrorObject {
    pub error_type: String,
    pub error_body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "text")]
pub enum TxExecutionOutcome {
    STUCK,
    DROPPED,
    SUCCEED,
    FAILED,
    REVERTED,
}
