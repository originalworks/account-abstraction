CREATE TABLE IF NOT EXISTS transactions (
    sequence_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tx_id TEXT NOT NULL UNIQUE,
    requester_id TEXT NOT NULL,
    tx_type TEXT NOT NULL,
    tx_status TEXT NOT NULL,
    calldata BYTEA NOT NULL,
    to_address TEXT NOT NULL,
    value_wei BIGINT NOT NULL,
    chain_id BIGINT NOT NULL REFERENCES networks(chain_id),
    pass_value_from_operator_wallet BOOLEAN NOT NULL,
    signature BYTEA NOT NULL,
    blob_file_path TEXT,
    use_operator_wallet_id UUID REFERENCES operator_wallets(id),
    tx_hash TEXT,
    attempts SMALLINT NOT NULL DEFAULT 0,
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

CREATE INDEX idx_tx_status_created
    ON transactions (created_at)
    WHERE tx_status = 'SIGNED';