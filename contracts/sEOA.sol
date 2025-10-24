// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/account/Account.sol";
import "@openzeppelin/contracts/token/ERC721/utils/ERC721Holder.sol";
import "@openzeppelin/contracts/token/ERC1155/utils/ERC1155Holder.sol";
import "@openzeppelin/contracts/account/extensions/draft-ERC7821.sol";
import "@openzeppelin/contracts/utils/cryptography/signers/SignerERC7702.sol";
import "./PermissionManager.sol";
import "./interfaces/IDdexSequencer.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

contract sEOA is Account, ERC721Holder, ERC1155Holder, PermissionManager {
    using ECDSA for bytes32;
    using SafeERC20 for IERC20;

    struct SubmitNewBlobInput {
        bytes32 imageId;
        bytes commitment;
        bytes32 blobSha2;
    }

    function entryPoint()
        public
        pure
        override(Account, PermissionManager)
        returns (IEntryPoint)
    {
        return ERC4337Utils.ENTRYPOINT_V07;
    }

    function submitNewBlobBatch(
        SubmitNewBlobInput[] calldata inputs,
        address ddexSequencerAddress
    ) public onlyRole(BLOB_SENDER_ROLE) {
        for (uint i = 0; i < inputs.length; i++) {
            IDdexSequencer(ddexSequencerAddress).submitNewBlob(
                inputs[i].imageId,
                inputs[i].commitment,
                inputs[i].blobSha2,
                i
            );
        }
    }

    function executeBatchPayment(
        address token,
        address from,
        address[] calldata to,
        uint256[] calldata values
    ) external onlyRole(PAYMENT_SENDER_ROLE) {
        require(to.length == values.length, "len mismatch");
        for (uint256 i = 0; i < to.length; i++) {
            IERC20(token).safeTransferFrom(from, to[i], values[i]);
        }
    }

    function supportsInterface(
        bytes4 interfaceId
    )
        public
        view
        virtual
        override(ERC1155Holder, AccessControl)
        returns (bool)
    {
        return super.supportsInterface(interfaceId);
    }
}
