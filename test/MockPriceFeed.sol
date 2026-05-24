// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {IPriceFeed} from "../src/interfaces/IPriceFeed.sol";

/// @notice Test-only IPriceFeed implementation. Mutable price + publishedAt
///         so tests can drive scenarios (price ticks, stale prices, oracle
///         outages). NOT for production. Production deployments wire a
///         PythPriceFeed adapter or equivalent.
contract MockPriceFeed is IPriceFeed {
    uint256 internal _price;
    uint256 internal _publishedAt;

    constructor(uint256 initialPrice) {
        _price = initialPrice;
        _publishedAt = block.timestamp;
    }

    function setPrice(uint256 price) external {
        _price = price;
        _publishedAt = block.timestamp;
    }

    function setPriceAt(uint256 price, uint256 publishedAt) external {
        _price = price;
        _publishedAt = publishedAt;
    }

    function latestPrice() external view override returns (uint256 price, uint256 publishedAt) {
        return (_price, _publishedAt);
    }
}
