CREATE TABLE IF NOT EXISTS operator_wallets (
    id UUID PRIMARY KEY,
    wallet_address TEXT NOT NULL UNIQUE,
    key_ref TEXT NOT NULL,
    key_type TEXT NOT NULL,
    chain_id BIGINT NOT NULL,
    nonce BIGINT NOT NULL,
    is_enabled BOOLEAN NOT NULL,
    in_use BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_wallet_address_per_chain UNIQUE (wallet_address, chain_id)
);

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE INDEX idx_wallet_free
    ON operator_wallets (chain_id)
    WHERE in_use = FALSE AND is_enabled = TRUE;