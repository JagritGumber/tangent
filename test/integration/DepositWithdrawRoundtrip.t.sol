// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Test} from "forge-std/Test.sol";
import {AccountManager} from "../../src/AccountManager.sol";
import {USDCVault, IERC20} from "../../src/USDCVault.sol";
import {MarketRegistry} from "../../src/MarketRegistry.sol";
import {IAccountManager} from "../../src/interfaces/IAccountManager.sol";
import {IMarketRegistry} from "../../src/interfaces/IMarketRegistry.sol";
import {MockUSDC} from "../USDCVault.t.sol";
import {MockPriceFeed} from "../MockPriceFeed.sol";

/// @notice End-to-end primitive smoke test: register a market, register an
///         account, deposit USDC, read live mark price, withdraw USDC.
contract DepositWithdrawRoundtripTest is Test {
    uint32 internal constant MAX_PRICE_AGE = 60;

    MockUSDC internal usdc;
    AccountManager internal accounts;
    USDCVault internal vault;
    MarketRegistry internal markets;
    MockPriceFeed internal btcFeed;

    address internal trader = address(0xCAFE);
    address internal admin = address(0xAD1);

    function setUp() public {
        usdc = new MockUSDC();
        accounts = new AccountManager();
        vault = new USDCVault(IERC20(address(usdc)), IAccountManager(address(accounts)));
        markets = new MarketRegistry(admin);
        btcFeed = new MockPriceFeed(65000_00000000); // $65k in 1e8

        usdc.mint(trader, 500_000_000); // 500 USDC
    }

    function test_v01_endToEndRoundtrip() public {
        // 0. Admin curates the BTC market. Markets are admin-gated in v0.3;
        //    permissionless registration ships in v0.9 with bond + slashing.
        IMarketRegistry.Market memory btc = IMarketRegistry.Market({
            symbol: "BTC",
            priceFeed: address(btcFeed),
            initialMarginBps: 1000,
            maintMarginBps: 500,
            maxLeverage: 10,
            tickSize: 100,
            lotSize: 1e15,
            maxPriceAge: MAX_PRICE_AGE,
            paused: false
        });
        vm.prank(admin);
        uint256 btcMarketId = markets.registerMarket(btc);
        assertEq(btcMarketId, 1, "first market is id 1");
        assertEq(markets.markPrice(btcMarketId), 65000_00000000, "live oracle price");

        // 1. Permissionless account registration.
        vm.prank(trader);
        uint256 accountId = accounts.registerAccount();
        assertEq(accountId, 1, "first account id");
        assertEq(accounts.ownerOf(accountId), trader);
        assertEq(accounts.accountIdOf(trader), accountId);

        // 2. Approve + deposit USDC into the vault.
        vm.prank(trader);
        usdc.approve(address(vault), 250_000_000);
        vm.prank(trader);
        vault.deposit(accountId, 250_000_000);

        assertEq(vault.freeBalanceOf(accountId), 250_000_000, "free balance after deposit");
        assertEq(vault.lockedBalanceOf(accountId), 0, "no margin locked pre-settlement-engine");
        assertEq(vault.totalBalanceOf(accountId), 250_000_000);
        assertEq(usdc.balanceOf(trader), 250_000_000, "trader wallet debited");
        assertEq(usdc.balanceOf(address(vault)), 250_000_000, "vault holds the funds");

        // 3. Partial withdrawal back to wallet.
        vm.prank(trader);
        vault.withdraw(accountId, 100_000_000, trader);

        assertEq(vault.freeBalanceOf(accountId), 150_000_000, "free balance after partial withdraw");
        assertEq(usdc.balanceOf(trader), 350_000_000, "trader wallet refunded");
        assertEq(usdc.balanceOf(address(vault)), 150_000_000, "vault holds residual");

        // 4. Full withdrawal back to wallet.
        vm.prank(trader);
        vault.withdraw(accountId, 150_000_000, trader);

        assertEq(vault.freeBalanceOf(accountId), 0, "free balance drained");
        assertEq(vault.totalBalanceOf(accountId), 0);
        assertEq(usdc.balanceOf(trader), 500_000_000, "trader wallet fully restored");
        assertEq(usdc.balanceOf(address(vault)), 0, "vault drained");

        // 5. Account row persists post-withdrawal so the trader can
        //    re-deposit later without re-registering.
        assertEq(accounts.accountIdOf(trader), accountId, "account row persists");

        // 6. Market also persists; its price updates when the oracle moves.
        //    Once OrderBook + SettlementEngine ship in v0.4/v0.5, this
        //    market is what new orders reference.
        btcFeed.setPrice(70000_00000000);
        assertEq(markets.markPrice(btcMarketId), 70000_00000000, "market tracks oracle updates");
        assertEq(markets.market(btcMarketId).symbol, "BTC", "market metadata persists");
    }

}
