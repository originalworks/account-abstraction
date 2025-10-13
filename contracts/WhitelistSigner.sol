// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {AbstractSigner} from "@openzeppelin/contracts/utils/cryptography/signers/AbstractSigner.sol";
import {IEntryPoint} from "@openzeppelin/contracts/interfaces/draft-IERC4337.sol";
import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import {IERC1271} from "@openzeppelin/contracts/interfaces/IERC1271.sol";
import "@openzeppelin/contracts/account/extensions/draft-ERC7821.sol";

abstract contract WhitelistSigner is AbstractSigner, ERC7821 {
    event SignersWhitelistUpdated(address signer, bool allowed);

    mapping(address => bool) public signerWhitelist;

    function entryPoint() public view virtual returns (IEntryPoint);

    function _setAllowedSigner(address signer, bool allowed) internal virtual {
        signerWhitelist[signer] = allowed;
        emit SignersWhitelistUpdated(signer, allowed);
    }

    function isSignerWhitelisted(address signer) public view returns (bool) {
        return signerWhitelist[signer];
    }

    function _erc7821AuthorizedExecutor(
        address caller,
        bytes32 /* mode */,
        bytes calldata /* executionData */
    ) internal view virtual override returns (bool) {
        return
            caller == address(entryPoint()) ||
            caller == address(this) ||
            isSignerWhitelisted(caller);
    }

    function _rawSignatureValidation(
        bytes32 hash,
        bytes calldata signature
    ) internal view virtual override returns (bool) {
        (address recovered, ECDSA.RecoverError err, ) = ECDSA.tryRecover(
            hash,
            signature
        );

        return
            (address(this) == recovered || isSignerWhitelisted(recovered)) &&
            err == ECDSA.RecoverError.NoError;
    }

    function isValidSignature(
        bytes32 hash,
        bytes calldata signature
    ) public view returns (bytes4) {
        return
            _rawSignatureValidation(hash, signature)
                ? IERC1271.isValidSignature.selector
                : bytes4(0xffffffff);
    }
}
