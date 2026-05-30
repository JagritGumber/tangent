// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {IAccountManager} from "./interfaces/IAccountManager.sol";
import {IMarketRegistry} from "./interfaces/IMarketRegistry.sol";
import {IOrderBook} from "./interfaces/IOrderBook.sol";
import {ISettlement} from "./interfaces/ISettlement.sol";
import {OrderTypes} from "./types/OrderTypes.sol";

/// @title  OrderBook
/// @notice v0.4 Tangent order book. Accepts EIP-712 signed orders, keeps
///         resting liquidity on-chain, and performs deterministic batched
///         matching through permissionless tick().
contract OrderBook is IOrderBook {
    uint256 public constant MAX_LIVE_ORDERS = 256;

    IAccountManager public immutable accountManager;
    IMarketRegistry public immutable marketRegistry;
    address public immutable settlementBinder;
    ISettlement public settlement;
    bytes32 public immutable DOMAIN_SEPARATOR;

    struct RestingOrder {
        OrderTypes.Order order;
        uint256 remaining;
        uint256 sequence;
        bool exists;
        bool live;
    }

    mapping(bytes32 => RestingOrder) private _orders;
    mapping(uint256 => uint256) public lastNonce;
    bytes32[] private _orderHashes;
    uint256 private _nextSequence;
    uint256 public liveOrderCount;

    error ZeroAddress();
    error ExpiredOrder(uint256 expiry);
    error InvalidSignature();
    error InvalidSignatureLength(uint256 length);
    error StaleNonce(uint256 accountId, uint256 nonce, uint256 last);
    error UnknownOrder(bytes32 orderHash);
    error NotAccountOwner(uint256 accountId, address caller);
    error DuplicateOrder(bytes32 orderHash);
    error PausedMarket(uint256 marketId);
    error InvalidTick(uint256 limitPrice, uint256 tickSize);
    error InvalidLot(uint256 size, uint256 lotSize);
    error ZeroSize();
    error SettlementNotBound();
    error OnlySettlementBinder(address caller, address binder);
    error SettlementAlreadyBound(address current);
    error TooManyLiveOrders(uint256 liveOrderCount, uint256 maxLiveOrders);

    constructor(address _accountManager, address _marketRegistry) {
        if (_accountManager == address(0) || _marketRegistry == address(0)) revert ZeroAddress();
        accountManager = IAccountManager(_accountManager);
        marketRegistry = IMarketRegistry(_marketRegistry);
        settlementBinder = msg.sender;
        DOMAIN_SEPARATOR = OrderTypes.domainSeparator(block.chainid, address(this));
    }

    /// @notice One-shot settlement wiring. The book starts unbound so the
    ///         SettlementEngine can safely be deployed with this book address.
    function bindSettlementEngine(address engine) external {
        if (msg.sender != settlementBinder) revert OnlySettlementBinder(msg.sender, settlementBinder);
        if (engine == address(0)) revert ZeroAddress();
        if (address(settlement) != address(0)) revert SettlementAlreadyBound(address(settlement));
        settlement = ISettlement(engine);
    }

    /// @inheritdoc IOrderBook
    function submitOrder(OrderTypes.Order calldata order, bytes calldata signature) external override {
        if (order.expiry <= block.timestamp) revert ExpiredOrder(order.expiry);
        if (order.size == 0) revert ZeroSize();
        if (liveOrderCount >= MAX_LIVE_ORDERS) revert TooManyLiveOrders(liveOrderCount, MAX_LIVE_ORDERS);

        IMarketRegistry.Market memory m = marketRegistry.market(order.marketId);
        if (m.paused && !order.reduceOnly) revert PausedMarket(order.marketId);
        if (order.limitPrice % m.tickSize != 0) revert InvalidTick(order.limitPrice, m.tickSize);
        if (order.size % m.lotSize != 0) revert InvalidLot(order.size, m.lotSize);

        uint256 previousNonce = lastNonce[order.accountId];
        if (order.nonce <= previousNonce) revert StaleNonce(order.accountId, order.nonce, previousNonce);

        address owner = accountManager.ownerOf(order.accountId);
        bytes32 digest = OrderTypes.digest(order, DOMAIN_SEPARATOR);
        if (_recover(digest, signature) != owner) revert InvalidSignature();

        bytes32 orderHash = OrderTypes.hash(order);
        if (_orders[orderHash].exists) revert DuplicateOrder(orderHash);

        lastNonce[order.accountId] = order.nonce;
        unchecked {
            _nextSequence++;
        }

        RestingOrder storage resting = _orders[orderHash];
        resting.order.accountId = order.accountId;
        resting.order.marketId = order.marketId;
        resting.order.isBuy = order.isBuy;
        resting.order.limitPrice = order.limitPrice;
        resting.order.size = order.size;
        resting.order.nonce = order.nonce;
        resting.order.expiry = order.expiry;
        resting.order.reduceOnly = order.reduceOnly;
        resting.remaining = order.size;
        resting.sequence = _nextSequence;
        resting.exists = true;
        resting.live = true;
        _orderHashes.push(orderHash);
        liveOrderCount++;

        emit OrderSubmitted(
            orderHash, order.accountId, order.marketId, order.isBuy, order.limitPrice, order.size
        );
    }

    /// @inheritdoc IOrderBook
    function cancelOrder(bytes32 orderHash) external override {
        RestingOrder storage resting = _orders[orderHash];
        if (!resting.exists) revert UnknownOrder(orderHash);

        address owner = accountManager.ownerOf(resting.order.accountId);
        if (msg.sender != owner) revert NotAccountOwner(resting.order.accountId, msg.sender);
        if (!resting.live) return;

        resting.live = false;
        resting.remaining = 0;
        liveOrderCount--;
        emit OrderCancelled(orderHash, resting.order.accountId, "owner");
    }

    /// @inheritdoc IOrderBook
    function tick() external override {
        if (address(settlement) == address(0)) revert SettlementNotBound();

        uint256 maxMatches = _orderHashes.length;
        ISettlement.Match[] memory matches = new ISettlement.Match[](maxMatches);
        uint256 count;

        _expireOrders();

        while (count < maxMatches) {
            (bytes32 buyHash, bytes32 sellHash) = _bestCrossedPair();
            if (buyHash == bytes32(0)) break;

            RestingOrder storage buy = _orders[buyHash];
            RestingOrder storage sell = _orders[sellHash];
            uint256 fillSize = buy.remaining < sell.remaining ? buy.remaining : sell.remaining;
            uint256 price = buy.sequence <= sell.sequence ? buy.order.limitPrice : sell.order.limitPrice;

            buy.remaining -= fillSize;
            sell.remaining -= fillSize;
            if (buy.remaining == 0) _markNotLive(buy);
            if (sell.remaining == 0) _markNotLive(sell);

            matches[count] = ISettlement.Match({
                buyOrderHash: buyHash,
                sellOrderHash: sellHash,
                buyAccountId: buy.order.accountId,
                sellAccountId: sell.order.accountId,
                marketId: buy.order.marketId,
                size: fillSize,
                price: price
            });
            unchecked {
                count++;
            }

            emit Matched(buyHash, sellHash, buy.order.marketId, fillSize, price);
        }

        if (count != 0) {
            ISettlement.Match[] memory compact = new ISettlement.Match[](count);
            for (uint256 i = 0; i < count; i++) {
                compact[i] = matches[i];
            }
            settlement.settleBatch(compact);
        }
    }

    /// @inheritdoc IOrderBook
    function isLive(bytes32 orderHash) external view override returns (bool) {
        return _orders[orderHash].live;
    }

    /// @inheritdoc IOrderBook
    function orderOf(bytes32 orderHash)
        external
        view
        override
        returns (OrderTypes.Order memory order, bool exists)
    {
        RestingOrder storage resting = _orders[orderHash];
        return (resting.order, resting.exists);
    }

    function orderCount() external view returns (uint256) {
        return _orderHashes.length;
    }

    function remaining(bytes32 orderHash) external view returns (uint256) {
        return _orders[orderHash].remaining;
    }

    function _expireOrders() internal {
        for (uint256 i = 0; i < _orderHashes.length; i++) {
            bytes32 orderHash = _orderHashes[i];
            RestingOrder storage resting = _orders[orderHash];
            if (resting.live && resting.order.expiry <= block.timestamp) {
                resting.live = false;
                resting.remaining = 0;
                liveOrderCount--;
                emit OrderCancelled(orderHash, resting.order.accountId, "expired");
            }
        }
    }

    function _markNotLive(RestingOrder storage resting) internal {
        if (!resting.live) return;
        resting.live = false;
        liveOrderCount--;
    }

    function _bestCrossedPair() internal view returns (bytes32 buyHash, bytes32 sellHash) {
        for (uint256 i = 0; i < _orderHashes.length; i++) {
            bytes32 candidateBuyHash = _orderHashes[i];
            RestingOrder storage buy = _orders[candidateBuyHash];
            if (!buy.live || !buy.order.isBuy) continue;

            for (uint256 j = 0; j < _orderHashes.length; j++) {
                bytes32 candidateSellHash = _orderHashes[j];
                RestingOrder storage sell = _orders[candidateSellHash];
                if (!sell.live || sell.order.isBuy) continue;
                if (buy.order.marketId != sell.order.marketId) continue;
                if (marketRegistry.market(buy.order.marketId).paused) continue;
                if (buy.order.accountId == sell.order.accountId) continue;
                if (buy.order.limitPrice < sell.order.limitPrice) continue;

                if (
                    buyHash == bytes32(0)
                        || _betterPair(candidateBuyHash, candidateSellHash, buyHash, sellHash)
                ) {
                    buyHash = candidateBuyHash;
                    sellHash = candidateSellHash;
                }
            }
        }
    }

    function _betterPair(
        bytes32 leftBuyHash,
        bytes32 leftSellHash,
        bytes32 rightBuyHash,
        bytes32 rightSellHash
    ) internal view returns (bool) {
        RestingOrder storage leftBuy = _orders[leftBuyHash];
        RestingOrder storage leftSell = _orders[leftSellHash];
        RestingOrder storage rightBuy = _orders[rightBuyHash];
        RestingOrder storage rightSell = _orders[rightSellHash];

        if (leftBuy.order.marketId != rightBuy.order.marketId) {
            return leftBuy.order.marketId < rightBuy.order.marketId;
        }
        if (leftBuy.order.limitPrice != rightBuy.order.limitPrice) {
            return leftBuy.order.limitPrice > rightBuy.order.limitPrice;
        }
        if (leftSell.order.limitPrice != rightSell.order.limitPrice) {
            return leftSell.order.limitPrice < rightSell.order.limitPrice;
        }
        if (leftBuy.sequence != rightBuy.sequence) return leftBuy.sequence < rightBuy.sequence;
        return leftSell.sequence < rightSell.sequence;
    }

    function _recover(bytes32 digest, bytes calldata signature) internal pure returns (address) {
        if (signature.length != 65) revert InvalidSignatureLength(signature.length);

        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            r := calldataload(signature.offset)
            s := calldataload(add(signature.offset, 0x20))
            v := byte(0, calldataload(add(signature.offset, 0x40)))
        }

        if (v < 27) v += 27;
        return ecrecover(digest, v, r, s);
    }
}
