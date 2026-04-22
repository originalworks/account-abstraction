// SPDX-License-Identifier: MIT
import "@originalworks/protocol-core-contracts/interfaces/IDdexSequencer.sol";

pragma solidity ^0.8.24;

contract FakeDdexSequencer is IDdexSequencer {
    function submitNewBlob(
        bytes32 _imageId,
        bytes memory _commitment,
        bytes32 _blobSha2
    ) public {
        bytes32 newBlobhash;
        assembly {
            newBlobhash := blobhash(0)
        }
        require(
            newBlobhash != bytes32(0),
            "DdexSequencer: Blob not found in tx"
        );
    }

    function submitNewBlobWithIndex(
        bytes32 _imageId,
        bytes memory _commitment,
        bytes32 _blobSha2,
        uint256 blobIndex
    ) public {
        bytes32 newBlobhash;
        assembly {
            newBlobhash := blobhash(blobIndex)
        }
        require(
            newBlobhash != bytes32(0),
            "DdexSequencer: Blob not found in tx"
        );
    }
}
