// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Test} from "forge-std/Test.sol";
import {AccountManager} from "../src/AccountManager.sol";
import {MarketRegistry} from "../src/MarketRegistry.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {SettlementEngine} from "../src/SettlementEngine.sol";
import {USDCVault, IERC20} from "../src/USDCVault.sol";
import {IAccountManager} from "../src/interfaces/IAccountManager.sol";
import {IMarketRegistry} from "../src/interfaces/IMarketRegistry.sol";
import {ISettlement} from "../src/interfaces/ISettlement.sol";
import {OrderTypes} from "../src/types/OrderTypes.sol";
import {MockPriceFeed} from "./MockPriceFeed.sol";

contract SettlementMockUSDC is IERC20 {
    string public name = "Mock USDC";
    string public symbol = "USDC";
    uint8 public decimals = 6;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transfer(address to, uint256 amount) external override returns (bool) {
        require(balanceOf[msg.sender] >= amount, "balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external override returns (bool) {
        require(balanceOf[from] >= amount, "balance");
        require(allowance[from][msg.sender] >= amount, "allowance");
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

contract SettlementEngineTest is Test {
    AccountManager internal accounts;
    MarketRegistry internal markets;
    OrderBook internal book;
    SettlementEngine internal settlement;
    USDCVault internal vault;
    SettlementMockUSDC internal usdc;
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

    uint256 internal constant PRICE_65K = 65_000_00000000;
    uint256 internal constant PRICE_66K = 66_000_00000000;
    uint256 internal constant ONE_BTC = 1e18;
    int256 internal constant ONE_BTC_SIGNED = 1e18;
    uint256 internal constant STARTING_COLLATERAL = 100_000_000_000; // 100,000 USDC

    function setUp() public {
        alice = vm.addr(ALICE_PK);
        bob = vm.addr(BOB_PK);
        carol = vm.addr(CAROL_PK);

        accounts = new AccountManager();
        markets = new MarketRegistry(address(this));
        usdc = new SettlementMockUSDC();
        vault = new USDCVault(IERC20(address(usdc)), IAccountManager(address(accounts)));
        book = new OrderBook(address(accounts), address(markets));
        settlement = new SettlementEngine(address(book), address(vault), address(markets));
        vault.bindSettlementEngine(address(settlement));
        book.bindSettlementEngine(address(settlement));

        btcFeed = new MockPriceFeed(PRICE_65K);
        btcMarket = markets.registerMarket(_btcMarket(false));

        vm.prank(alice);
        aliceAccount = accounts.registerAccount();
        vm.prank(bob);
        bobAccount = accounts.registerAccount();
        vm.prank(carol);
        carolAccount = accounts.registerAccount();

        _fund(alice, aliceAccount, STARTING_COLLATERAL);
        _fund(bob, bobAccount, STARTING_COLLATERAL);
        _fund(carol, carolAccount, STARTING_COLLATERAL);
    }

    function test_constructor_revertsOnZeroAddress() public {
        vm.expectRevert(SettlementEngine.ZeroAddress.selector);
        new SettlementEngine(address(0), address(vault), address(markets));

        vm.expectRevert(SettlementEngine.ZeroAddress.selector);
        new SettlementEngine(address(book), address(0), address(markets));

        vm.expectRevert(SettlementEngine.ZeroAddress.selector);
        new SettlementEngine(address(book), address(vault), address(0));
    }

    function test_settleBatch_revertsWhenCallerIsNotOrderBook() public {
        ISettlement.Match[] memory matches = new ISettlement.Match[](0);
        vm.expectRevert(abi.encodeWithSelector(SettlementEngine.OnlyOrderBook.selector, address(this)));
        settlement.settleBatch(matches);
    }

    function test_tickOpensLongAndShortAndLocksMargin() public {
        _openOneBtcAt65k();

        SettlementEngine.Position memory alicePosition = settlement.positionOf(aliceAccount, btcMarket);
        SettlementEngine.Position memory bobPosition = settlement.positionOf(bobAccount, btcMarket);

        assertEq(alicePosition.size, ONE_BTC_SIGNED);
        assertEq(bobPosition.size, -ONE_BTC_SIGNED);
        assertEq(alicePosition.entryPrice, PRICE_65K);
        assertEq(bobPosition.entryPrice, PRICE_65K);
        assertEq(alicePosition.lockedMargin, 6_500_000_000);
        assertEq(bobPosition.lockedMargin, 6_500_000_000);
        assertEq(vault.lockedBalanceOf(aliceAccount), 6_500_000_000);
        assertEq(vault.freeBalanceOf(aliceAccount), STARTING_COLLATERAL - 6_500_000_000);
    }

    function test_increaseUpdatesWeightedEntryAndLocksMoreMargin() public {
        _openOneBtcAt65k();

        OrderTypes.Order memory buy = _order(aliceAccount, true, PRICE_66K, ONE_BTC, 2, false);
        OrderTypes.Order memory sell = _order(carolAccount, false, PRICE_66K, ONE_BTC, 1, false);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(CAROL_PK, sell));
        book.tick();

        SettlementEngine.Position memory alicePosition = settlement.positionOf(aliceAccount, btcMarket);
        assertEq(alicePosition.size, 2 * ONE_BTC_SIGNED);
        assertEq(alicePosition.entryPrice, 65_500_00000000);
        assertEq(alicePosition.lockedMargin, 13_100_000_000);
        assertEq(vault.lockedBalanceOf(aliceAccount), 13_100_000_000);
    }

    function test_partialCloseReleasesMarginAndRealizesPnL() public {
        _openOneBtcAt65k();

        OrderTypes.Order memory buy = _order(carolAccount, true, PRICE_66K, ONE_BTC / 2, 1, false);
        OrderTypes.Order memory sell = _order(aliceAccount, false, PRICE_66K, ONE_BTC / 2, 2, false);
        book.submitOrder(buy, _sign(CAROL_PK, buy));
        book.submitOrder(sell, _sign(ALICE_PK, sell));
        book.tick();

        SettlementEngine.Position memory alicePosition = settlement.positionOf(aliceAccount, btcMarket);
        assertEq(alicePosition.size, ONE_BTC_SIGNED / 2);
        assertEq(alicePosition.entryPrice, PRICE_65K);
        assertEq(alicePosition.lockedMargin, 3_250_000_000);
        assertEq(vault.lockedBalanceOf(aliceAccount), 3_250_000_000);
        assertEq(vault.freeBalanceOf(aliceAccount), 97_250_000_000);
    }

    function test_fullCloseClearsPosition() public {
        _openOneBtcAt65k();

        OrderTypes.Order memory buy = _order(carolAccount, true, PRICE_66K, ONE_BTC, 1, false);
        OrderTypes.Order memory sell = _order(aliceAccount, false, PRICE_66K, ONE_BTC, 2, false);
        book.submitOrder(buy, _sign(CAROL_PK, buy));
        book.submitOrder(sell, _sign(ALICE_PK, sell));
        book.tick();

        SettlementEngine.Position memory alicePosition = settlement.positionOf(aliceAccount, btcMarket);
        assertEq(alicePosition.size, 0);
        assertEq(alicePosition.entryPrice, 0);
        assertEq(alicePosition.lockedMargin, 0);
        assertEq(vault.lockedBalanceOf(aliceAccount), 0);
        assertEq(vault.freeBalanceOf(aliceAccount), 101_000_000_000);
    }

    function test_flipClosesOldSideAndOpensResidual() public {
        _openOneBtcAt65k();

        OrderTypes.Order memory buy = _order(carolAccount, true, PRICE_66K, 2 * ONE_BTC, 1, false);
        OrderTypes.Order memory sell = _order(aliceAccount, false, PRICE_66K, 2 * ONE_BTC, 2, false);
        book.submitOrder(buy, _sign(CAROL_PK, buy));
        book.submitOrder(sell, _sign(ALICE_PK, sell));
        book.tick();

        SettlementEngine.Position memory alicePosition = settlement.positionOf(aliceAccount, btcMarket);
        assertEq(alicePosition.size, -ONE_BTC_SIGNED);
        assertEq(alicePosition.entryPrice, PRICE_66K);
        assertEq(alicePosition.lockedMargin, 6_600_000_000);
        assertEq(vault.lockedBalanceOf(aliceAccount), 6_600_000_000);
        assertEq(vault.freeBalanceOf(aliceAccount), 94_400_000_000);
    }

    function test_reduceOnlyAllowsReduction() public {
        _openOneBtcAt65k();

        OrderTypes.Order memory buy = _order(carolAccount, true, PRICE_66K, ONE_BTC / 2, 1, false);
        OrderTypes.Order memory sell = _order(aliceAccount, false, PRICE_66K, ONE_BTC / 2, 2, true);
        book.submitOrder(buy, _sign(CAROL_PK, buy));
        book.submitOrder(sell, _sign(ALICE_PK, sell));
        book.tick();

        SettlementEngine.Position memory alicePosition = settlement.positionOf(aliceAccount, btcMarket);
        assertEq(alicePosition.size, ONE_BTC_SIGNED / 2);
    }

    function test_reduceOnlyRejectsOpenOrFlipAndRollsBackBookFill() public {
        _openOneBtcAt65k();

        OrderTypes.Order memory buy = _order(aliceAccount, true, PRICE_66K, ONE_BTC, 2, true);
        OrderTypes.Order memory sell = _order(carolAccount, false, PRICE_66K, ONE_BTC, 1, false);
        bytes32 buyHash = OrderTypes.hash(buy);
        bytes32 sellHash = OrderTypes.hash(sell);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(CAROL_PK, sell));

        vm.expectRevert(
            abi.encodeWithSelector(SettlementEngine.ReduceOnlyViolation.selector, aliceAccount, btcMarket)
        );
        book.tick();

        assertTrue(book.isLive(buyHash));
        assertTrue(book.isLive(sellHash));
        assertEq(book.remaining(buyHash), ONE_BTC);
        assertEq(book.remaining(sellHash), ONE_BTC);
    }

    function test_insufficientMarginRevertsAndLeavesOrdersLive() public {
        OrderTypes.Order memory buy = _order(aliceAccount, true, PRICE_65K, 20 * ONE_BTC, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, PRICE_65K, 20 * ONE_BTC, 1, false);
        bytes32 buyHash = OrderTypes.hash(buy);
        bytes32 sellHash = OrderTypes.hash(sell);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));

        vm.expectRevert();
        book.tick();

        assertTrue(book.isLive(buyHash));
        assertTrue(book.isLive(sellHash));
        assertEq(vault.lockedBalanceOf(aliceAccount), 0);
        assertEq(vault.lockedBalanceOf(bobAccount), 0);
    }

    function test_settlementRejectsPausedMarketDefensively() public {
        OrderTypes.Order memory buy = _order(aliceAccount, true, PRICE_65K, ONE_BTC, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, PRICE_65K, ONE_BTC, 1, false);
        bytes32 buyHash = OrderTypes.hash(buy);
        bytes32 sellHash = OrderTypes.hash(sell);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));

        ISettlement.Match[] memory matches = new ISettlement.Match[](1);
        matches[0] = ISettlement.Match({
            buyOrderHash: buyHash,
            sellOrderHash: sellHash,
            buyAccountId: aliceAccount,
            sellAccountId: bobAccount,
            marketId: btcMarket,
            size: ONE_BTC,
            price: PRICE_65K
        });

        markets.setPaused(btcMarket, true);
        vm.expectRevert(abi.encodeWithSelector(SettlementEngine.PausedMarket.selector, btcMarket));
        vm.prank(address(book));
        settlement.settleBatch(matches);
    }

    function _openOneBtcAt65k() internal {
        OrderTypes.Order memory buy = _order(aliceAccount, true, PRICE_65K, ONE_BTC, 1, false);
        OrderTypes.Order memory sell = _order(bobAccount, false, PRICE_65K, ONE_BTC, 1, false);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));
        book.tick();
    }

    function _fund(address trader, uint256 accountId, uint256 amount) internal {
        usdc.mint(trader, amount);
        vm.prank(trader);
        usdc.approve(address(vault), type(uint256).max);
        vm.prank(trader);
        vault.deposit(accountId, amount);
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

    function _btcMarket(bool paused) internal view returns (IMarketRegistry.Market memory) {
        return IMarketRegistry.Market({
            symbol: "BTC",
            priceFeed: address(btcFeed),
            initialMarginBps: 1000,
            maintMarginBps: 500,
            maxLeverage: 10,
            tickSize: 100,
            lotSize: 1e15,
            paused: paused
        });
    }
}
