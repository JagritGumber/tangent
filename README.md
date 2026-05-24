# arc-perp-reference

A forkable open-source perpetual-futures DEX reference implementation for Arc Testnet.

MIT-licensed. Designed for the Arc OSS program (Canteen × Circle × Arc, 2026).

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

3. **Public EIP-712 order schema** (`Order(accountId, marketId, isBuy, limitPrice, size, nonce, expiry, reduceOnly)`) under EIP-712 domain `"ArcPerpRef v1"`. External agent builders can sign orders the matcher will accept without any permissioned binding step.

4. **Permissionless `settleBatch` entry point.** Anyone can call it with a valid match set. Margin and liquidation checks enforced on-chain. No `SETTLEMENT_ROLE`, no closed engine.

Plus the supporting infrastructure: standard ERC-20 USDC vault for collateral, Pyth or Chainlink price feeds on Arc for marks, on-chain liquidation keepers with slashing.

## Architecture at a glance

```
ArcPerpRef
├── AccountManager.sol        Permissionless account registration; balance + positions per account
├── OrderBook.sol             EIP-712 order submission; in-memory CLOB during block
├── SettlementEngine.sol      Permissionless settleBatch; margin + liquidation enforcement
├── USDCVault.sol             ERC-20 collateral vault; transparent per-account accounting
├── MarketRegistry.sol        Admin-curated markets in v0.3; permissionless in v0.9
├── LiquidationKeeper.sol     Bot-callable liquidation; slashing for invalid calls
└── types/OrderTypes.sol      EIP-712 typed-data schema for Order
```

## Status

**v0.1 shipping today.** Three working primitives plus the full interface surface for the rest:

- `AccountManager.sol` — permissionless EOA registration (5 unit + fuzz tests)
- `USDCVault.sol` — per-account USDC collateral with deposit/withdraw + margin hooks gated until SettlementEngine binds (20+ unit + fuzz tests + handler-driven invariant fuzz)
- `MarketRegistry.sol` — admin-curated perp market catalogue with risk params + pluggable `IPriceFeed` oracle adapter (`MockPriceFeed` for tests; Pyth adapter lands at deploy time) (20+ unit + fuzz tests)
- `OrderTypes.sol` — EIP-712 `Order` schema under domain `"ArcPerpRef v1"` with frozen-typehash + sign/recover tests
- Interface stubs for the rest: `IOrderBook`, `ISettlement`, `IPriceFeed` — frozen API surface so v0.4–v0.6 implementations don't churn downstream
- `script/Deploy.s.sol` wiring AccountManager + USDCVault + MarketRegistry end-to-end with on-chain manifest emission
- Integration test (`test/integration/DepositWithdrawRoundtrip.t.sol`) proving register market → register account → deposit → mark-price read → withdraw end-to-end against the three shipped contracts
- GitHub Actions CI (`.github/workflows/solidity.yml`) running `forge build + test + fmt + gas-report` on every push
- ADRs 0001 (batched end-of-block settlement), 0002 (permissionless account onboarding), 0003 (USDCVault design)

Read `ARCHITECTURE.md` for the system-wide design, the version roadmap (`v0.2` through `v0.10`, with `v1.0` reserved for the post-production-hardening graduation), and Mermaid diagrams of every key flow.

We don't reach `v1.0` until the system has been deployed and used on Arc Testnet for long enough to demonstrate stability. Pre-1.0 is the whole journey.

## Build

```bash
forge install foundry-rs/forge-std
forge build
forge test
```

## Deploy to Arc Testnet

```bash
forge script script/Deploy.s.sol --rpc-url $ARC_RPC --broadcast --verify
```

(See `script/Deploy.s.sol` for the wiring order — vault, market registry, order book, settlement engine, liquidation keeper, account manager.)

## Architecture Decision Records

See `docs/adr/` for the design rationale behind:

- **0001 — Batched end-of-block settlement.** Why we batch matches at block boundaries instead of continuous matching, and the MEV-protection tradeoff vs throughput.
- **0002 — Permissionless account onboarding.** Why we use EOA-registration rather than the Fireblocks-custodied pattern Shapeshifter chose.

## Reference architectures studied

- **Tempo enshrined DEX** (docs.tempo.xyz/guide/stablecoin-dex): inspiration for batched end-of-block settlement, on-chain CLOB, internal balance model. We adopt the batched-settlement pattern but implement as smart contracts rather than an EVM precompile.
- **dYdX v4** (github.com/dydxprotocol/v4-chain): full decentralized matching via validator-run in-memory CLOB. Too heavy as a smart-contract reference but the matching algorithm itself is portable.
- **Lighter Protocol** (docs.lighter.xyz): off-chain matching + on-chain ZK proof verification. Right long-term scaling path, deferred to v1.2.
- **Hyperliquid sub-accounts pattern**: master account + N sub-accounts per trader, isolated margin per sub-account. Optional in v1.0.

## What's deferred past v0.1

- Permissionless market creation (admin-curated in MVP).
- ZK-proven off-chain matching (Lighter-style).
- ERC-4337 SCA account model.
- Cross-margin across markets (start with isolated margin per market).
- Funding rate auctions (start with simple TWAP-based funding).
- Frontend (this is a reference implementation; consumers build their own UIs).

## Composability commitments

Every primitive in this repo is designed to be picked up and reused by other Arc builders:

- `AccountManager` can be forked for any application needing permissionless account registration without custodian binding.
- The `OrderBook` matching engine works for any orderbook market on Arc, not just perpetuals.
- The batched end-of-block settlement pattern is generalizable to any high-MEV-risk venue on Arc.
- `LiquidationKeeper` with slashing is reusable for any leveraged product.
- The EIP-712 `Order` schema is meant to be a canonical baseline future Arc perp builders can extend rather than reinvent.

## License

MIT. See [LICENSE](./LICENSE).

## Author

Built for the Arc OSS program. The author also built [Selbo](https://selbo.app) (an autonomous perp-trading agent) for the same Agora hackathon and hit the Shapeshifter wall on Arc firsthand — that's why this repo exists. `arc-perp-reference` is the missing primitive: a forkable open-source perp DEX that any future Arc agent builder can target without waiting on a permissioned matcher or custody binding. It stands on its own; integration with any specific consumer (Selbo included) is out of scope for this submission.
