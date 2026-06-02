# Rust workspace

Off-chain Rust crates and binaries that consume the on-chain Solidity primitives in `../src/`. Half of the deliberate two-layer architecture: Solidity for everything on-chain (Arc requires EVM bytecode, no Stylus equivalent), Rust for everything off-chain (typed clients, daemons, indexers, proving stacks).

See [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §2.2 for the layer's design rationale and §6 for the version roadmap.

## Members

| Crate | Kind | Roadmap | Status |
|---|---|---|---|
| [`tangent-sdk`](./tangent-sdk/) | library | v0.1 (typed-data) → v0.8 (full SDK) | **shipping today**: canonical EIP-712 `Order` + `DomainSeparatorInput` types |

## Roadmap (members landing in future versions)

| Crate | Kind | Lands in |
|---|---|---|
| `tangent-keeper` | binary daemon | v0.8 — calls `OrderBook.tick()` per block + scans for liquidations |
| `tangent-indexer` | binary daemon | v0.9 — Postgres + GraphQL event tap for frontends and analytics |
| `tangent-matcher` | binary daemon | v0.10 — off-chain CLOB with ZK proofs of fair matching |

The workspace `Cargo.toml` reserves the names commented-out so the dependency graph stays coherent as crates land.

## Build

```bash
cd rust
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo test --workspace
cargo run -p tangent-sdk --example construct_order
```

Run the commands above locally until a Rust CI workflow is added to this repository.

## Why ship the Rust workspace at v0.1 if only one crate has content

Two reasons:

1. **The architecture promise.** [`ARCHITECTURE.md`](../ARCHITECTURE.md) describes a two-layer system. Without a `rust/` directory in the file tree, that claim is unverifiable from a fresh clone. The workspace existing is the visible half of the promise.
2. **API stability.** Shipping the canonical EIP-712 `Order` shape and the `DomainSeparatorInput` constants from a versioned Rust crate today means downstream agent builders (Selbo, CapitalArc, future Arc-native agents) can target a stable type today, even before the on-chain `OrderBook` lands at v0.4. When the v0.8 keeper + signing client arrives, the types are already pinned and no consumer has to rewrite their order construction code.

## Versioning

Pre-1.0 like the parent repo. Pinned at `0.1.0` in the workspace package config. We promote past `1.0.0` once the SDK has been used in production by at least one external consumer; see the parent repo's pre-1.0 versioning note in `../ARCHITECTURE.md` §6.
