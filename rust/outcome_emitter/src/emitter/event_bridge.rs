#![cfg(feature = "aws")]

use aws_sdk_eventbridge::types::PutEventsRequestEntry;
use db_types::TxExecutionOutcome;
use execution_attempt_db::{
    execution_attempts::ExecutionAttempt,
    types::{ExecutionAttemptWithTxInputs, ExecutionAttemptWithTxs},
};

use crate::{
    constants::{OUTCOME_EVENT_DETAIL_TYPE, OUTCOME_EVENT_SOURCE},
    outcome::OutcomeEvent,
};

pub struct AwsEventBridgeOutcomeEmitter {
    client: aws_sdk_eventbridge::Client,
    event_bus_name: String,
}

impl AwsEventBridgeOutcomeEmitter {
    pub fn build(client: &aws_sdk_eventbridge::Client, event_bus_name: String) -> Self {
        Self {
            client: client.clone(),
            event_bus_name,
        }
    }

    pub async fn emit_outcome(&self, outcome_event: &OutcomeEvent) -> anyhow::Result<()> {
        let event = PutEventsRequestEntry::builder()
            .source(OUTCOME_EVENT_SOURCE)
            .detail_type(OUTCOME_EVENT_DETAIL_TYPE)
            .detail(serde_json::to_string(outcome_event)?)
            .event_bus_name(self.event_bus_name.clone()) // or your custom event bus
            .build();

        self.client.put_events().entries(event).send().await?;

        Ok(())
    }

    pub async fn emit_for_execution_attempt(
        &self,
        execution_attempt_with_txs: &ExecutionAttemptWithTxs,
        outcome: &TxExecutionOutcome,
        used_gas: Option<i64>,
    ) -> anyhow::Result<()> {
        for tx_request in execution_attempt_with_txs.tx_requests.clone() {
            let outcome_event = OutcomeEvent {
                outcome: outcome.clone(),
                tx_request_id: tx_request.tx_id,
                gas_fee: used_gas,
                transaction_hash: execution_attempt_with_txs.execution_attempt.tx_hash.clone(),
                error: execution_attempt_with_txs
                    .execution_attempt
                    .error_object
                    .clone(),
                metadata: tx_request.metadata,
            };

            self.emit_outcome(&outcome_event).await?;
        }
        Ok(())
    }
}
