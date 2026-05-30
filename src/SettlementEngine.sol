// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {ISettlement} from "./interfaces/ISettlement.sol";
import {IOrderBook} from "./interfaces/IOrderBook.sol";
import {IMarketRegistry} from "./interfaces/IMarketRegistry.sol";
import {IUSDCVault} from "./interfaces/IUSDCVault.sol";
import {OrderTypes} from "./types/OrderTypes.sol";

/// @title  SettlementEngine
/// @notice v0.5 minimal perp settlement core. Consumes matches emitted by the
///         bound OrderBook, updates account positions, locks/releases margin,
///         and realizes PnL on reductions and closes.
contract SettlementEngine is ISettlement {
    uint256 internal constant USDC_SCALE = 1e20; // size(1e18) * price(1e8) -> USDC(1e6)

    IOrderBook public immutable orderBook;
    IUSDCVault public immutable vault;
    IMarketRegistry public immutable markets;

    struct Position {
        int256 size;
        uint256 entryPrice;
        uint256 lockedMargin;
    }

    mapping(uint256 accountId => mapping(uint256 marketId => Position)) private _positions;

    event PositionUpdated(
        uint256 indexed accountId,
        uint256 indexed marketId,
        int256 size,
        uint256 entryPrice,
        uint256 lockedMargin
    );

    error ZeroAddress();
    error OnlyOrderBook(address caller);
    error InvalidMatch();
    error UnknownOrder(bytes32 orderHash);
    error OrderMismatch(bytes32 orderHash);
    error PausedMarket(uint256 marketId);
    error ReduceOnlyViolation(uint256 accountId, uint256 marketId);
    error Int256Overflow(uint256 value);

    constructor(address _orderBook, address _vault, address _markets) {
        if (_orderBook == address(0) || _vault == address(0) || _markets == address(0)) {
            revert ZeroAddress();
        }
        orderBook = IOrderBook(_orderBook);
        vault = IUSDCVault(_vault);
        markets = IMarketRegistry(_markets);
    }

    /// @inheritdoc ISettlement
    function settleBatch(Match[] calldata matches) external override {
        if (msg.sender != address(orderBook)) revert OnlyOrderBook(msg.sender);

        for (uint256 i = 0; i < matches.length; i++) {
            _settle(matches[i]);
        }
    }

    function positionOf(uint256 accountId, uint256 marketId) external view returns (Position memory) {
        return _positions[accountId][marketId];
    }

    function _settle(Match calldata m) internal {
        if (
            m.buyOrderHash == bytes32(0) || m.sellOrderHash == bytes32(0) || m.buyAccountId == m.sellAccountId
                || m.marketId == 0 || m.size == 0 || m.price == 0
        ) {
            revert InvalidMatch();
        }

        IMarketRegistry.Market memory market = markets.market(m.marketId);
        if (market.paused) revert PausedMarket(m.marketId);

        OrderTypes.Order memory buy = _validatedOrder(m.buyOrderHash, m.buyAccountId, m.marketId, true);
        OrderTypes.Order memory sell = _validatedOrder(m.sellOrderHash, m.sellAccountId, m.marketId, false);

        int256 signedSize = _toInt256(m.size);
        if (buy.reduceOnly) _requireReducesOnly(m.buyAccountId, m.marketId, signedSize);
        if (sell.reduceOnly) _requireReducesOnly(m.sellAccountId, m.marketId, -signedSize);

        _applyFill(m.buyAccountId, m.marketId, signedSize, m.price, market.initialMarginBps);
        _applyFill(m.sellAccountId, m.marketId, -signedSize, m.price, market.initialMarginBps);

        emit Settled(m.buyOrderHash, m.sellOrderHash, m.marketId, m.size, m.price);
    }

    function _validatedOrder(bytes32 orderHash, uint256 accountId, uint256 marketId, bool isBuy)
        internal
        view
        returns (OrderTypes.Order memory order)
    {
        bool exists;
        (order, exists) = orderBook.orderOf(orderHash);
        if (!exists) revert UnknownOrder(orderHash);
        if (order.accountId != accountId || order.marketId != marketId || order.isBuy != isBuy) {
            revert OrderMismatch(orderHash);
        }
    }

    function _requireReducesOnly(uint256 accountId, uint256 marketId, int256 delta) internal view {
        int256 current = _positions[accountId][marketId].size;
        if (current == 0 || _sameSign(current, delta)) {
            revert ReduceOnlyViolation(accountId, marketId);
        }

        uint256 currentAbs = _abs(current);
        uint256 deltaAbs = _abs(delta);
        if (deltaAbs > currentAbs) revert ReduceOnlyViolation(accountId, marketId);
    }

    function _applyFill(
        uint256 accountId,
        uint256 marketId,
        int256 delta,
        uint256 price,
        uint16 initialMarginBps
    ) internal {
        Position storage p = _positions[accountId][marketId];

        if (p.size == 0 || _sameSign(p.size, delta)) {
            _increase(p, accountId, marketId, delta, price, initialMarginBps);
            return;
        }

        uint256 currentAbs = _abs(p.size);
        uint256 deltaAbs = _abs(delta);
        uint256 closeSize = deltaAbs < currentAbs ? deltaAbs : currentAbs;

        int256 pnl = _realizedPnl(p.size, closeSize, p.entryPrice, price);
        uint256 released = p.lockedMargin * closeSize / currentAbs;

        if (released != 0) vault.releaseMargin(accountId, released);
        if (pnl != 0) vault.applyPnL(accountId, pnl);

        p.lockedMargin -= released;

        if (deltaAbs < currentAbs) {
            p.size += delta;
        } else if (deltaAbs == currentAbs) {
            p.size = 0;
            p.entryPrice = 0;
            p.lockedMargin = 0;
        } else {
            int256 residualAbs = _toInt256(deltaAbs - currentAbs);
            int256 residual = delta > 0 ? residualAbs : -residualAbs;
            p.size = 0;
            p.entryPrice = 0;
            p.lockedMargin = 0;
            _increase(p, accountId, marketId, residual, price, initialMarginBps);
        }

        emit PositionUpdated(accountId, marketId, p.size, p.entryPrice, p.lockedMargin);
    }

    function _increase(
        Position storage p,
        uint256 accountId,
        uint256 marketId,
        int256 delta,
        uint256 price,
        uint16 initialMarginBps
    ) internal {
        uint256 oldAbs = _abs(p.size);
        uint256 deltaAbs = _abs(delta);
        uint256 addedMargin = _initialMargin(deltaAbs, price, initialMarginBps);

        vault.lockMargin(accountId, addedMargin);

        if (oldAbs == 0) {
            p.entryPrice = price;
        } else {
            p.entryPrice = ((p.entryPrice * oldAbs) + (price * deltaAbs)) / (oldAbs + deltaAbs);
        }

        p.size += delta;
        p.lockedMargin += addedMargin;
        emit PositionUpdated(accountId, marketId, p.size, p.entryPrice, p.lockedMargin);
    }

    function _initialMargin(uint256 size, uint256 price, uint16 initialMarginBps)
        internal
        pure
        returns (uint256)
    {
        uint256 notional = size * price / USDC_SCALE;
        return notional * initialMarginBps / 10_000;
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

    function _sameSign(int256 a, int256 b) internal pure returns (bool) {
        return (a > 0 && b > 0) || (a < 0 && b < 0);
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
