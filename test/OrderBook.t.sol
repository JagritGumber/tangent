// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Test} from "forge-std/Test.sol";
import {AccountManager} from "../src/AccountManager.sol";
import {MarketRegistry} from "../src/MarketRegistry.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {ISettlement} from "../src/interfaces/ISettlement.sol";
import {IMarketRegistry} from "../src/interfaces/IMarketRegistry.sol";
import {OrderTypes} from "../src/types/OrderTypes.sol";
import {MockPriceFeed} from "./MockPriceFeed.sol";

contract MockSettlement is ISettlement {
    uint256 public batches;
    uint256 public lastBatchSize;
    Match[] private _lastMatches;

    function settleBatch(Match[] calldata matches) external override {
        delete _lastMatches;
        batches++;
        lastBatchSize = matches.length;
        for (uint256 i = 0; i < matches.length; i++) {
            _lastMatches.push(matches[i]);
            emit Settled(
                matches[i].buyOrderHash,
                matches[i].sellOrderHash,
                matches[i].marketId,
                matches[i].size,
                matches[i].price
            );
        }
    }

    function lastMatch(uint256 index) external view returns (Match memory) {
        return _lastMatches[index];
    }

    function positionOf(uint256, uint256) external pure override returns (Position memory) {
        return Position({size: 0, entryPrice: 0, lockedMargin: 0});
    }

    function marginState(uint256) external pure override returns (int256, uint256) {
        return (0, 0);
    }

    function forceClose(uint256, uint256, uint256) external pure override returns (int256) {
        return 0;
    }

    function validateWithdrawal(uint256, uint256) external pure override {}
}

