# Tangent

A forkable open-source perpetual-futures DEX reference implementation for Arc Testnet.

A circle is the locus of arcs; a tangent is the line that meets one. Arc is Circle's chain; Tangent is the perp primitive that meets it.

MIT-licensed. Designed for the Arc OSS program (Canteen × Circle × Arc, 2026).

> **Repository slug note.** This repository's GitHub slug is `arc-perp-reference` because that was the working name when the hackathon submission was filed. The project's actual name is **Tangent**; all code, contracts, and EIP-712 domains use `Tangent`. The slug is preserved for permalink stability.

## Why this exists

The only perp DEX deployed on Arc Testnet today is Shapeshifter's CMDT ClearingHouse at `arcperps.xyz`. Per the Arc team's own clarification, three things are closed off to external builders:

1. **The matcher.** `settleBatch` is gated behind `SETTLEMENT_ROLE` and fills come only from an off-chain Go matching engine.
2. **Account onboarding.** Accounts are Fireblocks-custodied; the contract checks signers against `getAccountOwner(accountId)` and that binding is provisioned off-chain.
3. **Discoverability.** The contracts aren't listed in `docs.arc.io` and the `arcperps` branding reads as Arc-native when it isn't.

The result is that every autonomous trading agent the Agora hackathon attracted (Selbo, CapitalArc) ended up on Hyperliquid testnet, because Arc had no permissionless venue to build against. This repository fills that gap with a minimal, forkable reference implementation that any future agent builder can deploy against directly.

## What it ships

Four reusable primitives that no current `circlefin/arc-*` repository covers:

1. **Permissionless EOA-registration account model** with optional sub-account factory. No Fireblocks custody binding required. Anyone can call `AccountManager.registerAccount()` and immediately be tradeable.

2. **On-chain CLOB with deterministic end-of-block batched settlement.** Tempo-inspired pattern adapted to Arc's Malachite sub-second finality. Orders accumulate during the block and match atomically at end-of-block, eliminating MEV between order placement and match.

3. **Public EIP-712 order schema** (`Order(accountId, marketId, isBuy, limitPrice, size, nonce, expiry, reduceOnly)`) under EIP-712 domain `"Tangent v1"`. External agent builders can sign orders the matcher will accept without any permissioned binding step.

4. **Permissionless matching path through `tick()`.** Anyone can call `OrderBook.tick()`; the book matches deterministically and calls the bound `SettlementEngine` atomically. There is no external matcher role, and direct settlement is restricted to the book so fills and book state cannot drift.

Plus the supporting infrastructure: standard ERC-20 USDC vault for collateral, Pyth or Chainlink price feeds on Arc for marks, and an on-chain liquidation entry point.

## Architecture at a glance

```
Tangent
├── AccountManager.sol        Permissionless account registration; balance + positions per account
├── OrderBook.sol             EIP-712 order submission; bounded on-chain CLOB
├── SettlementEngine.sol      Position accounting, margin locking, realized PnL
├── USDCVault.sol             ERC-20 collateral vault; transparent per-account accounting
├── MarketRegistry.sol        Admin-curated markets in v0.3; permissionless in v0.9
├── LiquidationKeeper.sol     Permissionless underwater-position close at mark price
└── types/OrderTypes.sol      EIP-712 typed-data schema for Order
```

## Status

**v0.1 live on Arc Testnet** (see addresses below). **v0.6 OrderBook + SettlementEngine + LiquidationKeeper implemented locally** and ready for the next deployment. Current code ships:

