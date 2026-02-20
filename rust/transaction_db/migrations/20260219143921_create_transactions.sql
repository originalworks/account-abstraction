CREATE TABLE IF NOT EXISTS transactions (
    tx_id TEXT PRIMARY KEY,
    sender_id TEXT NOT NULL,
    tx_type TEXT NOT NULL,
    tx_status TEXT NOT NULL,
    signed_calldata TEXT,
    blob_file_path TEXT,
    tx_hash TEXT
);