contract OrderBookTest is Test {
    uint32 internal constant MAX_PRICE_AGE = 60;

    AccountManager internal accounts;
    MarketRegistry internal markets;
    OrderBook internal book;
    MockSettlement internal settlement;
    MockPriceFeed internal btcFeed;

    uint256 internal constant ALICE_PK = 0xA11CE;
    uint256 internal constant BOB_PK = 0xB0B;
    uint256 internal constant CAROL_PK = 0xCA201;

    address internal alice;
    address internal bob;
    address internal carol;

    uint256 internal aliceAccount;
    uint256 internal bobAccount;
    uint256 internal carolAccount;
    uint256 internal btcMarket;

    event OrderSubmitted(
        bytes32 indexed orderHash,
        uint256 indexed accountId,
        uint256 indexed marketId,
        bool isBuy,
        uint256 limitPrice,
        uint256 size
    );
    event OrderCancelled(bytes32 indexed orderHash, uint256 indexed accountId, string reason);
    event Matched(
        bytes32 indexed buyOrderHash,
        bytes32 indexed sellOrderHash,
        uint256 indexed marketId,
        uint256 size,
        uint256 price
    );

    function setUp() public {
        alice = vm.addr(ALICE_PK);
        bob = vm.addr(BOB_PK);
        carol = vm.addr(CAROL_PK);

        accounts = new AccountManager();
        markets = new MarketRegistry(address(this));
        settlement = new MockSettlement();
        book = new OrderBook(address(accounts), address(markets));
        book.bindSettlementEngine(address(settlement));
        btcFeed = new MockPriceFeed(65_000_00000000);
        btcMarket = markets.registerMarket(_btcMarket(false));

        vm.prank(alice);
        aliceAccount = accounts.registerAccount();
        vm.prank(bob);
        bobAccount = accounts.registerAccount();
        vm.prank(carol);
        carolAccount = accounts.registerAccount();
    }

    function test_submitOrder_acceptsValidSignatureAndEmits() public {
        OrderTypes.Order memory order = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        bytes32 orderHash = OrderTypes.hash(order);

        vm.expectEmit(true, true, true, true);
        emit OrderSubmitted(orderHash, aliceAccount, btcMarket, true, 65_000_00000000, 1e18);
        book.submitOrder(order, _sign(ALICE_PK, order));

        assertTrue(book.isLive(orderHash));
        assertEq(book.remaining(orderHash), 1e18);
        assertEq(book.lastNonce(aliceAccount), 1);
        assertEq(book.orderCount(), 1);
        assertEq(book.liveOrderCount(), 1);
    }

    function test_bindSettlementEngine_revertsOnSecondBind() public {
        OrderBook fresh = new OrderBook(address(accounts), address(markets));
        fresh.bindSettlementEngine(address(settlement));
        vm.expectRevert(
            abi.encodeWithSelector(OrderBook.SettlementAlreadyBound.selector, address(settlement))
        );
        fresh.bindSettlementEngine(address(0xBEEF));
    }

    function test_submitOrder_revertsOnInvalidSignature() public {
        OrderTypes.Order memory order = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        bytes memory signature = _sign(BOB_PK, order);

        vm.expectRevert(OrderBook.InvalidSignature.selector);
        book.submitOrder(order, signature);
    }

    function test_submitOrder_revertsOnStaleNonce() public {
        OrderTypes.Order memory first = _order(aliceAccount, true, 65_000_00000000, 1e18, 7, false);
        book.submitOrder(first, _sign(ALICE_PK, first));

        OrderTypes.Order memory stale = _order(aliceAccount, true, 65_001_00000000, 1e18, 6, false);
        bytes memory signature = _sign(ALICE_PK, stale);
        vm.expectRevert(
            abi.encodeWithSelector(OrderBook.StaleNonce.selector, aliceAccount, uint256(6), uint256(7))
        );
        book.submitOrder(stale, signature);
    }

    function test_submitOrder_revertsOnPausedMarketUnlessReduceOnly() public {
        markets.setPaused(btcMarket, true);

        OrderTypes.Order memory open = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        bytes memory openSignature = _sign(ALICE_PK, open);
        vm.expectRevert(abi.encodeWithSelector(OrderBook.PausedMarket.selector, btcMarket));
        book.submitOrder(open, openSignature);

        OrderTypes.Order memory reduce = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, true);
        book.submitOrder(reduce, _sign(ALICE_PK, reduce));
        assertTrue(book.isLive(OrderTypes.hash(reduce)));
    }

    function test_submitOrder_revertsOnTickAndLotViolation() public {
        OrderTypes.Order memory badTick = _order(aliceAccount, true, 65_000_00000001, 1e18, 1, false);
        bytes memory badTickSignature = _sign(ALICE_PK, badTick);
        vm.expectRevert(
            abi.encodeWithSelector(OrderBook.InvalidTick.selector, badTick.limitPrice, uint256(100))
        );
        book.submitOrder(badTick, badTickSignature);

        OrderTypes.Order memory badLot = _order(aliceAccount, true, 65_000_00000000, 1e18 + 1, 1, false);
        bytes memory badLotSignature = _sign(ALICE_PK, badLot);
        vm.expectRevert(abi.encodeWithSelector(OrderBook.InvalidLot.selector, badLot.size, uint256(1e15)));
        book.submitOrder(badLot, badLotSignature);
    }

    function test_cancelOrder_ownerOnlyAndIdempotentAfterCancel() public {
        OrderTypes.Order memory order = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        bytes32 orderHash = OrderTypes.hash(order);
        book.submitOrder(order, _sign(ALICE_PK, order));

        vm.expectRevert(abi.encodeWithSelector(OrderBook.NotAccountOwner.selector, aliceAccount, bob));
        vm.prank(bob);
        book.cancelOrder(orderHash);

        vm.expectEmit(true, true, false, true);
        emit OrderCancelled(orderHash, aliceAccount, "owner");
        vm.prank(alice);
        book.cancelOrder(orderHash);

        assertFalse(book.isLive(orderHash));
        assertEq(book.liveOrderCount(), 0);

        vm.prank(alice);
        book.cancelOrder(orderHash);
        assertEq(book.liveOrderCount(), 0);
    }

    function test_tick_expiresOrdersBeforeMatching() public {
        OrderTypes.Order memory buy = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, 64_000_00000000, 1e18, 1, false);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));

        vm.warp(buy.expiry);
        vm.expectEmit(true, true, false, true);
        emit OrderCancelled(OrderTypes.hash(buy), aliceAccount, "expired");
        book.tick();

        assertFalse(book.isLive(OrderTypes.hash(buy)));
        assertFalse(book.isLive(OrderTypes.hash(sell)));
        assertEq(book.liveOrderCount(), 0);
        assertEq(settlement.batches(), 0);
    }

    function test_tick_matchesCrossedOrdersAndCallsSettlement() public {
        OrderTypes.Order memory buy = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, 64_900_00000000, 1e18, 1, false);
        bytes32 buyHash = OrderTypes.hash(buy);
        bytes32 sellHash = OrderTypes.hash(sell);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));

        vm.expectEmit(true, true, true, true);
        emit Matched(buyHash, sellHash, btcMarket, 1e18, buy.limitPrice);
        book.tick();

        assertFalse(book.isLive(buyHash));
        assertFalse(book.isLive(sellHash));
        assertEq(book.liveOrderCount(), 0);
        assertEq(settlement.batches(), 1);
        assertEq(settlement.lastBatchSize(), 1);

        ISettlement.Match memory m = settlement.lastMatch(0);
        assertEq(m.buyOrderHash, buyHash);
        assertEq(m.sellOrderHash, sellHash);
        assertEq(m.buyAccountId, aliceAccount);
        assertEq(m.sellAccountId, bobAccount);
        assertEq(m.marketId, btcMarket);
        assertEq(m.size, 1e18);
        assertEq(m.price, buy.limitPrice);
    }

    function test_tick_doesNothingWhenBookDoesNotCross() public {
        OrderTypes.Order memory buy = _order(aliceAccount, true, 64_000_00000000, 1e18, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, 65_000_00000000, 1e18, 1, false);
        bytes32 buyHash = OrderTypes.hash(buy);
        bytes32 sellHash = OrderTypes.hash(sell);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));

        book.tick();

        assertTrue(book.isLive(buyHash));
        assertTrue(book.isLive(sellHash));
        assertEq(book.remaining(buyHash), 1e18);
        assertEq(book.remaining(sellHash), 1e18);
        assertEq(settlement.batches(), 0);
    }

    function test_tick_doesNotMatchPausedMarketRestingOrders() public {
        OrderTypes.Order memory buy = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, 64_900_00000000, 1e18, 1, false);
        bytes32 buyHash = OrderTypes.hash(buy);
        bytes32 sellHash = OrderTypes.hash(sell);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));

        markets.setPaused(btcMarket, true);
        book.tick();

        assertTrue(book.isLive(buyHash));
        assertTrue(book.isLive(sellHash));
        assertEq(settlement.batches(), 0);
    }

    function test_tick_revertsWhenSettlementIsNotBound() public {
        OrderBook unboundBook = new OrderBook(address(accounts), address(markets));
        OrderTypes.Order memory buy = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, 64_900_00000000, 1e18, 1, false);
        bytes32 buyHash = OrderTypes.hash(buy);
        bytes32 sellHash = OrderTypes.hash(sell);

        unboundBook.submitOrder(buy, _signFor(unboundBook, ALICE_PK, buy));
        unboundBook.submitOrder(sell, _signFor(unboundBook, BOB_PK, sell));

        vm.expectRevert(OrderBook.SettlementNotBound.selector);
        unboundBook.tick();

        assertTrue(unboundBook.isLive(buyHash));
        assertTrue(unboundBook.isLive(sellHash));
        assertEq(settlement.batches(), 0);
    }

    function test_orderOf_returnsStoredMetadata() public {
        OrderTypes.Order memory order = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, true);
        bytes32 orderHash = OrderTypes.hash(order);
        book.submitOrder(order, _sign(ALICE_PK, order));

        (OrderTypes.Order memory stored, bool exists) = book.orderOf(orderHash);

        assertTrue(exists);
        assertEq(stored.accountId, order.accountId);
        assertEq(stored.marketId, order.marketId);
        assertEq(stored.isBuy, order.isBuy);
        assertEq(stored.limitPrice, order.limitPrice);
        assertEq(stored.size, order.size);
        assertEq(stored.nonce, order.nonce);
        assertEq(stored.expiry, order.expiry);
        assertEq(stored.reduceOnly, order.reduceOnly);
    }

    function test_tick_partialFillLeavesRemainderLive() public {
        OrderTypes.Order memory buy = _order(aliceAccount, true, 65_000_00000000, 2e18, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, 64_900_00000000, 1e18, 1, false);
        bytes32 buyHash = OrderTypes.hash(buy);
        bytes32 sellHash = OrderTypes.hash(sell);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));

        book.tick();

        assertTrue(book.isLive(buyHash));
        assertFalse(book.isLive(sellHash));
        assertEq(book.remaining(buyHash), 1e18);
        assertEq(book.liveOrderCount(), 1);
        assertEq(settlement.lastBatchSize(), 1);
    }

    function test_tick_usesBestPriceBeforeTimePriority() public {
        OrderTypes.Order memory olderBuy = _order(aliceAccount, true, 65_000_00000000, 1e18, 1, false);
        OrderTypes.Order memory betterBuy = _order(carolAccount, true, 65_100_00000000, 1e18, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, 64_900_00000000, 1e18, 1, false);
        book.submitOrder(olderBuy, _sign(ALICE_PK, olderBuy));
        book.submitOrder(betterBuy, _sign(CAROL_PK, betterBuy));
        book.submitOrder(sell, _sign(BOB_PK, sell));

        book.tick();

        ISettlement.Match memory m = settlement.lastMatch(0);
        assertEq(m.buyOrderHash, OrderTypes.hash(betterBuy));
        assertEq(m.sellOrderHash, OrderTypes.hash(sell));
        assertTrue(book.isLive(OrderTypes.hash(olderBuy)));
    }

    function test_tick_skipsSelfTradeAndMatchesExternalOrder() public {
        OrderTypes.Order memory selfBuy = _order(aliceAccount, true, 65_200_00000000, 1e18, 1, false);
        OrderTypes.Order memory selfSell = _order(aliceAccount, false, 64_800_00000000, 1e18, 2, false);
        OrderTypes.Order memory externalBuy = _order(carolAccount, true, 65_100_00000000, 1e18, 1, false);
        book.submitOrder(selfBuy, _sign(ALICE_PK, selfBuy));
        book.submitOrder(selfSell, _sign(ALICE_PK, selfSell));
        book.submitOrder(externalBuy, _sign(CAROL_PK, externalBuy));

        book.tick();

        ISettlement.Match memory m = settlement.lastMatch(0);
        assertEq(m.buyOrderHash, OrderTypes.hash(externalBuy));
        assertEq(m.sellOrderHash, OrderTypes.hash(selfSell));
        assertTrue(book.isLive(OrderTypes.hash(selfBuy)));
    }

    function test_submitOrder_revertsWhenLiveOrderCapReached() public {
        for (uint256 i = 1; i <= book.MAX_LIVE_ORDERS(); i++) {
            OrderTypes.Order memory order =
                _order(aliceAccount, true, 65_000_00000000 + (i * 100), 1e18, i, false);
            book.submitOrder(order, _sign(ALICE_PK, order));
        }
        assertEq(book.liveOrderCount(), book.MAX_LIVE_ORDERS());

        OrderTypes.Order memory overflow =
            _order(aliceAccount, true, 66_000_00000000, 1e18, book.MAX_LIVE_ORDERS() + 1, false);
        bytes memory signature = _sign(ALICE_PK, overflow);

        vm.expectRevert(
            abi.encodeWithSelector(
                OrderBook.TooManyLiveOrders.selector, book.MAX_LIVE_ORDERS(), book.MAX_LIVE_ORDERS()
            )
        );
        book.submitOrder(overflow, signature);
    }

    function _order(
        uint256 accountId,
        bool isBuy,
        uint256 limitPrice,
        uint256 size,
        uint256 nonce,
        bool reduceOnly
    ) internal view returns (OrderTypes.Order memory) {
        return OrderTypes.Order({
            accountId: accountId,
            marketId: btcMarket,
            isBuy: isBuy,
            limitPrice: limitPrice,
            size: size,
            nonce: nonce,
            expiry: block.timestamp + 1 days,
            reduceOnly: reduceOnly
        });
    }

    function _sign(uint256 privateKey, OrderTypes.Order memory order) internal view returns (bytes memory) {
        bytes32 digest = OrderTypes.digest(order, book.DOMAIN_SEPARATOR());
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, digest);
        return abi.encodePacked(r, s, v);
    }

    function _signFor(OrderBook targetBook, uint256 privateKey, OrderTypes.Order memory order)
        internal
        view
        returns (bytes memory)
    {
        bytes32 digest = OrderTypes.digest(order, targetBook.DOMAIN_SEPARATOR());
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, digest);
        return abi.encodePacked(r, s, v);
    }

    function _btcMarket(bool paused) internal view returns (IMarketRegistry.Market memory) {
        return IMarketRegistry.Market({
            symbol: "BTC",
            priceFeed: address(btcFeed),
            initialMarginBps: 1000,
            maintMarginBps: 500,
            maxLeverage: 10,
            tickSize: 100,
            lotSize: 1e15,
            maxPriceAge: MAX_PRICE_AGE,
            paused: paused
        });
    }
}
