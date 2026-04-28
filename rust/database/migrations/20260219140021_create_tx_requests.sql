CREATE TABLE IF NOT EXISTS tx_requests (
    sequence_id BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE,
    tx_id TEXT PRIMARY KEY,
    requester_id TEXT NOT NULL,
    tx_type TEXT NOT NULL,
    tx_status TEXT NOT NULL,
    chain_id BIGINT NOT NULL REFERENCES networks(chain_id),
    use_operator_wallet_id UUID REFERENCES operator_wallets(id),
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

CREATE TRIGGER trg_set_updated_at
BEFORE UPDATE ON tx_requests
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE INDEX idx_tx_status_created
    ON tx_requests (tx_status, created_at)
    WHERE tx_status = 'SIGNED';