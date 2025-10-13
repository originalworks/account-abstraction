# Account abstraction with 7702 PoC

## Core concepts

### EIP 4337 introduced Account Abstraction

#### Participants and infrastructure
    - bundler
    - entryPoint
    - smart account
    - smart account factory
    - paymaster

    Bundler is a backend application that mainly bundles user operations and sends them as an on-chain transaction to the entryPoint.
    It exposes JSON_RPC API so that users (through some kind of sdk or manually) can send user ops, get gas estimations and more (depends on bundle provider).



 