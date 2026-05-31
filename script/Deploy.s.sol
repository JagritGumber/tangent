// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Script, console2} from "forge-std/Script.sol";
import {AccountManager} from "../src/AccountManager.sol";
import {USDCVault, IERC20} from "../src/USDCVault.sol";
import {MarketRegistry} from "../src/MarketRegistry.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {SettlementEngine} from "../src/SettlementEngine.sol";
import {LiquidationKeeper} from "../src/LiquidationKeeper.sol";
import {IAccountManager} from "../src/interfaces/IAccountManager.sol";

/// @title  Deploy
/// @notice v0.6 deployment script. Deploys AccountManager, USDCVault,
///         MarketRegistry, OrderBook, SettlementEngine, and LiquidationKeeper.
///         USDCVault, OrderBook, and SettlementEngine are wired via one-shot
///         binding.
///         MarketRegistry's admin is set to the deployer for hackathon
///         deployments; production forks should pass a multisig address.
///
/// @dev    Required env vars:
///         - ARC_USDC: address of the USDC ERC-20 on Arc Testnet
///                     (canonical USDC contract on Arc).
///         Optional env vars:
///         - MARKET_REGISTRY_ADMIN: admin address for MarketRegistry.
///                     Defaults to the deployer EOA if unset.
///
///         Run:
///           forge script script/Deploy.s.sol \
///             --rpc-url $ARC_RPC --broadcast --verify
contract Deploy is Script {
    function run() external {
        address usdc = vm.envAddress("ARC_USDC");
        address marketAdmin = vm.envOr("MARKET_REGISTRY_ADMIN", msg.sender);

        vm.startBroadcast();

        AccountManager accountManager = new AccountManager();
        USDCVault vault = new USDCVault(IERC20(usdc), IAccountManager(address(accountManager)));
        MarketRegistry markets = new MarketRegistry(marketAdmin);
        OrderBook orderBook = new OrderBook(address(accountManager), address(markets));
        SettlementEngine settlement =
            new SettlementEngine(address(orderBook), address(vault), address(markets));
        LiquidationKeeper liquidationKeeper = new LiquidationKeeper(address(settlement), address(markets));
        vault.bindSettlementEngine(address(settlement));
        orderBook.bindSettlementEngine(address(settlement));
        settlement.bindLiquidationKeeper(address(liquidationKeeper));

        console2.log("--- Tangent deployment ---");
        console2.log("AccountManager: ", address(accountManager));
        console2.log("USDCVault:      ", address(vault));
        console2.log("MarketRegistry: ", address(markets));
        console2.log("OrderBook:      ", address(orderBook));
        console2.log("Settlement:     ", address(settlement));
        console2.log("Liquidations:   ", address(liquidationKeeper));
        console2.log("USDC (Arc):     ", usdc);
        console2.log("MarketAdmin:    ", marketAdmin);
        console2.log("ChainId:        ", block.chainid);

        vm.stopBroadcast();
    }
}
