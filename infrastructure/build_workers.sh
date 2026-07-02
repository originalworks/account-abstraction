SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."
export DATABASE_URL=postgres://user:password@localhost/postgres
cargo lambda build --release --features aws --bin aws_migrator
cargo lambda build --release --features aws --bin aws_standard_tx_signer
cargo lambda build --release --features aws --bin aws_standard_tx_sender
cargo lambda build --release --features aws --bin aws_receipt_poller
cargo lambda build --release --features aws --bin aws_retry_handler