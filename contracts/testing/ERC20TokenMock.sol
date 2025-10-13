// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract ERC20TokenMock is ERC20 {
    string public constant version = "2";
    uint8 private _decimalPoints = 6;

    constructor() ERC20("USDC", "USDC") {}

    function mintTo(address user, uint256 amount) public {
        _mint(user, amount);
    }

    function decimals() public view override returns (uint8) {
        return _decimalPoints;
    }
}
