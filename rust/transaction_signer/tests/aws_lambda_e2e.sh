#!/bin/bash
export POSTGRES_USER=user
export POSTGRES_PASSWORD=password
export POSTGRES_DB=mydb
export PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" # Default private key from Anvil localhost
export USE_KMS=false
export RPC_URL=http://localhost:8545
export DATABASE_URL=postgresql://$POSTGRES_USER:$POSTGRES_PASSWORD@localhost:5432/$POSTGRES_DB
export TRANSACTION_SENDER_QUEUE_URL="http://localhost:4566/000000000000/transaction-sender-queue"
export AWS_ACCESS_KEY_ID=test
export AWS_SECRET_ACCESS_KEY=test
export AWS_REGION=us-east-1
export AWS_ENDPOINT_URL=http://localhost:4566


cd ../../../
docker compose up -d

echo "Waiting for local Postgres container..."

until [ "$(docker inspect -f '{{.State.Health.Status}}' local-postgres)" = "healthy" ]; do
  sleep 1
done

echo "Container is ready. Running migrations.."

cd rust/database
sqlx database create
sqlx migrate run

echo "Waiting for localstack SQS container..."

until [ "$(docker inspect -f '{{.State.Health.Status}}' localstack)" = "healthy" ]; do
  sleep 1
done

echo "Containers are ready, running e2e test..."

cd ../transaction_signer
cargo test --features aws --test aws_lambda_e2e -- --ignored --no-capture

cd ../../
docker compose down -v