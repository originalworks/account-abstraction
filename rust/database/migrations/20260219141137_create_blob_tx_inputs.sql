CREATE TABLE IF NOT EXISTS blob_tx_inputs (
    tx_id TEXT PRIMARY KEY REFERENCES tx_requests(tx_id) ON DELETE CASCADE,
    signature BYTEA NOT NULL,
    image_id BYTEA NOT NULL,
    commitment BYTEA NOT NULL,
    blob_sha2 BYTEA NOT NULL,
    deadline_timestamp BIGINT NOT NULL,
    storage_type TEXT NOT NULL,
    source_file_path TEXT NOT NULL,
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

CREATE TRIGGER trg_set_updated_at
BEFORE UPDATE ON blob_tx_inputs
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();