ALTER TABLE execution_attempts
ADD COLUMN retried_by_execution_attempt_id UUID
REFERENCES execution_attempts(id);

ALTER TABLE networks
ADD COLUMN max_retry_attempts SMALLINT NOT NULL DEFAULT 3;