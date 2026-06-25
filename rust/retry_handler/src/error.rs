use crate::orchestrator::aws::AwsLambdaOrchestrator;
use execution_attempt_db::execution_attempts::ExecutionAttemptRepo;
use execution_attempt_item_db::execution_attempt_items::ExecutionAttemptItemRepo;
use outcome_emitter::emitter::event_bridge::AwsEventBridgeOutcomeEmitter;
use sqs_queue::queue::SqsQueue;
use standard_tx_sender::error::ExecutionErrorHandler;
use tx_request_db::repo::TxRequestRepo;

impl ExecutionErrorHandler for AwsLambdaOrchestrator {
    fn execution_attempt_repo(&self) -> &ExecutionAttemptRepo {
        &self.execution_attempt_repo
    }

    fn execution_attempt_item_repo(&self) -> &ExecutionAttemptItemRepo {
        &self.execution_attempt_item_repo
    }

    fn tx_request_repo(&self) -> &TxRequestRepo {
        &self.tx_request_repo
    }

    fn retry_queue(&self) -> &SqsQueue {
        &self.retry_queue
    }

    fn outcome_emitter(&self) -> &AwsEventBridgeOutcomeEmitter {
        &self.outcome_emitter
    }
}
