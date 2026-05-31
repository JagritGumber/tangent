// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {ISettlement} from "./interfaces/ISettlement.sol";
import {IMarketRegistry} from "./interfaces/IMarketRegistry.sol";
import {IUSDCVault} from "./interfaces/IUSDCVault.sol";

/// @title  LiquidationKeeper
/// @notice v0.6 minimal liquidation entry point. Anyone can close an
///         underwater isolated-margin position at the current mark price.
///         Liquidator bounty and insurance-fund routing are deferred.
contract LiquidationKeeper {
    uint256 internal constant USDC_SCALE = 1e20; // size(1e18) * price(1e8) -> USDC(1e6)

    ISettlement public immutable settlement;
    IMarketRegistry public immutable markets;
    IUSDCVault public immutable vault;

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

    constructor(address _settlement, address _markets, address _vault) {
        if (_settlement == address(0) || _markets == address(0) || _vault == address(0)) revert ZeroAddress();
        settlement = ISettlement(_settlement);
        markets = IMarketRegistry(_markets);
        vault = IUSDCVault(_vault);
    }

    function liquidate(uint256 accountId, uint256 marketId) external {
        uint256 price = markets.markPrice(marketId);
        (bool liquidatable, int256 equity, uint256 maintenanceMargin) =
            _liquidationState(accountId, marketId, price);
        if (!liquidatable) revert NotLiquidatable(accountId, marketId, equity, maintenanceMargin);

        int256 pnl = settlement.forceClose(accountId, marketId, price);
        emit Liquidated(accountId, marketId, msg.sender, price, pnl);
    }

    function isLiquidatable(uint256 accountId, uint256 marketId) external view returns (bool) {
        uint256 price = markets.markPrice(marketId);
        (bool liquidatable,,) = _liquidationState(accountId, marketId, price);
        return liquidatable;
    }

    function liquidationState(uint256 accountId, uint256 marketId)
        external
        view
        returns (bool liquidatable, int256 equity, uint256 maintenanceMargin)
    {
        uint256 price = markets.markPrice(marketId);
        return _liquidationState(accountId, marketId, price);
    }

    function _liquidationState(uint256 accountId, uint256 marketId, uint256 price)
        internal
        view
        returns (bool liquidatable, int256 equity, uint256 maintenanceMargin)
    {
        ISettlement.Position memory p = settlement.positionOf(accountId, marketId);
        if (p.size == 0) return (false, 0, 0);

        IMarketRegistry.Market memory market = markets.market(marketId);
        uint256 size = _abs(p.size);
        int256 unrealized = _realizedPnl(p.size, size, p.entryPrice, price);

        equity = _toInt256(vault.totalBalanceOf(accountId)) + unrealized;
        maintenanceMargin = size * price / USDC_SCALE * market.maintMarginBps / 10_000;
        liquidatable = equity < _toInt256(maintenanceMargin);
    }

    function _realizedPnl(int256 currentSize, uint256 closeSize, uint256 entryPrice, uint256 exitPrice)
        internal
        pure
        returns (int256)
    {
        if (currentSize > 0) {
            return _pricePnl(exitPrice, entryPrice, closeSize);
        }
        return _pricePnl(entryPrice, exitPrice, closeSize);
    }

    function _pricePnl(uint256 gainPrice, uint256 lossPrice, uint256 size) internal pure returns (int256) {
        if (gainPrice >= lossPrice) {
            return _toInt256(size * (gainPrice - lossPrice) / USDC_SCALE);
        }
        return -_toInt256(size * (lossPrice - gainPrice) / USDC_SCALE);
    }

    function _abs(int256 value) internal pure returns (uint256) {
        // forge-lint: disable-next-line(unsafe-typecast)
        return value < 0 ? uint256(-value) : uint256(value);
    }

    function _toInt256(uint256 value) internal pure returns (int256) {
        if (value > uint256(type(int256).max)) revert Int256Overflow(value);
        // forge-lint: disable-next-line(unsafe-typecast)
        return int256(value);
    }
}
