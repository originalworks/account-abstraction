# Account Abstraction

The account abstraction (AA) layer accepts transaction requests through Amazon SQS and executes them on Ethereum-compatible networks. It uses an EIP-7702 delegated account (sEOA) to run calls on behalf of the account, while an operator wallet broadcasts the outer transaction and pays for gas.

1. A signer worker reads requests from the standard or blob FIFO queue, signs them, and stores them in PostgreSQL. The AWS deployment uses a master signing key in AWS KMS.
2. A sender worker groups requests into batches and broadcasts them using an operator wallet. Blob requests reference a sidecar stored in S3.
3. The receipt poller checks the result on-chain. The retry worker handles eligible failures, and the layer publishes transaction outcomes to Amazon EventBridge.

## Deploy to AWS

Install Rust, Cargo Lambda, Node.js and npm, the AWS CLI, and the AWS SAM CLI. Configure AWS credentials and review the target environment in [samconfig.toml](infrastructure/aws/samconfig.toml).

- Provision PostgreSQL and configure `DatabaseName`, `DbSecretsName`, and the SSM parameters for the private subnets and database access security group.
- In AWS Secrets Manager, provide the database connection fields and the KMS master key ID. See the [AWS deployment prerequisites](infrastructure/aws/README.md#prequisites).
- From the repository root, compile the contracts used by the Rust bindings, build the Lambda workers, and deploy the SAM stack. Replace `dev` with `stage` or `prod` as needed.

```sh
(cd contracts && npm ci && npx hardhat compile)
bash infrastructure/build_workers.sh
(cd infrastructure/aws && sam deploy --template-file template.yml --config-env dev)
```

After deployment, invoke the stack's migration Lambda to apply the PostgreSQL schema. Before submitting requests, register each target network with its RPC URL and delegated sEOA address, and configure funded operator wallets. The [contract deployment scripts](contracts/scripts/) include helpers for deploying the sEOA implementation and delegating the account with KMS.

## Run tests locally

From the repository root, run the test suite you need with Docker Compose. Both runners start PostgreSQL, LocalStack, and Anvil, apply database migrations, and deploy the test contracts.

```sh
cd local_setup/docker
docker compose --env-file .env.local run --rm e2e_test_runner
docker compose --env-file .env.local run --rm component_test_runner
```

The end-to-end suite covers standard and blob transactions, including retries. The component suite checks the standard signer’s database and queue behavior.

## Enqueue a transaction

For a standard transaction, save the following JSON as `request.json`. Set a unique `tx_id`, identify your producer with `requester_id`, and replace the destination, calldata, chain ID, and deadline with values for your request. The deadline is an integer Unix timestamp in seconds and must be in the future.

```json
{
  "tx_id": "payment-123",
  "requester_id": "payment-worker-321",
  "chain_id": 1,
  "calldata": "0x",
  "to_address": "0x1111111111111111111111111111111111111111",
  "value_wei": 100,
  "deadline_timestamp": 1893456000,
  "pass_value_from_operator_wallet": true,
  "metadata": "order-123"
}
```

- `pass_value_from_operator_wallet` selects which balance covers `value_wei`. Use `true` for the operator wallet and `false` for the sEOA. The field has no effect when the value is zero.
- Optionally set `use_operator_wallet_id` to the UUID of a specific operator wallet. The optional `metadata` string is passed through to the transaction outcome.

Use the queue URL and FIFO message group ID published by the stack in SSM Parameter Store. Run these commands with AWS credentials and a region for the deployment.

```sh
AA_ENV=dev
parameter_prefix="/account-abstraction/${AA_ENV}/sqs/standard-tx-signer"

queue_url=$(aws ssm get-parameter --name "$parameter_prefix/url" --query 'Parameter.Value' --output text)
group_id=$(aws ssm get-parameter --name "$parameter_prefix/message-group-id" --query 'Parameter.Value' --output text)

aws sqs send-message \
  --queue-url "$queue_url" \
  --message-group-id "$group_id" \
  --message-body file://request.json
```

For a blob transaction, upload the [blob input JSON](rust/tx_request/src/blob_tx.rs) to the stack’s S3 bucket and send a `BlobTxRequestBody` to the queue at `/account-abstraction/<environment>/sqs/blob-tx-signer/url` in SSM. Set `storage_type` to `"S3"`, set `source_file_path` to the S3 object key, and supply a FIFO message group ID.

OWEN is a separate tool for submitting blob transactions. To send through AA, point OWEN’s `PROCESSED_BLOB_BUCKET_NAME` at the AA blob bucket and `PROCESSED_BLOB_QUEUE_URL` at the AA blob signer queue. OWEN uploads the processed blob JSON to S3 and enqueues a `BlobTxRequestBody` that references the object. AA then handles signing, broadcasting, receipt polling, and retries. [OWEN in protocol-core](https://github.com/originalworks/protocol-core/tree/master/owen)
