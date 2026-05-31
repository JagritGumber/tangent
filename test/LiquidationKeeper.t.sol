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
    AccountManager internal accounts;
    MarketRegistry internal markets;
    OrderBook internal book;
    SettlementEngine internal settlement;
    LiquidationKeeper internal keeper;
    USDCVault internal vault;
    LiquidationMockUSDC internal usdc;
    MockPriceFeed internal btcFeed;

    uint256 internal constant ALICE_PK = 0xA11CE;
    uint256 internal constant BOB_PK = 0xB0B;

    address internal alice;
    address internal bob;
    address internal liquidator = address(0x1);

    uint256 internal aliceAccount;
    uint256 internal bobAccount;
    uint256 internal btcMarket;

    uint256 internal constant PRICE_65K = 65_000_00000000;
    uint256 internal constant PRICE_62K = 62_000_00000000;
    uint256 internal constant PRICE_50K = 50_000_00000000;
    uint256 internal constant ONE_BTC = 1e18;
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
        btcMarket = markets.registerMarket(_btcMarket(false));

        vm.prank(alice);
        aliceAccount = accounts.registerAccount();
        vm.prank(bob);
        bobAccount = accounts.registerAccount();

        _fund(alice, aliceAccount, STARTING_COLLATERAL);
        _fund(bob, bobAccount, STARTING_COLLATERAL);
    }

    function test_constructor_revertsOnZeroAddress() public {
        vm.expectRevert(LiquidationKeeper.ZeroAddress.selector);
        new LiquidationKeeper(address(0), address(markets));

        vm.expectRevert(LiquidationKeeper.ZeroAddress.selector);
        new LiquidationKeeper(address(settlement), address(0));
    }

    function test_isLiquidatableFalseAboveMaintenance() public {
        _openOneBtcAt65k();
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

    function test_liquidateClosesUnderwaterLongAtMark() public {
        _openOneBtcAt65k();
        btcFeed.setPrice(PRICE_50K);

        assertTrue(keeper.isLiquidatable(aliceAccount, btcMarket));

        vm.prank(liquidator);
        keeper.liquidate(aliceAccount, btcMarket);

        ISettlement.Position memory alicePosition = settlement.positionOf(aliceAccount, btcMarket);
        assertEq(alicePosition.size, 0);
        assertEq(alicePosition.entryPrice, 0);
        assertEq(alicePosition.lockedMargin, 0);
        assertEq(vault.lockedBalanceOf(aliceAccount), 0);
        assertEq(vault.freeBalanceOf(aliceAccount), 85_000_000_000);
    }

    function test_liquidateRevertsWhenNoPosition() public {
        vm.expectRevert(
            abi.encodeWithSelector(LiquidationKeeper.NoPosition.selector, aliceAccount, btcMarket)
        );
        keeper.isLiquidatable(aliceAccount, btcMarket);
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
