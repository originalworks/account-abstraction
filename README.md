Example of JSON input for standard TX request:

```json
{
  "tx_id": "xyz123", // self-declared unique transaction identifier
  "requester_id": "payment-worker-321", // self-declared
  "calldata": "0x01",
  "to_address": "0xAA",
  "value_wei": 100,
  "chain_id": 1,
  "deadline_timestamp": "1772460383",
  "pass_value_from_operator_wallet": true,    // "true" -> tx value ("value_wei") covered from operator wallet balance
                                              // "false" -> tx value covered from sEOA balance
                                              // no effect if "value_wei" is zero
  "use_operator_wallet_id": "00000000-0000-0000-0000-000000000000" // optional, allows to choose specific Operator Wallet
}
```
