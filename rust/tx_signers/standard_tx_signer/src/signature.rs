use alloy::{
    primitives::{Address, U256, Uint, keccak256},
    signers::Signature,
    sol,
};
use alloy_sol_types::Eip712Domain;
use ow_wallet_adapter::wallet::OwWallet;
use tx_request::standard::StandardTxRequestBody;
// use signer_queue::tx_request::TxRequestBody;
use std::str::FromStr;

use crate::{
    calldata::parse_calldata,
    constants::{EIP712_DOMAIN_NAME, EIP712_DOMAIN_VERSION},
};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug)]
    SEOA,
    "../../../contracts/artifacts/contracts/sEOA.sol/sEOA.json"
);

pub async fn sign_tx_request(
    tx_request_body: &StandardTxRequestBody,
    wallet: &OwWallet,
) -> anyhow::Result<Signature> {
    let signed_call = sEOA::SignedCall {
        target: Address::from_str(&tx_request_body.to_address)?,
        payloadHash: keccak256(parse_calldata(&tx_request_body.calldata)?),
        value: U256::from(tx_request_body.value_wei),
        salt: keccak256(tx_request_body.tx_id.as_bytes()),
        deadline: U256::from(tx_request_body.deadline_timestamp),
    };

    let domain = Eip712Domain {
        name: Some(EIP712_DOMAIN_NAME.into()),
        version: Some(EIP712_DOMAIN_VERSION.into()),
        chain_id: Some(Uint::<256, 4>::from(tx_request_body.chain_id)),
        verifying_contract: Some(wallet.get_address()?),
        salt: None,
    };

    let signature = wallet.sign_typed_data(&signed_call, &domain).await?;
    Ok(signature)
}
