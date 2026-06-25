# Rust workspace

Off-chain Rust crates and binaries that consume the on-chain Solidity primitives in `../src/`. Half of the deliberate two-layer architecture: Solidity for everything on-chain (Arc requires EVM bytecode, no Stylus equivalent), Rust for everything off-chain (typed clients, daemons, indexers, proving stacks).

See [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §2.2 for the layer's design rationale and §6 for the version roadmap.

## Members

| Crate | Kind | Roadmap | Status |
|---|---|---|---|
| [`tangent-sdk`](./tangent-sdk/) | library | v0.1 (raw SDK) → v0.8 (RPC client) | **shipping today**: EIP-712 orders, deployment manifest parsing, signed-order calldata, workflow plans, contract calldata helpers, ABI return decoders, decoded-status helpers |

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
.\rust\scripts\check-light.ps1 -Test tx::tests::exposes_selector_helpers -Exact
.\rust\scripts\check-light.ps1 -Example load_manifest
.\rust\scripts\check-light.ps1 -Clippy
```

For broader pre-push verification on a machine with enough headroom:

```bash
cd rust
cargo clippy -p tangent-sdk --all-targets -- -D warnings
cargo test -p tangent-sdk
cargo run -p tangent-sdk --example construct_order
cargo run -p tangent-sdk --example load_manifest
cargo run -p tangent-sdk --example keeper_polling
cargo run -p tangent-sdk --example rpc_executor
```

The helper always serializes Cargo work through the workspace's one-job config.
Its default path is intentionally cheap (`fmt` + `metadata`); `-Clippy` still
compiles all SDK targets, so use it when the machine has enough headroom.
Use `-Exact` with a full test path when you want to avoid Cargo's default
substring test filtering.
Use `-Example` to run one example through the same serialized Cargo settings.
Avoid running test, clippy, and examples in parallel on memory-constrained
Windows machines. The full workspace commands are still valid, but unnecessary
while `tangent-sdk` is the only active crate.
The `load_manifest` example also shows which optional full-stack plans are
available from the checked-in manifest without opening RPC connections. The
`keeper_polling` example shows keeper preview/checkpoint/report shapes without
opening a transport.

## What the SDK does today

`tangent-sdk` is a raw integration layer for builders and agents that want to target Tangent without copying Solidity ABI details into their own code. It currently provides:

- Canonical `Order`, `OrderParams`, `OrderConstraints`, and EIP-712 domain/digest helpers matching `OrderTypes.sol`.
- `PreparedOrder`, `OrderSigningRequest`, `OrderSigner`, `OrderSignature`, signer backend configuration with secret-free backend reports and metadata presence flags, external signing request/response envelopes, compact request reports with explicit request-kind flags plus order and raw transaction presence/review fields, compact response reports with explicit payload-kind and signed-payload presence flags, `ExternalSigningClient`/`ExternalSignerAdapter` bridges for wallet/KMS/relayer transports, and `SignedOrder` helpers for signer-ready digest/domain/order-hash hex, caller-provided signer backends, attaching externally produced signatures, and encoding `OrderBook.submitOrder`.
- `OrderPlacementPlan` and `OrderPlacement` for composing market-read validation, EIP-712 preparation, caller-provided order signing, compact placement review summaries, submit/cancel transactions, and lifecycle read calls for a single order.
- `OrderLifecyclePlan` for composing submit, cancel, `isLive`, and `orderOf` calls around one signed order, with compact submit/cancel/read plan summaries.
- Grouped order lifecycle read calls and fixed-order `isLive` + stored-order decoding for transport layers that batch `isLive` and `orderOf` lookups, including compact decoded lifecycle summaries with cancel transaction flags, inconsistent live/missing-order rejection, and local lifecycle-state / cancel-readiness classification.
- `OrderBookMaintenancePlan` for composing the permissionless `tick()` transaction used by keepers.
- Calldata builders for `OrderBook`, `AccountManager`, `USDCVault`, `MarketRegistry`, and standard ERC-20 calls used by the collateral path.
- `AccountOnboardingPlan` and `AccountStatusPlan` for composing permissionless account registration, register-return decoding, account read calls, owner/account binding classification, compact onboarding summaries with action flags, and onboarding next-step decisions.
- `CollateralDepositPlan`, `CollateralWithdrawPlan`, and `CollateralStatusPlan` for composing unsigned collateral transactions and read calls against USDC + `USDCVault`, including compact deposit/withdrawal summaries, local deposit/withdraw readiness, next-transaction flags, and next-transaction classification.
- `MarketReadPlan` for composing market registry and mark-price read calls, plus compact read/status summaries with price/constraint flags, fixed-order market metadata, registered/tradable-market checks, and summary-based order preflight before order construction or signing.
- `SettlementReadPlan` for composing position, margin-state, withdrawal-validation reads, fixed-order settlement status decoding, compact withdrawal-validation summaries with validation-call and decoded margin/position flags, and withdrawal validation next-call classification without calling restricted settlement entry points.
- `LiquidationReadPlan` for composing liquidation status reads, fixed-order status decoding, compact read/status summaries, readiness and next-transaction flags/classification, and unsigned permissionless liquidation calldata without choosing keeper transport or profitability policy.
- `KeeperRuntimePlan` plus `KeeperPollingPolicy`/`KeeperPollingSnapshot`/`KeeperPollingOutcome` for manifest-derived keeper startup capabilities, chunked event polling queries, serializable polling plan summaries with nested event-query review details, orderbook maintenance due-work classification, liquidation scan readiness, retry-safe checkpoint advancement, and per-candidate settlement/liquidation read + liquidation transaction planning without running a daemon.
- Fixed transaction batches for single-path workflows such as account registration, collateral deposit/withdrawal, keeper tick, and permissionless liquidation.
- Typed read summaries for account, collateral, market, order lifecycle, settlement, and liquidation call results, with fixed-array and transport-returned slice decoders for batched reads.
- Decoded-status helpers for registered account binding checks, collateral balance consistency, deposit/withdrawal amount coverage, withdrawal readiness against optional settlement state, market order constraints, position openness, aggregate margin health, liquidation margin comparison/readiness, settlement status summaries, and manifest full-stack availability.
- Manifest-derived exact event filter sets, broad provider request/query shapes with optional block ranges, compact event-query and query-batch review summaries with open-ended/invalid-range flags, JSON-RPC-ready log query views, bounded JSON-RPC log-window chunking, provider-style `0x` raw-log data parsing with optional source metadata, log cursor/checkpoint helpers, resume queries, post-cursor decoding, exact post-filtered decoding, source-preserving decoded event records for persistence, typed event-log decoders, a known-event dispatcher, event-kind introspection/count summaries, decoded-log batch summaries with known/unknown counts and cursor coverage, and mixed-log batch summaries for core receipt/indexer logs: account registration, market registry changes, collateral movements, order submission/cancel/matches, settlements, and liquidations.
- `TangentEventProjection` for database-free indexer/reference state folding from decoded event records into account collateral/margin/PnL summaries, market registry status, order lifecycle/fill summaries, liquidation history, account-market candidate discovery/counts, replay-safe cursor-aware record application, unknown-log counts, and last-cursor checkpoints.
- `UnsignedCall` / `UnsignedTx` helpers for inspecting selectors, calldata hex, byte lengths, `0x` calldata parsing/serialization, serializable read-call and call-batch review summaries with call/multi-contract presence flags, single/batched RPC-friendly call + block-tagged query views, JSON-RPC 2.0 request/response envelopes and compact request/response batch summaries for calls, logs, receipts, block number, chain id, nonce, gas price, EIP-1559 priority fee, gas estimates, node-managed sends, validated raw signed transaction sends, id-matched batch call responses, decoded transaction preflight bundles with compact readiness summaries, transport-neutral fee policy application, transport-neutral submission plans, serializable transaction-plan review summaries with batch readiness counts, raw-transaction signing request/backend traits, confirmation request/policy/status helpers, receipt and confirmation-plan summaries with log/fee presence flags, batch confirmation aggregation, compact confirmation reports with nested receipt summaries, receipt field presence flags, and batch continuation flags, a dependency-free `JsonRpcTransport` / `JsonRpcExecutor` boundary, a retrying JSON-RPC transport wrapper with compact retry-stat summaries and capped backoff policy for transient transport/provider failures, and `TxWorkflowExecutor` orchestration for preflight -> sign -> raw-send -> confirmation flows plus shared-discovery batch preflight planning, sequential prepared-plan raw submission batches, serializable ordered-batch resume plans with compact resume summaries, continuation flags, and safe-continue decisions, and serializable workflow submission reports with compact confirmation-plan summaries plus transaction/fee/confirmation presence flags where callers bring their own provider, signer, wallet service, or relayer. It also includes zero-value transaction request views, optional sender/nonce/gas/fee transaction metadata, fixed-order batch transaction envelopes with sequential nonces, `0x` call-return and ordered return-batch parsing/read-plan decoding with compact return-size summaries, provider quantity parsing, and transport-returned transaction hash/receipt adapters with decoded receipt-log and execution-fee helpers.
- Deployment-manifest parsing for checked-in Arc Testnet manifests, including typed full-stack availability, compact present/missing contract summaries, missing-contract gates for keeper/client startup, and `TangentContext` as a manifest-bound factory for account, collateral, market, order placement/lifecycle, settlement, liquidation, event-filter plans, and compact deployment capability summaries with explicit availability flags.
- `RpcEndpointConfig`, `TangentClientConfig`, `TangentClientPlan`, and `TangentClient` for validating provider endpoint shape, carrying optional static RPC headers and signer backend configs with unique-key lookup/redaction plus secret-free auth-header and signer-backend presence config/support reports and external signer adapter factories, bundling retry/backoff/confirmation/keeper polling policies, binding those policies to a deployment manifest, producing serializable startup reports with explicit workflow readiness gates, startup health presence flags, and configured keeper polling summaries/previews for manifest/config/keeper capability checks, summarizing/executing typed read plans and manifest-bound account/collateral/market/full-stack read helpers through caller-provided transports, summarizing manifest-bound event log queries before RPC, fetching and exact-filter decoding manifest-bound event logs or source-preserving event records with cursor resume and caller-sized chunked range support, folding fetched records into fresh or caller-carried `TangentEventProjection` snapshots with compact presence-flag summaries, surfacing missing-contract read errors before RPC, preparing market-validated order placements from live market reads, preflighting and summarizing manifest-bound account/collateral/signed-order/keeper-maintenance transaction batches for dry-run review, building single-candidate, explicit-batch, and projection-derived liquidation dry-run reports with decoded readiness plus compact candidate/batch summaries, batch readiness flags, transaction-summary presence flags, and optional ready-transaction summaries, submitting account/collateral/order/keeper transaction workflows through caller-provided signers, exposing compact workflow batch-resume summaries, fetching compact workflow confirmation reports, previewing and executing one caller-managed keeper polling pass with decoded records/events, compact polling plan work-category/block-bound/maintenance selector flags, compact preview summaries with explicit/derived/scan candidate flags, projection-derived liquidation candidates, batched maintenance/liquidation submissions, candidate scan results with submission presence flags, checkpoint outcomes, compact keeper polling checkpoints, restart resume helpers/reports, serializable polling execution/state reports and small execution summaries with compact workflow submission details, event/candidate/scan flags, submission-report presence flags, and submitted transaction hashes, and a persistable next polling state, gating liquidation submission on decoded readiness, and delegating lower-level workflow execution without opening a socket.
- Minimal ABI return decoders for balances, ids, addresses, booleans, no-return guard calls, and bounded signed settlement/liquidation values.

It does not yet open an RPC connection, sleep/poll on its own, or sign through Circle Dev Wallets. Concrete signer backends and keeper daemon runtime wiring still land with the keeper/client work.

## Why ship the Rust workspace before every crate has content

Two reasons:

1. **The architecture promise.** [`ARCHITECTURE.md`](../ARCHITECTURE.md) describes a two-layer system. Without a `rust/` directory in the file tree, that claim is unverifiable from a fresh clone. The workspace existing is the visible half of the promise.
2. **API stability.** Shipping the canonical EIP-712 `Order` shape, domain constants, calldata helpers, and manifest types from a versioned Rust crate today means downstream agent builders (Selbo, CapitalArc, future Arc-native agents) can target a stable raw integration surface. When the v0.8 keeper + RPC client arrives, the low-level order and calldata types are already pinned and consumers do not have to rewrite their order construction code.

## Versioning

Pre-1.0 like the parent repo. Pinned at `0.1.0` in the workspace package config. We promote past `1.0.0` once the SDK has been used in production by at least one external consumer; see the parent repo's pre-1.0 versioning note in `../ARCHITECTURE.md` §6.
