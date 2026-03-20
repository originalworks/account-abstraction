CREATE TABLE IF NOT EXISTS transaction_assignments (
    id UUID PRIMARY KEY,
    tx_id TEXT NOT NULL REFERENCES tx_requests(tx_id),
    operator_wallet_id UUID NOT NULL REFERENCES operator_wallets(id),
    nonce_used BIGINT,
    gas_limit BIGINT,
    max_fee_per_gas  BIGINT,
    max_priority_fee  BIGINT,
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
BEFORE UPDATE ON transaction_assignments
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();