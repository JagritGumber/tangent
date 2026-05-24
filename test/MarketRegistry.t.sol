// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Test} from "forge-std/Test.sol";
import {MarketRegistry} from "../src/MarketRegistry.sol";
import {IMarketRegistry} from "../src/interfaces/IMarketRegistry.sol";
import {MockPriceFeed} from "./MockPriceFeed.sol";

contract MarketRegistryTest is Test {
    MarketRegistry internal mr;
    MockPriceFeed internal btcFeed;
    MockPriceFeed internal ethFeed;

    address internal admin = address(0xAD1);
    address internal nonAdmin = address(0xBAD);

    event MarketRegistered(uint256 indexed marketId, string symbol, address priceFeed);
    event MarketParamsUpdated(uint256 indexed marketId);
    event MarketPaused(uint256 indexed marketId, bool paused);

    function setUp() public {
        mr = new MarketRegistry(admin);
        btcFeed = new MockPriceFeed(65000_00000000); // $65,000.00 in 1e8 scale
        ethFeed = new MockPriceFeed(3500_00000000); // $3,500.00
    }

    // -- constructor --

    function test_constructor_revertsOnZeroAdmin() public {
        vm.expectRevert(MarketRegistry.ZeroAddress.selector);
        new MarketRegistry(address(0));
    }

    function test_initialState() public view {
        assertEq(mr.admin(), admin);
        assertEq(mr.totalMarkets(), 0);
    }

    // -- registerMarket --

    function test_registerMarket_emitsAndAssignsMonotonicIds() public {
        vm.expectEmit(true, false, false, true);
        emit MarketRegistered(1, "BTC", address(btcFeed));

        vm.prank(admin);
        uint256 btcId = mr.registerMarket(_btcMarket());
        assertEq(btcId, 1, "first market is id 1, not 0");

        vm.prank(admin);
        uint256 ethId = mr.registerMarket(_ethMarket());
        assertEq(ethId, 2);
        assertEq(mr.totalMarkets(), 2);
    }

    function test_registerMarket_revertsOnNonAdmin() public {
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.NotAdmin.selector, nonAdmin));
        vm.prank(nonAdmin);
        mr.registerMarket(_btcMarket());
    }

    function test_registerMarket_storesAllFields() public {
        vm.prank(admin);
        uint256 id = mr.registerMarket(_btcMarket());
        IMarketRegistry.Market memory m = mr.market(id);
        assertEq(m.symbol, "BTC");
        assertEq(m.priceFeed, address(btcFeed));
        assertEq(m.initialMarginBps, 1000);
        assertEq(m.maintMarginBps, 500);
        assertEq(m.maxLeverage, 10);
        assertEq(m.tickSize, 100); // 1e-6 in price scale
        assertEq(m.lotSize, 1e15); // 0.001 BTC
        assertEq(m.paused, false);
    }

    function test_registerMarket_revertsOnEmptySymbol() public {
        IMarketRegistry.Market memory m = _btcMarket();
        m.symbol = "";
        vm.expectRevert(MarketRegistry.EmptySymbol.selector);
        vm.prank(admin);
        mr.registerMarket(m);
    }

    function test_registerMarket_revertsOnZeroPriceFeed() public {
        IMarketRegistry.Market memory m = _btcMarket();
        m.priceFeed = address(0);
        vm.expectRevert(MarketRegistry.InvalidPriceFeed.selector);
        vm.prank(admin);
        mr.registerMarket(m);
    }

    function test_registerMarket_revertsOnMaintGreaterThanInitial() public {
        IMarketRegistry.Market memory m = _btcMarket();
        m.initialMarginBps = 500;
        m.maintMarginBps = 600; // illegal: maint > initial
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.InvalidMarginParams.selector, uint16(500), uint16(600)));
        vm.prank(admin);
        mr.registerMarket(m);
    }

    function test_registerMarket_revertsOnZeroMargin() public {
        IMarketRegistry.Market memory m = _btcMarket();
        m.initialMarginBps = 0;
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.InvalidMarginParams.selector, uint16(0), uint16(500)));
        vm.prank(admin);
        mr.registerMarket(m);
    }

    function test_registerMarket_revertsOnExcessLeverage() public {
        IMarketRegistry.Market memory m = _btcMarket();
        m.maxLeverage = 200; // > 100x cap
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.InvalidLeverage.selector, uint8(200)));
        vm.prank(admin);
        mr.registerMarket(m);
    }

    function test_registerMarket_revertsOnZeroTickOrLot() public {
        IMarketRegistry.Market memory m = _btcMarket();
        m.tickSize = 0;
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.InvalidTickOrLot.selector, uint256(0), uint256(1e15)));
        vm.prank(admin);
        mr.registerMarket(m);
    }

    // -- updateMarketParams --

    function test_updateMarketParams_replacesAndEmits() public {
        vm.prank(admin);
        uint256 id = mr.registerMarket(_btcMarket());

        IMarketRegistry.Market memory updated = _btcMarket();
        updated.maintMarginBps = 700; // tighten maint margin

        vm.expectEmit(true, false, false, true);
        emit MarketParamsUpdated(id);
        vm.prank(admin);
        mr.updateMarketParams(id, updated);

        assertEq(mr.market(id).maintMarginBps, 700);
    }

    function test_updateMarketParams_revertsOnUnknownMarket() public {
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.UnknownMarket.selector, uint256(99)));
        vm.prank(admin);
        mr.updateMarketParams(99, _btcMarket());
    }

    function test_updateMarketParams_revertsOnNonAdmin() public {
        vm.prank(admin);
        uint256 id = mr.registerMarket(_btcMarket());
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.NotAdmin.selector, nonAdmin));
        vm.prank(nonAdmin);
        mr.updateMarketParams(id, _btcMarket());
    }

    // -- setPaused --

    function test_setPaused_togglesAndEmits() public {
        vm.prank(admin);
        uint256 id = mr.registerMarket(_btcMarket());

        vm.expectEmit(true, false, false, true);
        emit MarketPaused(id, true);
        vm.prank(admin);
        mr.setPaused(id, true);
        assertTrue(mr.market(id).paused);

        vm.prank(admin);
        mr.setPaused(id, false);
        assertFalse(mr.market(id).paused);
    }

    function test_setPaused_revertsOnNonAdmin() public {
        vm.prank(admin);
        uint256 id = mr.registerMarket(_btcMarket());
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.NotAdmin.selector, nonAdmin));
        vm.prank(nonAdmin);
        mr.setPaused(id, true);
    }

    // -- markPrice --

    function test_markPrice_returnsFeedPrice() public {
        vm.prank(admin);
        uint256 id = mr.registerMarket(_btcMarket());
        assertEq(mr.markPrice(id), 65000_00000000);

        btcFeed.setPrice(70000_00000000);
        assertEq(mr.markPrice(id), 70000_00000000, "tracks feed price changes");
    }

    function test_markPrice_revertsOnUnknownMarket() public {
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.UnknownMarket.selector, uint256(99)));
        mr.markPrice(99);
    }

    // -- market view revert behavior --

    function test_market_revertsOnUnknown() public {
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.UnknownMarket.selector, uint256(0)));
        mr.market(0);
        vm.expectRevert(abi.encodeWithSelector(MarketRegistry.UnknownMarket.selector, uint256(99)));
        mr.market(99);
    }

    // -- fuzz --

    function testFuzz_registerMarket_assignsUniqueMonotonicIds(uint8 count) public {
        count = uint8(bound(count, 1, 12));
        vm.startPrank(admin);
        for (uint256 i = 1; i <= count; i++) {
            IMarketRegistry.Market memory m = _btcMarket();
            // unique-ish symbol per iteration
            m.symbol = string(abi.encodePacked("M", vm.toString(i)));
            uint256 id = mr.registerMarket(m);
            assertEq(id, i);
        }
        vm.stopPrank();
        assertEq(mr.totalMarkets(), count);
    }

    // -- helpers --

    function _btcMarket() internal view returns (IMarketRegistry.Market memory) {
        return IMarketRegistry.Market({
            symbol: "BTC",
            priceFeed: address(btcFeed),
            initialMarginBps: 1000, // 10% initial = 10x max leverage at entry
            maintMarginBps: 500, // 5% maintenance
            maxLeverage: 10,
            tickSize: 100,
            lotSize: 1e15,
            paused: false
        });
    }

    function _ethMarket() internal view returns (IMarketRegistry.Market memory) {
        return IMarketRegistry.Market({
            symbol: "ETH",
            priceFeed: address(ethFeed),
            initialMarginBps: 1000,
            maintMarginBps: 500,
            maxLeverage: 10,
            tickSize: 10,
            lotSize: 1e16,
            paused: false
        });
    }
}
