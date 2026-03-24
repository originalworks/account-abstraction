CREATE TABLE IF NOT EXISTS networks (
    chain_id BIGINT PRIMARY KEY,
    chain_name TEXT NOT NULL,
    rpc_url TEXT NOT NULL,
    contract_address TEXT NOT NULL,
    min_operator_wallet_balance BIGINT NOT NULL,
    gas_estimation_buffer_ppm BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);


CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_set_updated_at
BEFORE UPDATE ON networks
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();