// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/account/Account.sol";
import "@openzeppelin/contracts/token/ERC721/utils/ERC721Holder.sol";
import "@openzeppelin/contracts/token/ERC1155/utils/ERC1155Holder.sol";
import "@openzeppelin/contracts/account/extensions/draft-ERC7821.sol";
import "@openzeppelin/contracts/utils/cryptography/signers/SignerERC7702.sol";
import "./WhitelistSigner.sol";

contract sEOA is Account, ERC721Holder, ERC1155Holder, WhitelistSigner {
    using ECDSA for bytes32;

    modifier onlyOwner() {
        require(
            msg.sender == address(this),
            "sEOA: Msg.sender is not the owner"
        );
        _;
    }

    function entryPoint()
        public
        pure
        override(Account, WhitelistSigner)
        returns (IEntryPoint)
    {
        return ERC4337Utils.ENTRYPOINT_V07;
    }

    function setAllowedSigners(
        address[] calldata who,
        bool[] calldata allowed
    ) external onlyOwner {
        require(who.length == allowed.length, "sEOA: Args length mismatch");
        for (uint256 i = 0; i < who.length; i++) {
            _setAllowedSigner(who[i], allowed[i]);
        }
    }

    function executeBatchPayment(
        address token,
        address from,
        address[] calldata to,
        uint256[] calldata values
    ) external onlyOwner {
        require(to.length == values.length, "len mismatch");
        for (uint256 i = 0; i < to.length; i++) {
            (bool ok, bytes memory res) = token.call(
                abi.encodeWithSelector(
                    bytes4(keccak256("transferFrom(address,address,uint256)")),
                    from,
                    to[i],
                    values[i]
                )
            );
            require(
                ok && (res.length == 0 || abi.decode(res, (bool))),
                "transferFrom failed"
            );
        }
    }
}
