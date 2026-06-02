// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {ISettlement} from "./interfaces/ISettlement.sol";
import {IMarketRegistry} from "./interfaces/IMarketRegistry.sol";

/// @title  LiquidationKeeper
/// @notice v0.6 minimal liquidation entry point. Anyone can close an
///         underwater account-margin position at the current mark price.
///         Liquidator bounty and insurance-fund routing are deferred.
contract LiquidationKeeper {
    ISettlement public immutable settlement;
    IMarketRegistry public immutable markets;

    event Liquidated(
        uint256 indexed accountId,
        uint256 indexed marketId,
        address indexed liquidator,
        uint256 markPrice,
        int256 pnl
    );

    error ZeroAddress();
    error NotLiquidatable(uint256 accountId, uint256 marketId, int256 equity, uint256 maintenanceMargin);
    error Int256Overflow(uint256 value);

    constructor(address _settlement, address _markets) {
        if (_settlement == address(0) || _markets == address(0)) {
            revert ZeroAddress();
        }
        settlement = ISettlement(_settlement);
        markets = IMarketRegistry(_markets);
    }

    function liquidate(uint256 accountId, uint256 marketId) external {
        uint256 price = markets.markPrice(marketId);
        (bool liquidatable, int256 equity, uint256 maintenanceMargin) =
            _liquidationState(accountId, marketId);
        if (!liquidatable) revert NotLiquidatable(accountId, marketId, equity, maintenanceMargin);

        int256 pnl = settlement.forceClose(accountId, marketId, price);
        emit Liquidated(accountId, marketId, msg.sender, price, pnl);
    }

    function isLiquidatable(uint256 accountId, uint256 marketId) external view returns (bool) {
        (bool liquidatable,,) = _liquidationState(accountId, marketId);
        return liquidatable;
    }

    function liquidationState(uint256 accountId, uint256 marketId)
        external
        view
        returns (bool liquidatable, int256 equity, uint256 maintenanceMargin)
    {
        return _liquidationState(accountId, marketId);
    }

    function _liquidationState(uint256 accountId, uint256 marketId)
        internal
        view
        returns (bool liquidatable, int256 equity, uint256 maintenanceMargin)
    {
        ISettlement.Position memory p = settlement.positionOf(accountId, marketId);
        if (p.size == 0) return (false, 0, 0);

        (equity, maintenanceMargin) = settlement.marginState(accountId);
        liquidatable = equity < _toInt256(maintenanceMargin);
    }

    function _toInt256(uint256 value) internal pure returns (int256) {
        if (value > uint256(type(int256).max)) revert Int256Overflow(value);
        // forge-lint: disable-next-line(unsafe-typecast)
        return int256(value);
    }
}
