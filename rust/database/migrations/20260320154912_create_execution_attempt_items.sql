CREATE TABLE IF NOT EXISTS execution_attempt_items (
    id UUID PRIMARY KEY,
    execution_attempt_id UUID NOT NULL REFERENCES execution_attempts(id),
    tx_id TEXT NOT NULL REFERENCES tx_requests(tx_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);