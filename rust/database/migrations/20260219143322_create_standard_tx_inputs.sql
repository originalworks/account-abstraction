CREATE TABLE IF NOT EXISTS standard_tx_inputs (
    tx_id TEXT PRIMARY KEY REFERENCES tx_requests(tx_id) ON DELETE CASCADE,
    signature BYTEA NOT NULL,
    calldata BYTEA NOT NULL,
    to_address TEXT NOT NULL,
    value_wei BIGINT NOT NULL,
    deadline_timestamp BIGINT NOT NULL,
    pass_value_from_operator_wallet BOOLEAN NOT NULL,
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
BEFORE UPDATE ON standard_tx_inputs
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();