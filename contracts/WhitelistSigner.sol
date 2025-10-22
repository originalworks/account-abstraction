// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {AbstractSigner} from "@openzeppelin/contracts/utils/cryptography/signers/AbstractSigner.sol";
import {IEntryPoint} from "@openzeppelin/contracts/interfaces/draft-IERC4337.sol";
import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import {IERC1271} from "@openzeppelin/contracts/interfaces/IERC1271.sol";
import "@openzeppelin/contracts/account/extensions/draft-ERC7821.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";

abstract contract WhitelistSigner is AbstractSigner, ERC7821, AccessControl {
    event SignersWhitelistUpdated(address signer, bool allowed);

    bytes32 public constant PAYMENT_SENDER_ROLE =
        keccak256("PAYMENT_SENDER_ROLE");
    bytes32 public constant TOKENIZER_ROLE = keccak256("TOKENIZER_ROLE");
    bytes32 public constant BLOB_SENDER_ROLE = keccak256("BLOB_SENDER_ROLE");

    function entryPoint() public view virtual returns (IEntryPoint);

    function grantRole(bytes32 role, address account) public override {
        require(
            msg.sender == address(this),
            "Unauthorized: only self-call allowed"
        );
        _grantRole(role, account);
    }

    function revokeRole(bytes32 role, address account) public override {
        require(
            msg.sender == address(this),
            "Unauthorized: only self-call allowed"
        );
        _revokeRole(role, account);
    }

    function _erc7821AuthorizedExecutor(
        address caller,
        bytes32 /* mode */,
        bytes calldata /* executionData */
    ) internal view virtual override returns (bool) {
        return
            caller == address(entryPoint()) ||
            caller == address(this) ||
            hasRole(DEFAULT_ADMIN_ROLE, caller);
    }

    function _rawSignatureValidation(
        bytes32 hash,
        bytes calldata signature
    ) internal view virtual override returns (bool) {
        (address recovered, ECDSA.RecoverError err, ) = ECDSA.tryRecover(
            hash,
            signature
        );

        return err == ECDSA.RecoverError.NoError;
    }

    // function isValidSignature(
    //     bytes32 hash,
    //     bytes calldata signature
    // ) public view returns (bytes4) {
    //     return
    //         _rawSignatureValidation(hash, signature)
    //             ? IERC1271.isValidSignature.selector
    //             : bytes4(0xffffffff);
    // }
}
