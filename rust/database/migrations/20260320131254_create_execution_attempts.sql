CREATE TABLE IF NOT EXISTS execution_attempts (
    id UUID PRIMARY KEY,
    operator_wallet_id UUID NOT NULL REFERENCES operator_wallets(id),
    chain_id BIGINT NOT NULL REFERENCES networks(chain_id),
    nonce_used BIGINT NOT NULL,
    tx_type TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    gas_limit BIGINT NOT NULL,
    max_fee_per_gas BIGINT NOT NULL,
    max_priority_fee BIGINT NOT NULL,
    max_fee_per_blob_gas BIGINT,
    outcome TEXT,
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
BEFORE UPDATE ON execution_attempts
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();