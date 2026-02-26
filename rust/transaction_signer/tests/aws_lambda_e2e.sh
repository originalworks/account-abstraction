#!/bin/bash
export POSTGRES_USER=user
export POSTGRES_PASSWORD=password
export POSTGRES_DB=mydb
export PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" # Default private key from Anvil localhost
export USE_KMS=false
export RPC_URL=http://localhost:8545
export DATABASE_URL=postgresql://$POSTGRES_USER:$POSTGRES_PASSWORD@localhost:5432/$POSTGRES_DB

docker run  \
    -d \
    --name local-postgres  \
    -e POSTGRES_USER=$POSTGRES_USER  \
    -e POSTGRES_PASSWORD=$POSTGRES_PASSWORD  \
    -e POSTGRES_DB=$POSTGRES_DB  \
    --health-cmd="pg_isready -U user" \
    --health-interval=2s \
    --health-timeout=2s \
    --health-retries=20 \
    -p 5432:5432  \
    postgres:17

echo "Waiting for local Postgres container..."

until [ "$(docker inspect -f '{{.State.Health.Status}}' local-postgres)" = "healthy" ]; do
  sleep 1
done

echo "Container is ready. Running migrations.."

cd ../../transaction_db
sqlx database create
sqlx migrate run

cd ../transaction_signer
cargo test --features aws --test aws_lambda_e2e -- --ignored --no-capture

echo "Removing local Postgres container..."
docker stop local-postgres
docker rm local-postgres