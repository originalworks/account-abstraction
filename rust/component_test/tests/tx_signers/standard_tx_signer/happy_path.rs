// // use e2e_test::{
// //     aws::sqs::{
// //         event::{TestEventMessage, build_lambda_sqs_event},
// //         test_queue::SqsQueueTester,
// //     },
// //     fixture::E2eTestFixture,
// //     tx_request::{StandardTxRequestBodyForTest, StandardTxRequestBodyOptional},
// // };
// use std::time::Duration;
// use tx_request::standard::StandardTxRequestBody;

// pub async fn happy_path_single_standard_tx(
//     e2e_test_fixture: &E2eTestFixture,
// ) -> anyhow::Result<()> {
//     println!("Entering test: {}", module_path!());
//     let tx_request_body = StandardTxRequestBody::test_build(
//         StandardTxRequestBodyOptional::default(e2e_test_fixture.env_vars.anvil_chain_id),
//     )?;

//     let tx_request_event = build_lambda_sqs_event(vec![TestEventMessage::new(
//         &tx_request_body.to_string(),
//         None,
//     )])?;

//     standard_tx_signer::aws_lambda::function_handler(
//         tx_request_event,
//         &e2e_test_fixture.pool,
//         &e2e_test_fixture.aws_config,
//     )
//     .await
//     .unwrap();

//     let standard_tx_input = e2e_test_fixture
//         .db_repositories
//         .standard_tx_input_repo
//         .find_by_tx_id(&tx_request_body.tx_id)
//         .await?;

//     assert!(standard_tx_input.signature.is_empty() == false);
//     Ok(())
// }
