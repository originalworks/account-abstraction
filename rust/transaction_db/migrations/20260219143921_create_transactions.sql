CREATE TABLE IF NOT EXISTS transactions (
    tx_id TEXT PRIMARY KEY,
    sender_id TEXT NOT NULL,
    tx_type TEXT NOT NULL,
    tx_status TEXT NOT NULL,
    calldata TEXT NOT NULL,
    chain_id INT NOT NULL,
    signature TEXT NOT NULL,
    blob_file_path TEXT,
    tx_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()

);

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;