# ADR 0003 — USDCVault design

Status: accepted, v0.1
Date: 2026-05-25

## Context

`USDCVault` is the only contract in Tangent that holds user funds. It must do three things well:

1. Let any registered account deposit and withdraw USDC without intermediaries.
2. Expose margin hooks (`lockMargin`, `releaseMargin`, `applyPnL`) that `SettlementEngine` can call to enforce position health during trades.
3. Not be the contract through which someone exfiltrates the entire vault if a bug is found elsewhere in the system.

The Shapeshifter "USDCCollateralVault" on Arc Testnet (`0x75E4FBFBA942A82F0f5CA9663571233823A71f11`) is the contrast point. That vault binds collateral to Fireblocks-custodied accounts via an off-chain provisioning step. The on-chain checks read against an off-chain-mutated mapping. For an autonomous-agent venue this binding is a hard onboarding wall (see ADR 0002). The vault we ship has to break that pattern while remaining safe.

Three design decisions in `USDCVault.sol` are worth recording because they trade off against alternatives that a reviewer might reasonably ask about.

## Decision 1: Deposits accept any depositor for any registered account

`deposit(uint256 accountId, uint256 amount)` does not require `msg.sender == AccountManager.ownerOf(accountId)`. Any address can fund any registered account's vault balance. The check that matters is the existence of the account (we revert if `ownerOf` returns `address(0)` — the unregistered sentinel).

**Why:** the natural pattern for autonomous agents on Arc is to be funded by an external custody desk, a treasury wallet, a sponsor, or a Circle programmable wallet whose entity-secret holder is operationally different from the agent's signing wallet. Requiring the depositor to be the account owner forces those flows through a redundant approve-then-transfer-then-deposit dance from the owner's address, which adds attack surface and friction without security benefit (the funds end up in the same internal balance either way).

**What this gives up:** we lose the ability to detect "wrong-account deposit" mistakes at the protocol level. A user who fat-fingers an accountId in the SDK funds the wrong account and has no recourse from the vault. Mitigation: the SDK exposes a high-level helper that auto-binds the deposit to the caller's own account; manual misuse of the contract-level entry point is on the caller.

**What this does NOT give up:** withdrawal is owner-only (`withdraw` checks `msg.sender == ownerOf`), so the third-party-funding asymmetry is intentional. Anyone can put money in; only the account owner can take it out.

## Decision 2: One-shot SettlementEngine binding, no admin upgrade path

The margin hooks (`lockMargin`, `releaseMargin`, `applyPnL`) are gated by `onlySettlement`, which requires `msg.sender == settlementEngine`. The `settlementEngine` address is set exactly once via `bindSettlementEngine(address)`. The setter reverts on the second call:

```solidity
if (settlementEngine != address(0)) revert SettlementEngineAlreadyBound(settlementEngine);
```

There is no admin role, no proxy, no upgrade path, no governance vote to change the bound engine. The binding is immutable in practice.

**Why:** the SettlementEngine has the power to mutate every account's balance (via `applyPnL` with a negative value, in the limit). If the binding were mutable, an admin compromise would let an attacker swap the engine for a malicious contract and drain every locked margin into one account. The one-shot binding moves that risk surface from "trust the admin key forever" to "trust the deploy script once". The deploy script is reproducible from public source; the admin key is a long-lived secret.

**What this gives up:** if the SettlementEngine has a critical bug, the fix is to redeploy `USDCVault` alongside the new `SettlementEngine` and migrate funds. That is a heavier upgrade path than a proxy. For a reference implementation aimed at being forked cleanly, this is the right tradeoff — forks that need an admin upgrade path can add one in their own deployment.

**Time-of-deploy ordering:** the deploy script (`script/Deploy.s.sol`) deploys USDCVault before SettlementEngine, then calls `bindSettlementEngine(settlement)` as part of the one-shot wiring. Until that final step runs, all margin hooks revert with `SettlementEngineNotBound`. This is covered in the vault unit tests.

## Decision 3: Negative PnL absorbs from free first, then locked, then zeros out

The `applyPnL(int256 pnl)` hook handles three regimes:

