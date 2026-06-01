ALTER TABLE execution_attempts
    ADD COLUMN error_object TEXT,
    ADD COLUMN retryable BOOLEAN,
    ALTER COLUMN nonce_used DROP NOT NULL,
    ALTER COLUMN tx_hash DROP NOT NULL,
    ALTER COLUMN gas_limit DROP NOT NULL,
    ALTER COLUMN max_fee_per_gas DROP NOT NULL,
    ALTER COLUMN max_priority_fee DROP NOT NULL;