- `AccountManager.sol`: permissionless EOA registration (5 unit + fuzz tests)
- `USDCVault.sol`: per-account USDC collateral with deposit/withdraw + margin hooks gated until the immutable settlement binder binds SettlementEngine once (20+ unit + fuzz tests + handler-driven invariant fuzz)
- `MarketRegistry.sol`: admin-curated perp market catalogue with risk params + pluggable `IPriceFeed` oracle adapter (`MockPriceFeed` for tests; Pyth adapter lands at deploy time) (20+ unit + fuzz tests)
- `OrderBook.sol`: EIP-712 signed order submission, account-owner recovery, nonce protection, market tick/lot validation, owner cancellation, expiry sweep, deterministic price-time matching, partial fills, self-trade skip, pause-aware matching, `Matched` events, bounded live-order count, stored order metadata, and one-shot settlement-engine binding
- `SettlementEngine.sol`: bound-book settlement, per-account/per-market positions, isolated initial-margin locking, proportional margin release, realized PnL application, reduce-only enforcement, paused-market defense, and atomic batch revert
- `LiquidationKeeper.sol`: permissionless underwater-position close at mark price using isolated locked-margin equity; bounty and insurance-fund routing are deferred
- `OrderTypes.sol`: EIP-712 `Order` schema under domain `"Tangent v1"` with frozen-typehash + sign/recover tests
- Interface stubs for the rest: `IPriceFeed`, plus the public `IOrderBook` / `ISettlement` surface used by the local settlement path
- `script/Deploy.s.sol` wiring AccountManager + USDCVault + MarketRegistry + OrderBook + SettlementEngine + LiquidationKeeper end-to-end with one-shot vault/book/liquidation binding
- Integration test (`test/integration/DepositWithdrawRoundtrip.t.sol`) proving register market → register account → deposit → mark-price read → withdraw end-to-end against the three shipped contracts
- GitHub Actions CI (`.github/workflows/solidity.yml`) running `forge build + test + fmt + gas-report` on every push
- ADRs 0001 (batched end-of-block settlement), 0002 (permissionless account onboarding), 0003 (USDCVault design)
- Rust workspace at [`rust/`](./rust/) with the [`tangent-sdk`](./rust/tangent-sdk/) crate shipping the canonical EIP-712 `Order` + `DomainSeparatorInput` types so downstream agents (Selbo, CapitalArc, future Arc-native agents) target the same order shape as the on-chain `OrderBook`. Future crates (`tangent-keeper`, `tangent-indexer`, `tangent-matcher`) reserved in the workspace manifest, landing at v0.8 / v0.9 / v0.10 respectively. Rust CI (`cargo fmt + clippy + check + test + run example`) on every push that touches `rust/**`.

Read `ARCHITECTURE.md` for the system-wide design, the version roadmap (`v0.2` through `v0.10`, with `v1.0` reserved for the post-production-hardening graduation), and Mermaid diagrams of every key flow.

We don't reach `v1.0` until the system has been deployed and used on Arc Testnet for long enough to demonstrate stability. Pre-1.0 is the whole journey.

## Build

```bash
forge install foundry-rs/forge-std
forge build
forge test
```

## Live on Arc Testnet

Tangent v0.1 is **deployed and source-verified** on Arc Testnet (chainId `11111`) as of 2026-05-25. Click any address below and select the "Code" tab on Arcscan to read the verified Solidity:

| Contract | Address |
|---|---|
| AccountManager | [`0x2b1ca7ca0a883cd162c619b4a74f0942b22c0e40`](https://testnet.arcscan.app/address/0x2b1ca7ca0a883cd162c619b4a74f0942b22c0e40) |
| USDCVault | [`0xa4d41df0ad7c420c19c971772e57469459204833`](https://testnet.arcscan.app/address/0xa4d41df0ad7c420c19c971772e57469459204833) |
| MarketRegistry | [`0x96a6e69af20ae3a52e164373c345e0a47f23ead2`](https://testnet.arcscan.app/address/0x96a6e69af20ae3a52e164373c345e0a47f23ead2) |

USDC collateral token on Arc Testnet: [`0x3600000000000000000000000000000000000000`](https://testnet.arcscan.app/address/0x3600000000000000000000000000000000000000).

Full manifest: [`docs/deployments/arc-testnet.json`](./docs/deployments/arc-testnet.json).

**Proof of life.** `AccountManager.registerAccount()` invoked from the deployer wallet on 2026-05-26: [tx 0x46a66e...29dea8](https://testnet.arcscan.app/tx/0x46a66e2b8d5c0f6df7d89c141fb791463f4a14f5031fabf0bb0a8f213e29dea8). The AccountManager now has one registered account on-chain; click through to see the `AccountRegistered` event.

The three primitives are permissionless and live. Anyone can:

- Call `AccountManager.registerAccount()` and receive an `accountId` (no allowlist, no custody binding, no Fireblocks step).
- Call `USDCVault.deposit(accountId, amount)` after approving USDC, and `USDCVault.withdraw(accountId, amount)` to retrieve their balance.
- Read `MarketRegistry.market(marketId)` once the admin (the deployer wallet, by design at v0.1) registers markets. Permissionless market creation lands at v0.9.

The live v0.1 deployment does not include the local v0.6 `OrderBook`, `SettlementEngine`, or `LiquidationKeeper`. Until a new deployment binds those contracts, the margin / lock / PnL hooks on the live `USDCVault` revert if called; deposits and withdrawals work without binding. The local v0.6 deploy script now performs the full one-shot binding for new forks.

## Deploy your own fork

For forks deploying their own instance of the Tangent primitives, the canonical path is Foundry:

```bash
forge script script/Deploy.s.sol --rpc-url $ARC_RPC --broadcast --verify
```

See `script/Deploy.s.sol` for the wiring order. Alternatively, deploy via Circle Smart Contract Platform using the runbook at [`docs/deploy/circle-scp-runbook.md`](./docs/deploy/circle-scp-runbook.md) (no local Foundry required; the GitHub Actions `solidity` workflow ships ready-to-deploy artifacts on every push).

## Architecture Decision Records

See `docs/adr/` for the design rationale behind:

- **0001, Batched end-of-block settlement.** Why we batch matches at block boundaries instead of continuous matching, and the MEV-protection tradeoff vs throughput.
- **0002, Permissionless account onboarding.** Why we use EOA-registration rather than the Fireblocks-custodied pattern Shapeshifter chose.
- **0003, USDCVault design.** Why per-account isolated balances + free/locked split + one-shot settlement-engine binding, vs the alternatives.

## Reference architectures studied

- **Tempo enshrined DEX** (docs.tempo.xyz/guide/stablecoin-dex): inspiration for batched end-of-block settlement, on-chain CLOB, internal balance model. We adopt the batched-settlement pattern but implement as smart contracts rather than an EVM precompile.
- **dYdX v4** (github.com/dydxprotocol/v4-chain): full decentralized matching via validator-run in-memory CLOB. Too heavy as a smart-contract reference but the matching algorithm itself is portable.
- **Lighter Protocol** (docs.lighter.xyz): off-chain matching + on-chain ZK proof verification. Right long-term scaling path, deferred to v1.2.
- **Hyperliquid sub-accounts pattern**: master account + N sub-accounts per trader, isolated margin per sub-account. Optional in v1.0.

## What's deferred past v0.6

- Permissionless market creation (admin-curated in MVP).
- Liquidator bounty payout and insurance-fund routing.
- Funding payments.
- ZK-proven off-chain matching (Lighter-style).
- ERC-4337 SCA account model.
- Cross-margin across markets (start with isolated margin per market).
- Frontend (this is a reference implementation; consumers build their own UIs).

## Composability commitments

Every primitive in this repo is designed to be picked up and reused by other Arc builders:

- `AccountManager` can be forked for any application needing permissionless account registration without custodian binding.
- The `OrderBook` matching engine works for any orderbook market on Arc, not just perpetuals.
- The `SettlementEngine` position/margin core is reusable for isolated-margin leveraged products.
- The batched end-of-block settlement pattern is generalizable to any high-MEV-risk venue on Arc.
- `LiquidationKeeper`'s close-at-mark predicate is reusable for any isolated-margin leveraged product.
- The EIP-712 `Order` schema is meant to be a canonical baseline future Arc perp builders can extend rather than reinvent.

## License

MIT. See [LICENSE](./LICENSE).

## Author

Built for the Arc OSS program. The author also built [Selbo](https://selbo.app) (an autonomous perp-trading agent) for the same Agora hackathon and hit the Shapeshifter wall on Arc firsthand. That's why Tangent exists. Tangent is the missing primitive: a forkable open-source perp DEX that any future Arc agent builder can target without waiting on a permissioned matcher or custody binding. It stands on its own; integration with any specific consumer (Selbo included) is out of scope for this submission.