- `pnl > 0` — credit to free balance, no clamps. Wins land in withdrawable balance immediately.
- `pnl < 0` and `|pnl| <= free` — debit from free balance. Account remains margin-healthy.
- `pnl < 0` and `|pnl| > free` — drain free to zero, take the residual from locked balance. If the residual also exceeds locked, zero out locked too and absorb the rest at the vault level (the bad-debt path).

The bad-debt absorption (residual loss greater than total balance, zeroing out without reverting) is the most subtle behavior in the vault. It is intentional and documented inline.

**Why:** liquidations on a leveraged-positions venue can cascade. A position that goes underwater past its locked margin produces residual loss that must be absorbed somewhere. The two real choices are:

(a) Revert the `applyPnL` call when residual > total balance. This makes liquidations potentially fail, which leaves the underwater position open and harms the protocol as the price moves further.

(b) Zero out the account's balance and accept the bad debt at the protocol level. The protocol now has a small accounting hole where `sum(account.totalBalance) < vault.usdcBalance` due to the absorbed loss.

We chose (b). It keeps liquidations live and predictable. The accounting hole is detectable on-chain (compare vault USDC balance against the sum of all account balances) and a future ADR will route those holes to an insurance fund (see open question §10.1 in `ARCHITECTURE.md`).

**The invariant test catches drift:** `test/invariant/VaultInvariants.t.sol::invariant_globalAccountingIdentity` runs Foundry's invariant fuzzer against deposit / withdraw / lock / release / applyPnL action sequences and asserts the global accounting identity holds even after long random sequences. The expected formula accounts for the bad-debt clamping:

```
sum(account.totalBalance) == max(0, netExternalFlow + netPnLApplied)
```

where `netPnLApplied` is the *actual* PnL absorbed (after the clamping), not the *intended* PnL passed to the function. The handler mirrors the clamping logic so the invariant accounting stays accurate; an unintended drift in the clamping logic would break the invariant immediately.

## Consequences

**Positive:**
- Vault is independently usable in v0.1 even before SettlementEngine ships. Deposit and withdraw work today against the live deployment.
- The trust surface is small (one immutable binding, one immutable USDC token, one immutable AccountManager). A reviewer can read every line of the vault in ~200 LOC.
- Negative PnL is handled deterministically with no silent reverts that could strand underwater positions.
- The invariant test makes accounting bugs loud rather than silent.

**Negative:**
- Third-party deposits enable a footgun (fund the wrong accountId). Mitigated by SDK helpers, not by the contract.
- No admin upgrade path means SettlementEngine bug-fix requires full redeploy + migration. Acceptable for a reference; forks can add governance.
- Bad-debt absorption creates a vault-level accounting hole that needs an insurance fund design (open question §10.1).

**Neutral:**
- Custom errors throughout (rather than `require` strings) for cheaper revert encoding and clearer ABI surfaces in client SDKs.
- USDC token + AccountManager bound as `immutable`. A vault that needs a different collateral token deploys a new vault rather than swapping the token mid-life.

## Alternatives considered

- **Owner-only deposits (rejected).** Adds friction for sponsored / treasury / Circle-wallet funding flows without security benefit. Withdrawal is already owner-only.
- **Proxy upgrade path for SettlementEngine swap (rejected).** Introduces the largest-possible trust surface (an admin key that can swap the contract authorized to mutate every margin balance). For a reference implementation, immutability wins. Forks can add governance.
- **Revert on bad-debt PnL (rejected).** Makes liquidations potentially fail. Worse outcome than absorbing the loss and routing it to a future insurance fund.
- **Per-account isolated USDC sub-vaults (rejected).** Would prevent ANY cross-account accounting hole at the cost of separate USDC transfers per margin operation. Massive gas overhead, not worth it for a reference.

## References

- `src/USDCVault.sol` — the implementation
- `test/USDCVault.t.sol` — unit + fuzz coverage
- `test/invariant/VaultInvariants.t.sol` — handler-driven invariant fuzz
- `test/integration/DepositWithdrawRoundtrip.t.sol` — end-to-end deposit/withdraw smoke test
- Shapeshifter CMDT USDCCollateralVault: `0x75E4FBFBA942A82F0f5CA9663571233823A71f11` (Arc Testnet) — the contrast point
- ADR 0002 — Permissionless account onboarding (why the third-party-deposit decision is consistent with the account model)
