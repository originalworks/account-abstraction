// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/utils/cryptography/EIP712.sol";

contract sEOA is EIP712 {
    using ECDSA for bytes32;

    event Executed(
        bytes32 indexed salt,
        address indexed submitter,
        bool success
    );

    error Expired();
    error AlreadyUsed();
    error InvalidSignature();
    error InvalidBatchInput();
    error ExecutionFailed(bytes reason);
    error ZeroAddress();

    bytes32 private constant EXECUTE_TYPEHASH =
        keccak256(
            "Execute(address target,bytes32 payloadHash,bytes32 salt,uint256 deadline)"
        );

    mapping(bytes32 => bool) public usedSalts;

    constructor() EIP712("sEOA", "1") {}

    function execute(
        address target,
        bytes calldata payload,
        bytes32 salt,
        uint256 deadline,
        bytes calldata signature
    ) public returns (bool success, bytes memory returnData) {
        if (block.timestamp > deadline) revert Expired();
        if (usedSalts[salt]) revert AlreadyUsed();

        bytes32 digest = _hashTypedDataV4(
            keccak256(
                abi.encode(
                    EXECUTE_TYPEHASH,
                    target,
                    keccak256(payload),
                    salt,
                    deadline
                )
            )
        );

        address recovered = digest.recover(signature);
        if (recovered != address(this)) revert InvalidSignature();

        usedSalts[salt] = true;

        (success, returnData) = target.call(payload);

        emit Executed(salt, msg.sender, success);
        if (!success) revert ExecutionFailed(returnData);
    }

    function executeBatch(
        address[] calldata targets,
        bytes[] calldata payloads,
        bytes32[] calldata salts,
        uint256[] calldata deadlines,
        bytes[] calldata signatures
    ) external {
        uint256 len = payloads.length;
        if (
            salts.length != len ||
            deadlines.length != len ||
            signatures.length != len
        ) {
            revert InvalidBatchInput();
        }

        for (uint256 i = 0; i < len; i++) {
            execute(
                targets[i],
                payloads[i],
                salts[i],
                deadlines[i],
                signatures[i]
            );
        }
    }

    function buildDigest(
        address target,
        bytes calldata payload,
        bytes32 salt,
        uint256 deadline
    ) external view returns (bytes32) {
        return
            _hashTypedDataV4(
                keccak256(
                    abi.encode(
                        EXECUTE_TYPEHASH,
                        target,
                        keccak256(payload),
                        salt,
                        deadline
                    )
                )
            );
    }

    function domainSeparator() external view returns (bytes32) {
        return _domainSeparatorV4();
    }
}
