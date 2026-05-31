# Deploy to Arc Testnet via Circle Smart Contract Platform

This runbook deploys the three v0.1 contracts (`AccountManager`, `USDCVault`, `MarketRegistry`) to Arc Testnet via Circle Smart Contract Platform's web UI. No local Foundry or private key handling required.

## Prerequisites

- Circle account with access to Smart Contract Platform.
- A Circle Developer-Controlled Wallet on Arc Testnet with some testnet gas.
- The deploy artifacts bundle: download from the latest green `solidity` workflow run on GitHub Actions (https://github.com/JagritGumber/arc-perp-reference/actions/workflows/solidity.yml). Look for the `tangent-deploy-artifacts` artifact at the bottom of the run page.

The bundle contains:

```
deploy-artifacts/
  flat/
    AccountManager.flat.sol
    USDCVault.flat.sol
    MarketRegistry.flat.sol
  AccountManager.compiled.json
  USDCVault.compiled.json
  MarketRegistry.compiled.json
```

`flat/*.sol` are single-file flattened sources (all imports inlined). `*.compiled.json` are the raw Foundry compiler output (contains bytecode + ABI + metadata). Paste whichever format Circle SCP asks for.

## Constants

- **Arc Testnet chain ID**: `11111`
- **USDC on Arc Testnet**: `0x3600000000000000000000000000000000000000`
- **Solidity version used**: `0.8.24` (see `foundry.toml`)
- **Optimizer**: enabled, 200 runs

## Deploy order

The contracts have a dependency chain. Deploy in this order:

### 1. AccountManager

- **Source**: `flat/AccountManager.flat.sol`
- **Constructor args**: none
- After deploy: copy the deployed contract address. Call it `ACCOUNT_MANAGER_ADDR`.

### 2. USDCVault

- **Source**: `flat/USDCVault.flat.sol`
- **Constructor args (2)**:
  1. `_usdc` (address): `0x3600000000000000000000000000000000000000`
  2. `_accounts` (address): `ACCOUNT_MANAGER_ADDR` from step 1
- After deploy: copy the deployed contract address. Call it `USDC_VAULT_ADDR`.

Note: the live v0.1 deployment intentionally left `USDCVault.bindSettlementEngine(...)` uncalled. Until a newer local v0.6 deployment wires `SettlementEngine` and `LiquidationKeeper`, the live margin/PnL hooks revert if called. Deposits and withdrawals work without binding. This is by design.

### 3. MarketRegistry

- **Source**: `flat/MarketRegistry.flat.sol`
- **Constructor args (1)**:
  1. `_admin` (address): the wallet address you want to control the market list. Use your Circle Dev Wallet address.
- After deploy: copy the deployed contract address. Call it `MARKET_REGISTRY_ADDR`.

## After deploy

Three addresses are now live on Arc Testnet. Verify each on a block explorer (whichever Arc Testnet explorer Circle SCP links to in the UI) — you should see the contract's bytecode and a successful deployment transaction.

Then commit the addresses to the repo by editing `docs/deployments/arc-testnet.json`:

```json
{
  "chainId": 11111,
  "deployedAt": "2026-05-25T...Z",
  "deployer": "0x...",
  "contracts": {
    "AccountManager": "0x...",
    "USDCVault": "0x...",
    "MarketRegistry": "0x..."
  },
  "constants": {
    "USDC": "0x3600000000000000000000000000000000000000"
  }
}
```

Also update the `README.md` "Deploy to Arc Testnet" section: replace the placeholder `forge script ...` line with a "Live on Arc Testnet" subsection listing the three addresses.

## What is NOT done in v0.1

- No `OrderBook` deployed (lands v0.4).
- No `SettlementEngine` deployed in the live v0.1 manifest. `USDCVault.bindSettlementEngine` is unbound there.
- No `LiquidationKeeper` deployed in the live v0.1 manifest.
- No live markets registered (admin can call `MarketRegistry.registerMarket(...)` once a real `IPriceFeed` adapter exists, v0.3).

The deploy demonstrates the three working primitives. Anyone can already:
- Call `AccountManager.registerAccount()` and receive an `accountId`.
- Call `USDCVault.deposit(accountId, amount)` after approving USDC.
- Call `USDCVault.withdraw(accountId, amount)` to retrieve their balance.
- Read `MarketRegistry.market(marketId)` once the admin has registered markets.
