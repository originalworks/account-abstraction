// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract MockExchange {
    address public _erc20Address;

    constructor(address erc20Address) {
        _erc20Address = erc20Address;
    }

    function swapToEth(uint256 erc20Value) public {
        ERC20(_erc20Address).transferFrom(
            msg.sender,
            address(this),
            erc20Value
        );
        (bool sent, ) = payable(msg.sender).call{value: 1}("");
        require(sent == true, "Transfer failed");
    }

    receive() external payable {}
}
