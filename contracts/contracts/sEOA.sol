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
    error ExecutionFailed(bytes reason);
    error ZeroAddress();

    struct ExecuteInput {
        address target;
        bytes payload;
        uint256 value;
        bytes32 salt;
        uint256 deadline;
        bytes signature;
    }

    bytes32 private constant EXECUTE_TYPEHASH =
        keccak256(
            "Execute(address target,bytes32 payloadHash,uint256 value,bytes32 salt,uint256 deadline)"
        );

    mapping(bytes32 => bool) public usedSalts;

    constructor() EIP712("sEOA", "1") {}

    function execute(
        ExecuteInput calldata input
    ) public payable returns (bool success, bytes memory returnData) {
        if (block.timestamp > input.deadline) revert Expired();
        if (usedSalts[input.salt]) revert AlreadyUsed();

        bytes32 digest = buildDigest(
            input.target,
            input.payload,
            input.value,
            input.salt,
            input.deadline
        );

        address recovered = digest.recover(input.signature);
        if (recovered != address(this)) revert InvalidSignature();

        usedSalts[input.salt] = true;

        (success, returnData) = input.target.call{value: input.value}(
            input.payload
        );

        emit Executed(input.salt, msg.sender, success);
        if (!success) revert ExecutionFailed(returnData);
    }

    function executeBatch(ExecuteInput[] calldata inputs) external payable {
        uint256 len = inputs.length;

        for (uint256 i = 0; i < len; i++) {
            execute(inputs[i]);
        }
    }

    function buildDigest(
        address target,
        bytes calldata payload,
        uint256 value,
        bytes32 salt,
        uint256 deadline
    ) public view returns (bytes32) {
        return
            _hashTypedDataV4(
                keccak256(
                    abi.encode(
                        EXECUTE_TYPEHASH,
                        target,
                        keccak256(payload),
                        value,
                        salt,
                        deadline
                    )
                )
            );
    }

    function domainSeparator() external view returns (bytes32) {
        return _domainSeparatorV4();
    }

    receive() external payable {}
}
