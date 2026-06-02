# ADR 0001 — Batched end-of-block settlement

Status: accepted, v0.1
Date: 2026-05-25

## Context

A permissionless on-chain perp DEX has two structural choices for when matching happens:

1. **Continuous matching.** Each `submitOrder` call attempts to match against the resting book in the same transaction. Filled orders settle immediately. This is what most EVM CLOBs default to (0x v3 style, Vest, etc).
2. **Batched matching.** Orders accumulate in contract state. A permissionless `tick()` call walks the book once and produces a deterministic batch of matches.

The continuous-matching approach has a well-known MEV problem in derivatives contexts. A searcher seeing a victim's `submitOrder` in the mempool can frontrun with their own order to capture the spread, sandwich the victim, or trigger early liquidations against an order they know will move the mid. Standard solutions (commit-reveal, encrypted mempools, MEV-burn) add operational complexity and either delay execution or require external infrastructure.

Tempo's enshrined DEX (https://docs.tempo.xyz/guide/stablecoin-dex) adopts a third path: orders accumulate during the block and match atomically at end-of-block as a system operation. Because matching is deferred to block finalization, no searcher can profit from the order of arrival within the block. The block proposer cannot reorder for MEV because the matching algorithm is deterministic on the final block contents.

Arc inherits the architectural fit for this approach: Malachite BFT gives sub-second deterministic finality (no reorgs to worry about), and Arc's fee market makes per-order gas predictable. The latency cost of waiting one block (~1 second) for matching is acceptable for the agent-trading and swing-trading flows this reference is targeting; high-frequency strategies are explicitly out of scope.

## Decision

Tangent ships deterministic batched matching as a smart-contract approximation of end-of-block matching. Specifically:

- `OrderBook.submitOrder` validates the EIP-712 signature, checks nonce / expiry / market state, and stores the order in contract state without attempting to match it.
- `OrderBook.tick()` is permissionless. Anyone can call it. The caller walks the resting book once with price-time priority, emits `Matched` events for each fill, and hands the match set off to `SettlementEngine.settleBatch` in the same transaction.
- `SettlementEngine.settleBatch` is callable only by the bound `OrderBook`. System-level permissionlessness comes from `tick()` being public; restricting direct settlement keeps fills and book state atomic without a richer proof path.
- A keeper bot can call `tick()` regularly. In the absence of a keeper, any trader who needs their order matched can call `tick()` themselves. Keeper rewards are deferred until fee accounting exists.

The matching algorithm itself is deterministic price-time priority: best bid lifts best ask, FIFO within each price level, partial fills allowed. The current implementation reduces matcher discretion but does not fully eliminate transaction-ordering MEV because `tick()` is still a normal EVM transaction rather than an enshrined block-finalization operation.

## Consequences

**Positive:**
- Matcher discretion is minimized: given the resting book at `tick()`, the fill set is deterministic and atomic.
- Matching cost is shared across all orders in a block, amortizing the per-fill gas overhead.
- Settlement is atomic — partial-batch failure is impossible. Either every match in a `tick()` settles or none do.
- The on-chain matching algorithm is fully transparent and auditable. There is no off-chain matcher in the trust path.

**Negative:**
- Latency floor of ~1 block (~1 second on Arc) between submitting an order and seeing it match. High-frequency strategies need a different venue.
- `tick()` is a gas-heavy operation as book depth grows. Mitigation: bounded live-order count now, per-market queues or ZK-proven off-chain matching later.
- A keeper-coordination problem if no one calls `tick()`. Mitigation: trader self-service today; explicit keeper rewards require fee accounting and are deferred.

**Neutral:**
- Order cancellation works the same way it would under continuous matching — `cancelOrder` flips a flag on the resting order and `tick()` skips it. The cancellation can race a match within the same block, with the match winning (orders are matched in submission order, cancellations are processed in the matching pass).

## Alternatives considered

- **Continuous matching (rejected).** Stronger UX for HFT but unacceptable MEV surface for a leveraged-positions venue. Liquidations become especially exploitable.
- **Encrypted mempool (rejected for MVP).** Adds external trust dependencies and operational complexity. It would improve pre-`tick()` ordering privacy, but it is outside this reference implementation's current scope.
- **Validator-run in-memory matching (dYdX v4 style; rejected).** Requires running a sovereign chain. Out of scope for an Arc smart-contract reference.
- **ZK-proven off-chain matching (Lighter style; deferred to v1.2).** Right long-term scaling path once book depth exceeds what end-of-block on-chain matching can handle. Adds prover infrastructure that's overkill for v0.1.

## References

- Tempo enshrined DEX spec: https://docs.tempo.xyz/guide/stablecoin-dex
- Arc Malachite consensus: https://www.fintechfutures.com/m-a/circle-acquires-malachite-engine-for-arc-blockchain
- dYdX v4 matching design: https://github.com/dydxprotocol/v4-chain
- Lighter Protocol whitepaper: https://assets.lighter.xyz/whitepaper.pdf
