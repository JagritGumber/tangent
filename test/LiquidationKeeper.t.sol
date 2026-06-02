// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Test} from "forge-std/Test.sol";
import {AccountManager} from "../src/AccountManager.sol";
import {MarketRegistry} from "../src/MarketRegistry.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {SettlementEngine} from "../src/SettlementEngine.sol";
import {LiquidationKeeper} from "../src/LiquidationKeeper.sol";
import {USDCVault, IERC20} from "../src/USDCVault.sol";
import {IAccountManager} from "../src/interfaces/IAccountManager.sol";
import {IMarketRegistry} from "../src/interfaces/IMarketRegistry.sol";
import {ISettlement} from "../src/interfaces/ISettlement.sol";
import {OrderTypes} from "../src/types/OrderTypes.sol";
import {MockPriceFeed} from "./MockPriceFeed.sol";

contract LiquidationMockUSDC is IERC20 {
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

contract LiquidationKeeperTest is Test {
    uint32 internal constant MAX_PRICE_AGE = 60;

    AccountManager internal accounts;
    MarketRegistry internal markets;
    OrderBook internal book;
    SettlementEngine internal settlement;
    LiquidationKeeper internal keeper;
    USDCVault internal vault;
    LiquidationMockUSDC internal usdc;
    MockPriceFeed internal btcFeed;
    MockPriceFeed internal ethFeed;

    uint256 internal constant ALICE_PK = 0xA11CE;
    uint256 internal constant BOB_PK = 0xB0B;

    address internal alice;
    address internal bob;
    address internal liquidator = address(0x1);

    uint256 internal aliceAccount;
    uint256 internal bobAccount;
    uint256 internal btcMarket;
    uint256 internal ethMarket;

    uint256 internal constant PRICE_65K = 65_000_00000000;
    uint256 internal constant PRICE_62K = 62_000_00000000;
    uint256 internal constant PRICE_50K = 50_000_00000000;
    uint256 internal constant PRICE_3500 = 3_500_00000000;
    uint256 internal constant PRICE_1000 = 1_000_00000000;
    uint256 internal constant ONE_BTC = 1e18;
    uint256 internal constant TEN_ETH = 10e18;
    uint256 internal constant STARTING_COLLATERAL = 100_000_000_000;

    function setUp() public {
        alice = vm.addr(ALICE_PK);
        bob = vm.addr(BOB_PK);

        accounts = new AccountManager();
        markets = new MarketRegistry(address(this));
        usdc = new LiquidationMockUSDC();
        vault = new USDCVault(IERC20(address(usdc)), IAccountManager(address(accounts)));
        book = new OrderBook(address(accounts), address(markets));
        settlement = new SettlementEngine(address(book), address(vault), address(markets));
        keeper = new LiquidationKeeper(address(settlement), address(markets));
        vault.bindSettlementEngine(address(settlement));
        book.bindSettlementEngine(address(settlement));
        settlement.bindLiquidationKeeper(address(keeper));

        btcFeed = new MockPriceFeed(PRICE_65K);
        ethFeed = new MockPriceFeed(PRICE_3500);
        btcMarket = markets.registerMarket(_btcMarket(false));
        ethMarket = markets.registerMarket(_ethMarket(false));

        vm.prank(alice);
        aliceAccount = accounts.registerAccount();
        vm.prank(bob);
        bobAccount = accounts.registerAccount();

        _fund(alice, aliceAccount, STARTING_COLLATERAL);
        _fund(bob, bobAccount, STARTING_COLLATERAL);
    }

    function test_isLiquidatableFalseAboveMaintenance() public {
        _openOneBtcAt65k();
        vm.prank(alice);
        vault.withdraw(aliceAccount, 93_500_000_000, alice);
        btcFeed.setPrice(PRICE_62K);

        assertFalse(keeper.isLiquidatable(aliceAccount, btcMarket));
        (bool liquidatable, int256 equity, uint256 maintenanceMargin) =
            keeper.liquidationState(aliceAccount, btcMarket);
        assertFalse(liquidatable);
        assertEq(equity, 3_500_000_000);
        assertEq(maintenanceMargin, 3_100_000_000);
    }

    function test_liquidateRevertsWhenHealthy() public {
        _openOneBtcAt65k();
        vm.prank(alice);
        vault.withdraw(aliceAccount, 93_500_000_000, alice);
        btcFeed.setPrice(PRICE_62K);

        vm.expectRevert(
            abi.encodeWithSelector(
                LiquidationKeeper.NotLiquidatable.selector,
                aliceAccount,
                btcMarket,
                int256(3_500_000_000),
                uint256(3_100_000_000)
            )
        );
        keeper.liquidate(aliceAccount, btcMarket);
    }

    function test_liquidationUsesAggregateAccountHealthAcrossMarkets() public {
        _openOneBtcAt65k();
        _openTenEthShortAt3500();
        vm.prank(alice);
        vault.withdraw(aliceAccount, 90_000_000_000, alice);

        btcFeed.setPrice(PRICE_50K);
        ethFeed.setPrice(PRICE_1000);

        assertFalse(keeper.isLiquidatable(aliceAccount, btcMarket));
        (bool liquidatable, int256 equity, uint256 maintenanceMargin) =
            keeper.liquidationState(aliceAccount, btcMarket);
        assertFalse(liquidatable);
        assertEq(equity, 20_000_000_000);
        assertEq(maintenanceMargin, 3_000_000_000);
    }

    function test_withdrawRevertsWhenItWouldBreachMaintenance() public {
        _openOneBtcAt65k();
        btcFeed.setPrice(PRICE_50K);

        vm.expectRevert(
            abi.encodeWithSelector(
                SettlementEngine.WithdrawalWouldBreachMaintenance.selector,
                aliceAccount,
                -8_500_000_000,
                uint256(2_500_000_000)
            )
        );
        vm.prank(alice);
        vault.withdraw(aliceAccount, 93_500_000_000, alice);
    }

    function test_liquidationStateRevertsOnStaleMarkPrice() public {
        vm.warp(1 days);
        _openOneBtcAt65k();
        btcFeed.setPriceAt(PRICE_50K, block.timestamp - MAX_PRICE_AGE - 1);

        vm.expectRevert(
            abi.encodeWithSelector(
                MarketRegistry.StalePrice.selector, btcMarket, block.timestamp - MAX_PRICE_AGE - 1, MAX_PRICE_AGE
            )
        );
        keeper.liquidationState(aliceAccount, btcMarket);
    }

    function test_isLiquidatableRevertsOnStaleMarkPrice() public {
        vm.warp(1 days);
        _openOneBtcAt65k();
        btcFeed.setPriceAt(PRICE_50K, block.timestamp - MAX_PRICE_AGE - 1);

        vm.expectRevert(
            abi.encodeWithSelector(
                MarketRegistry.StalePrice.selector, btcMarket, block.timestamp - MAX_PRICE_AGE - 1, MAX_PRICE_AGE
            )
        );
        keeper.isLiquidatable(aliceAccount, btcMarket);
    }

    function test_liquidateRevertsOnStaleMarkPrice() public {
        vm.warp(1 days);
        _openOneBtcAt65k();
        btcFeed.setPriceAt(PRICE_50K, block.timestamp - MAX_PRICE_AGE - 1);

        vm.expectRevert(
            abi.encodeWithSelector(
                MarketRegistry.StalePrice.selector, btcMarket, block.timestamp - MAX_PRICE_AGE - 1, MAX_PRICE_AGE
            )
        );
        keeper.liquidate(aliceAccount, btcMarket);
    }

    function test_liquidateClosesUnderwaterLongAtMark() public {
        _openOneBtcAt65k();
        vm.prank(alice);
        vault.withdraw(aliceAccount, 93_500_000_000, alice);
        btcFeed.setPrice(PRICE_50K);

        assertTrue(keeper.isLiquidatable(aliceAccount, btcMarket));

        vm.prank(liquidator);
        keeper.liquidate(aliceAccount, btcMarket);

        ISettlement.Position memory alicePosition = settlement.positionOf(aliceAccount, btcMarket);
        assertEq(alicePosition.size, 0);
        assertEq(alicePosition.entryPrice, 0);
        assertEq(alicePosition.lockedMargin, 0);
        assertEq(vault.lockedBalanceOf(aliceAccount), 0);
        assertEq(vault.totalBalanceOf(aliceAccount), 0);
    }

    function test_isLiquidatableReturnsFalseWhenNoPosition() public view {
        assertFalse(keeper.isLiquidatable(aliceAccount, btcMarket));
        (bool liquidatable, int256 equity, uint256 maintenanceMargin) =
            keeper.liquidationState(aliceAccount, btcMarket);
        assertFalse(liquidatable);
        assertEq(equity, 0);
        assertEq(maintenanceMargin, 0);
    }

    function _openOneBtcAt65k() internal {
        OrderTypes.Order memory buy =
            _orderForMarket(aliceAccount, btcMarket, true, PRICE_65K, ONE_BTC, 1, false);
        OrderTypes.Order memory sell =
            _orderForMarket(bobAccount, btcMarket, false, PRICE_65K, ONE_BTC, 1, false);
        book.submitOrder(buy, _sign(ALICE_PK, buy));
        book.submitOrder(sell, _sign(BOB_PK, sell));
        book.tick();
    }

    function _openTenEthShortAt3500() internal {
        OrderTypes.Order memory buy =
            _orderForMarket(bobAccount, ethMarket, true, PRICE_3500, TEN_ETH, 2, false);
        OrderTypes.Order memory sell =
            _orderForMarket(aliceAccount, ethMarket, false, PRICE_3500, TEN_ETH, 2, false);
        book.submitOrder(buy, _sign(BOB_PK, buy));
        book.submitOrder(sell, _sign(ALICE_PK, sell));
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
        return _orderForMarket(accountId, btcMarket, isBuy, limitPrice, size, nonce, reduceOnly);
    }

    function _orderForMarket(
        uint256 accountId,
        uint256 marketId,
        bool isBuy,
        uint256 limitPrice,
        uint256 size,
        uint256 nonce,
        bool reduceOnly
    ) internal view returns (OrderTypes.Order memory) {
        return OrderTypes.Order({
            accountId: accountId,
            marketId: marketId,
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
            maxPriceAge: MAX_PRICE_AGE,
            paused: paused
        });
    }

    function _ethMarket(bool paused) internal view returns (IMarketRegistry.Market memory) {
        return IMarketRegistry.Market({
            symbol: "ETH",
            priceFeed: address(ethFeed),
            initialMarginBps: 1000,
            maintMarginBps: 500,
            maxLeverage: 10,
            tickSize: 100,
            lotSize: 1e16,
            maxPriceAge: MAX_PRICE_AGE,
            paused: paused
        });
    }
}
