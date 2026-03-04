CREATE TABLE IF NOT EXISTS transaction_assignments (
    id UUID PRIMARY KEY,
    transaction_sequence_id BIGINT NOT NULL REFERENCES transactions(sequence_id),
    operator_wallet_id UUID NOT NULL REFERENCES operator_wallets(id),
    outcome TEXT NOT NULL,
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