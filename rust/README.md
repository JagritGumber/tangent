# Rust workspace

Off-chain Rust crates and binaries that consume the on-chain Solidity primitives in `../src/`. Half of the deliberate two-layer architecture: Solidity for everything on-chain (Arc requires EVM bytecode, no Stylus equivalent), Rust for everything off-chain (typed clients, daemons, indexers, proving stacks).

See [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §2.2 for the layer's design rationale and §6 for the version roadmap.

## Members

| Crate | Kind | Roadmap | Status |
|---|---|---|---|
| [`tangent-sdk`](./tangent-sdk/) | library | v0.1 (raw SDK) → v0.8 (RPC client) | **shipping today**: EIP-712 orders, deployment manifest parsing, signed-order calldata, contract calldata helpers, ABI return decoders |

## Roadmap (members landing in future versions)

| Crate | Kind | Lands in |
|---|---|---|
| `tangent-keeper` | binary daemon | v0.8 — calls `OrderBook.tick()` per block + scans for liquidations |
| `tangent-indexer` | binary daemon | v0.9 — Postgres + GraphQL event tap for frontends and analytics |
| `tangent-matcher` | binary daemon | v0.10 — off-chain CLOB with ZK proofs of fair matching |

The workspace `Cargo.toml` reserves the names commented-out so the dependency graph stays coherent as crates land.

## Build

This workspace is configured for low-resource local verification by default:
`rust/.cargo/config.toml` sets `jobs = 1` and disables incremental builds to
avoid Windows paging-file pressure during repeated SDK checks.

```bash
cd rust
cargo fmt --check
cargo test -p tangent-sdk <module>::tests::<test_name>
```

Or run the low-resource helper from the repository root:

```powershell
.\rust\scripts\check-light.ps1
.\rust\scripts\check-light.ps1 -Test lifecycle::tests::decodes_order_lifecycle_status
.\rust\scripts\check-light.ps1 -Clippy
```

For broader pre-push verification on a machine with enough headroom:

```bash
cd rust
cargo clippy -p tangent-sdk --all-targets -- -D warnings
cargo test -p tangent-sdk
cargo run -p tangent-sdk --example construct_order
cargo run -p tangent-sdk --example load_manifest
```

The helper always serializes Cargo work through the workspace's one-job config.
Its default path is intentionally cheap (`fmt` + `metadata`); `-Clippy` still
compiles all SDK targets, so use it when the machine has enough headroom.
Avoid running test, clippy, and examples in parallel on memory-constrained
Windows machines. The full workspace commands are still valid, but unnecessary
while `tangent-sdk` is the only active crate.

## What the SDK does today

`tangent-sdk` is a raw integration layer for builders and agents that want to target Tangent without copying Solidity ABI details into their own code. It currently provides:

- Canonical `Order`, `OrderParams`, `OrderConstraints`, and EIP-712 domain/digest helpers matching `OrderTypes.sol`.
- `PreparedOrder`, `OrderSignature`, and `SignedOrder` helpers for attaching externally produced signatures and encoding `OrderBook.submitOrder`.
- `OrderLifecyclePlan` for composing submit, cancel, `isLive`, and `orderOf` calls around one signed order.
- Grouped order lifecycle read calls for transport layers that batch `isLive` and `orderOf` lookups.
- `OrderBookMaintenancePlan` for composing the permissionless `tick()` transaction used by keepers.
- Calldata builders for `OrderBook`, `AccountManager`, `USDCVault`, `MarketRegistry`, and standard ERC-20 calls used by the collateral path.
- `AccountOnboardingPlan` and `AccountStatusPlan` for composing permissionless account registration, register-return decoding, and account read calls.
- `CollateralDepositPlan`, `CollateralWithdrawPlan`, and `CollateralStatusPlan` for composing unsigned collateral transactions and read calls against USDC + `USDCVault`.
- `MarketReadPlan` for composing market registry and mark-price read calls, plus fixed-order summary decoding before order construction.
- `LiquidationReadPlan` for composing liquidation status reads and unsigned permissionless liquidation calldata without choosing keeper transport or profitability policy.
- Typed read summaries for account, collateral, market, order lifecycle, and liquidation call results.
- Deployment-manifest parsing for checked-in Arc Testnet manifests.
- Minimal ABI return decoders for balances, ids, addresses, booleans, and bounded signed liquidation equity.

It does not yet open an RPC connection, sign through Circle Dev Wallets, estimate gas, or broadcast transactions. Those higher-level client pieces still land with the keeper/client work.

## Why ship the Rust workspace before every crate has content

Two reasons:

1. **The architecture promise.** [`ARCHITECTURE.md`](../ARCHITECTURE.md) describes a two-layer system. Without a `rust/` directory in the file tree, that claim is unverifiable from a fresh clone. The workspace existing is the visible half of the promise.
2. **API stability.** Shipping the canonical EIP-712 `Order` shape, domain constants, calldata helpers, and manifest types from a versioned Rust crate today means downstream agent builders (Selbo, CapitalArc, future Arc-native agents) can target a stable raw integration surface. When the v0.8 keeper + RPC client arrives, the low-level order and calldata types are already pinned and consumers do not have to rewrite their order construction code.

## Versioning

Pre-1.0 like the parent repo. Pinned at `0.1.0` in the workspace package config. We promote past `1.0.0` once the SDK has been used in production by at least one external consumer; see the parent repo's pre-1.0 versioning note in `../ARCHITECTURE.md` §6.
