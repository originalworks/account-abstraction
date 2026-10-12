use aws_sdk_eventbridge::types::Target;
use aws_sdk_sqs::types::QueueAttributeName;
use serde_json::json;
use sqs_queue::queue::SqsQueue;

use crate::aws::sqs::test_queue::SqsQueueTester;

const OUTCOME_EVENT_RULE_NAME: &str = "outcome-event-rule";

pub async fn attach_outcome_event_bridge_to_queue(
    aws_config: &aws_config::SdkConfig,
    event_bus_name: &String,
    outcome_queue: &SqsQueue,
) -> anyhow::Result<()> {
    let client = aws_sdk_eventbridge::Client::new(aws_config);
    let outcome_queue_arn = outcome_queue.get_queue_arn().await?;
    client
        .create_event_bus()
        .name(event_bus_name)
        .send()
        .await?;

    let event_pattern = json!({
        "source": [outcome_emitter::constants::OUTCOME_EVENT_SOURCE],
        "detail-type": [outcome_emitter::constants::OUTCOME_EVENT_DETAIL_TYPE]
    });

    client
        .put_rule()
        .name(OUTCOME_EVENT_RULE_NAME)
        .event_bus_name(event_bus_name)
        .event_pattern(event_pattern.to_string())
        .send()
        .await?;

    client
        .put_targets()
        .rule(OUTCOME_EVENT_RULE_NAME)
        .event_bus_name(event_bus_name)
        .targets(
            Target::builder()
                .id("sqs-target")
                .arn(&outcome_queue_arn)
                .build()?,
        )
        .send()
        .await?;

    let event_source_arn = format!(
        "arn:aws:events:us-east-1:000000000000:rule/{}/{}",
        event_bus_name, OUTCOME_EVENT_RULE_NAME
    );

    let policy = serde_json::json!({
      "Version": "2012-10-17",
      "Statement": [{
        "Effect": "Allow",
        "Principal": "events.amazonaws.com",
        "Action": "sqs:SendMessage",
        "Resource": &outcome_queue_arn,
        "Condition": {
          "ArnEquals": {
            "aws:SourceArn": event_source_arn
          }
        }
      }]
    });

    outcome_queue
        .client
        .set_queue_attributes()
        .queue_url(&outcome_queue.queue_url)
        .attributes(QueueAttributeName::Policy, policy.to_string())
        .send()
        .await?;

    Ok(())
}
