pub const STANDARD_SENDER_QUEUE_NAME: &str = "sender-standard-queue.fifo";
pub const BLOB_SENDER_QUEUE_NAME: &str = "sender-blob-queue.fifo";
pub const RECEIPT_POLLER_QUEUE_NAME: &str = "receipt-poller-queue.fifo";
pub const RETRY_QUEUE_NAME: &str = "retry-queue.fifo";
pub const TX_OUTCOME_QUEUE_NAME: &str = "tx-outcome-queue";
pub const DEFAULT_TX_MAX_AGE_SEC: i64 = 3600;
pub const MAX_SCHEDULER_RUNS: u8 = 15;
pub const SCHEDULER_INTERVAL_SEC: u64 = 1;
