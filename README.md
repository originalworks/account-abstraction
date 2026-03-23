Example of JSON input for TX request:

```json
{
  "tx_id": "xyz123", // externally assigned unique transaction identifier
  "requester_id": "payment-worker-321",
  "tx_type": "STANDARD", // "STANDARD" or "BLOB"
  "calldata": "0x01",
  "to_address": "0xAA",
  "value_wei": 100,
  "chain_id": 1,
  "pass_value_from_operator_wallet": true,      // "true" -> tx value ("value_wei") covered from operator wallet balance
                                                // "false" -> tx value covered from sEOA balance
  "blob_file_path": "s3://some-bucket", // optional, needed only when "tx_type": "BLOB"
  "use_operator_wallet_id": "00000000-0000-0000-0000-000000000000" // optional, allows to choose specific Operator Wallet
}
```
