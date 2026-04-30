use alloy::{
    primitives::{U256, Uint, keccak256},
    signers::Signature,
    sol,
};
use alloy_sol_types::Eip712Domain;
use eip712_domain::constants::{EIP712_DOMAIN_NAME, EIP712_DOMAIN_VERSION};
use ow_wallet_adapter::wallet::OwWallet;
use tx_request::blob_tx::{BlobInputJsonFile, BlobTxRequestBody};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug)]
    SEOA,
    "../../../contracts/artifacts/contracts/sEOA.sol/sEOA.json"
);

pub async fn sign_tx_request(
    tx_request_body: &BlobTxRequestBody,
    blob_input_json_file: &BlobInputJsonFile,
    wallet: &OwWallet,
) -> anyhow::Result<Signature> {
    let signed_call = sEOA::SignedBlobCall {
        imageId: blob_input_json_file.image_id,
        commitmentHash: keccak256(blob_input_json_file.commitment.clone()),
        blobSha2: blob_input_json_file.blob_sha2,
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
