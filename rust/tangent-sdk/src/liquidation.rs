//! High-level liquidation read helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    AbiDecodeError, CallReturnBatch, DeploymentManifest, LiquidationKeeperCalls, UnsignedCall,
    UnsignedCallBatchSummary, UnsignedCallSummary, UnsignedTx,
};

/// Read-side liquidation state calls for one account/market pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationReadPlan {
    pub liquidation_keeper: Address,
    pub account_id: u128,
    pub market_id: u128,
}

/// Compact review shape for liquidation read and transaction planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationReadPlanSummary {
    pub liquidation_keeper: Address,
    pub account_id: u128,
    pub market_id: u128,
    pub read_summary: UnsignedCallBatchSummary,
    pub liquidation_transaction: UnsignedCallSummary,
}

/// Decoded liquidation state for one account/market pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationStatus {
    pub is_liquidatable: bool,
    pub equity: i128,
    pub maintenance_margin: u128,
}

/// Local liquidation transaction readiness from decoded keeper reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiquidationReadiness {
    /// `LiquidationKeeper.isLiquidatable` returned true; `liquidate` is worth submitting.
    Ready,
    /// The contract predicate says this account/market is not liquidatable.
    NotLiquidatable,
}

/// Next unsigned transaction for a permissionless liquidation workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiquidationNextStep {
    /// Submit `LiquidationKeeper.liquidate(accountId, marketId)`.
    Liquidate(UnsignedTx),
    /// No liquidation transaction should be submitted for this decoded status.
    Blocked(LiquidationReadiness),
}

/// Compact review shape for a decoded liquidation decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationStatusSummary {
    pub liquidation_keeper: Address,
    pub account_id: u128,
    pub market_id: u128,
    pub readiness: LiquidationReadiness,
    pub is_liquidatable: bool,
    pub is_below_maintenance: bool,
    pub equity: i128,
    pub maintenance_margin: u128,
    pub read_summary: UnsignedCallBatchSummary,
    #[serde(default)]
    pub has_next_transaction: bool,
    pub next_transaction: Option<UnsignedCallSummary>,
}

impl LiquidationStatus {
    /// True when decoded equity is below the decoded maintenance margin.
    ///
    /// `is_liquidatable` remains the contract predicate. This helper only
    /// exposes the simple margin comparison for clients and keeper logs.
    #[must_use]
    pub const fn is_below_maintenance(&self) -> bool {
        if self.equity < 0 {
            return true;
        }

        (self.equity as u128) < self.maintenance_margin
    }

    /// Classify whether a liquidation tx should be submitted.
    #[must_use]
    pub const fn readiness(&self) -> LiquidationReadiness {
        if self.is_liquidatable {
            LiquidationReadiness::Ready
        } else {
            LiquidationReadiness::NotLiquidatable
        }
    }
}

/// Errors that can occur while decoding batched liquidation reads.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LiquidationDecodeError {
    #[error(transparent)]
    Abi(#[from] AbiDecodeError),
    #[error(
        "inconsistent liquidation status: isLiquidatable returned {is_liquidatable_return}, liquidationState returned {liquidation_state_return}"
    )]
    InconsistentLiquidationFlag {
        is_liquidatable_return: bool,
        liquidation_state_return: bool,
    },
}

impl LiquidationReadPlan {
    #[must_use]
    pub const fn new(liquidation_keeper: Address, account_id: u128, market_id: u128) -> Self {
        Self {
            liquidation_keeper,
            account_id,
            market_id,
        }
    }

    #[must_use]
    pub fn from_manifest(
        manifest: &DeploymentManifest,
        account_id: u128,
        market_id: u128,
    ) -> Option<Self> {
        manifest
            .contracts
            .liquidation_keeper
            .map(|keeper| Self::new(keeper, account_id, market_id))
    }

    #[must_use]
    pub fn is_liquidatable_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.liquidation_keeper,
            data: LiquidationKeeperCalls::is_liquidatable_calldata(self.account_id, self.market_id),
        }
    }

    #[must_use]
    pub fn liquidation_state_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.liquidation_keeper,
            data: LiquidationKeeperCalls::liquidation_state_calldata(
                self.account_id,
                self.market_id,
            ),
        }
    }

    #[must_use]
    pub fn calls(&self) -> [UnsignedCall; 2] {
        [self.is_liquidatable_call(), self.liquidation_state_call()]
    }

    #[must_use]
    pub fn read_summary(&self) -> UnsignedCallBatchSummary {
        let calls = self.calls();
        UnsignedCall::summarize_batch(&calls)
    }

    /// Build the unsigned permissionless liquidation transaction.
    ///
    /// This only encodes `LiquidationKeeper.liquidate(accountId, marketId)`;
    /// callers still choose transport, signer, gas, and profitability policy.
    #[must_use]
    pub fn liquidate_tx(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.liquidation_keeper,
            data: LiquidationKeeperCalls::liquidate_calldata(self.account_id, self.market_id),
        }
    }

    #[must_use]
    pub fn transactions(&self) -> [UnsignedTx; 1] {
        [self.liquidate_tx()]
    }

    #[must_use]
    pub fn summary(&self) -> LiquidationReadPlanSummary {
        LiquidationReadPlanSummary {
            liquidation_keeper: self.liquidation_keeper,
            account_id: self.account_id,
            market_id: self.market_id,
            read_summary: self.read_summary(),
            liquidation_transaction: self.liquidate_tx().summary(),
        }
    }

    /// Classify whether this plan's decoded status is ready for liquidation.
    #[must_use]
    pub const fn readiness(&self, status: &LiquidationStatus) -> LiquidationReadiness {
        status.readiness()
    }

    /// Return the next transaction a caller should submit for this liquidation.
    #[must_use]
    pub fn next_step(&self, status: &LiquidationStatus) -> LiquidationNextStep {
        match self.readiness(status) {
            LiquidationReadiness::Ready => LiquidationNextStep::Liquidate(self.liquidate_tx()),
            blocked => LiquidationNextStep::Blocked(blocked),
        }
    }

    #[must_use]
    pub fn status_summary(&self, status: &LiquidationStatus) -> LiquidationStatusSummary {
        let readiness = self.readiness(status);
        let next_transaction = match readiness {
            LiquidationReadiness::Ready => Some(self.liquidate_tx().summary()),
            LiquidationReadiness::NotLiquidatable => None,
        };
        LiquidationStatusSummary {
            liquidation_keeper: self.liquidation_keeper,
            account_id: self.account_id,
            market_id: self.market_id,
            readiness,
            is_liquidatable: status.is_liquidatable,
            is_below_maintenance: status.is_below_maintenance(),
            equity: status.equity,
            maintenance_margin: status.maintenance_margin,
            read_summary: self.read_summary(),
            has_next_transaction: next_transaction.is_some(),
            next_transaction,
        }
    }

    pub fn decode_state_return(
        &self,
        liquidation_state_return: &[u8],
    ) -> Result<LiquidationStatus, AbiDecodeError> {
        let (is_liquidatable, equity, maintenance_margin) =
            LiquidationKeeperCalls::decode_liquidation_state_return(liquidation_state_return)?;

        Ok(LiquidationStatus {
            is_liquidatable,
            equity,
            maintenance_margin,
        })
    }

    /// Decode returns from [`Self::calls`] in the same fixed order.
    ///
    /// Both calls expose the liquidation predicate. This validates that the
    /// standalone `isLiquidatable` return agrees with the richer
    /// `liquidationState` tuple so callers catch misordered or stale batches.
    pub fn decode_returns(
        &self,
        returns: [&[u8]; 2],
    ) -> Result<LiquidationStatus, LiquidationDecodeError> {
        let is_liquidatable = LiquidationKeeperCalls::decode_is_liquidatable_return(returns[0])?;
        let status = self.decode_state_return(returns[1])?;

        if is_liquidatable != status.is_liquidatable {
            return Err(LiquidationDecodeError::InconsistentLiquidationFlag {
                is_liquidatable_return: is_liquidatable,
                liquidation_state_return: status.is_liquidatable,
            });
        }

        Ok(status)
    }

    /// Decode a transport-returned batch from [`Self::calls`].
    pub fn decode_return_slices<T: AsRef<[u8]>>(
        &self,
        returns: &[T],
    ) -> Result<LiquidationStatus, LiquidationDecodeError> {
        let returns = crate::abi::expect_return_count(returns, 2)?;
        self.decode_returns([returns[0], returns[1]])
    }

    /// Decode an ordered transport-returned batch from [`Self::calls`].
    pub fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<LiquidationStatus, LiquidationDecodeError> {
        self.decode_return_slices(returns.as_returns())
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use super::*;

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    #[test]
    fn builds_liquidation_read_calls_and_tx() {
        let plan = LiquidationReadPlan::new(addr(0x20), 7, 1);
        let [is_liquidatable, state] = plan.calls();

        assert_eq!(is_liquidatable.to, addr(0x20));
        assert_eq!(
            &is_liquidatable.data[..4],
            &LiquidationKeeperCalls::is_liquidatable_selector()
        );
        assert_eq!(
            hex::encode(&is_liquidatable.data[4..36]),
            format!("{:064x}", 7)
        );
        assert_eq!(
            hex::encode(&is_liquidatable.data[36..68]),
            format!("{:064x}", 1)
        );

        assert_eq!(state.to, addr(0x20));
        assert_eq!(
            &state.data[..4],
            &LiquidationKeeperCalls::liquidation_state_selector()
        );

        let liquidate = plan.liquidate_tx();
        let [liquidate_from_batch] = plan.transactions();
        assert_eq!(liquidate.to, addr(0x20));
        assert_eq!(liquidate_from_batch, liquidate);
        assert_eq!(
            &liquidate.data[..4],
            &LiquidationKeeperCalls::liquidate_selector()
        );
        let summary = plan.summary();
        assert_eq!(summary.liquidation_keeper, addr(0x20));
        assert_eq!(summary.account_id, 7);
        assert_eq!(summary.market_id, 1);
        assert_eq!(summary.read_summary.len, 2);
        assert_eq!(summary.liquidation_transaction.to, addr(0x20));
        let json = serde_json::to_string(&summary).expect("summary serializes");
        let restored: LiquidationReadPlanSummary =
            serde_json::from_str(&json).expect("summary deserializes");
        assert_eq!(restored, summary);
    }

    #[test]
    fn current_arc_manifest_has_no_liquidation_plan() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        assert_eq!(LiquidationReadPlan::from_manifest(&manifest, 7, 1), None);
    }

    #[test]
    fn decodes_liquidation_state_return() {
        let plan = LiquidationReadPlan::new(addr(0x20), 7, 1);
        let mut yes = [0u8; 32];
        yes[31] = 1;

        let mut equity = [0xffu8; 32];
        equity[16..].copy_from_slice(&(-7i128).to_be_bytes());

        let mut maintenance = [0u8; 32];
        maintenance[31] = 9;

        let mut data = Vec::new();
        data.extend_from_slice(&yes);
        data.extend_from_slice(&equity);
        data.extend_from_slice(&maintenance);

        assert_eq!(
            plan.decode_state_return(&data).expect("state decodes"),
            LiquidationStatus {
                is_liquidatable: true,
                equity: -7,
                maintenance_margin: 9,
            }
        );
        assert!(plan
            .decode_state_return(&data)
            .expect("state decodes")
            .is_below_maintenance());
        assert_eq!(
            plan.decode_state_return(&data)
                .expect("state decodes")
                .readiness(),
            LiquidationReadiness::Ready
        );
        assert_eq!(
            plan.next_step(&plan.decode_state_return(&data).expect("state decodes")),
            LiquidationNextStep::Liquidate(plan.liquidate_tx())
        );

        let decoded = plan
            .decode_returns([&yes, &data])
            .expect("batched status decodes");
        assert_eq!(
            plan.decode_return_slices(&[yes.to_vec(), data.clone()])
                .expect("status decodes from slices"),
            decoded
        );
        let batch = CallReturnBatch::new(vec![
            crate::CallReturn::new(yes.to_vec()),
            crate::CallReturn::new(data.clone()),
        ]);
        assert_eq!(
            plan.decode_return_batch(&batch)
                .expect("status decodes from batch"),
            decoded
        );
        assert_eq!(
            decoded,
            LiquidationStatus {
                is_liquidatable: true,
                equity: -7,
                maintenance_margin: 9,
            }
        );
        assert_eq!(plan.readiness(&decoded), LiquidationReadiness::Ready);
        let summary = plan.status_summary(&decoded);
        assert_eq!(summary.liquidation_keeper, addr(0x20));
        assert_eq!(summary.account_id, 7);
        assert_eq!(summary.market_id, 1);
        assert_eq!(summary.readiness, LiquidationReadiness::Ready);
        assert!(summary.is_liquidatable);
        assert!(summary.is_below_maintenance);
        assert_eq!(summary.equity, -7);
        assert_eq!(summary.maintenance_margin, 9);
        assert_eq!(summary.read_summary.len, 2);
        assert!(summary.has_next_transaction);
        assert_eq!(
            summary
                .next_transaction
                .as_ref()
                .expect("liquidation tx summary")
                .to,
            addr(0x20)
        );
        let json = serde_json::to_string(&summary).expect("status summary serializes");
        let restored: LiquidationStatusSummary =
            serde_json::from_str(&json).expect("status summary deserializes");
        assert_eq!(restored, summary);
        let mut legacy_json = serde_json::to_value(&summary).expect("status summary value");
        let legacy_object = legacy_json.as_object_mut().expect("status summary object");
        legacy_object.remove("has_next_transaction");
        let legacy: LiquidationStatusSummary =
            serde_json::from_value(legacy_json).expect("legacy status summary");
        assert!(!legacy.has_next_transaction);
    }

    #[test]
    fn exposes_liquidation_margin_status_helper() {
        assert!(LiquidationStatus {
            is_liquidatable: true,
            equity: -1,
            maintenance_margin: 0,
        }
        .is_below_maintenance());

        assert!(LiquidationStatus {
            is_liquidatable: true,
            equity: 9,
            maintenance_margin: 10,
        }
        .is_below_maintenance());

        assert!(!LiquidationStatus {
            is_liquidatable: false,
            equity: 10,
            maintenance_margin: 10,
        }
        .is_below_maintenance());
        assert_eq!(
            LiquidationStatus {
                is_liquidatable: false,
                equity: 9,
                maintenance_margin: 10,
            }
            .readiness(),
            LiquidationReadiness::NotLiquidatable
        );
        let plan = LiquidationReadPlan::new(addr(0x20), 7, 1);
        let status = LiquidationStatus {
            is_liquidatable: false,
            equity: 9,
            maintenance_margin: 10,
        };
        assert_eq!(
            plan.next_step(&status),
            LiquidationNextStep::Blocked(LiquidationReadiness::NotLiquidatable)
        );
        let summary = plan.status_summary(&status);
        assert_eq!(summary.readiness, LiquidationReadiness::NotLiquidatable);
        assert!(!summary.is_liquidatable);
        assert!(summary.is_below_maintenance);
        assert!(!summary.has_next_transaction);
        assert_eq!(summary.next_transaction, None);
    }

    #[test]
    fn rejects_inconsistent_liquidation_read_returns() {
        let plan = LiquidationReadPlan::new(addr(0x20), 7, 1);
        let no = [0u8; 32];
        let mut yes = [0u8; 32];
        yes[31] = 1;
        let equity = [0u8; 32];
        let maintenance = [0u8; 32];

        let mut state = Vec::new();
        state.extend_from_slice(&yes);
        state.extend_from_slice(&equity);
        state.extend_from_slice(&maintenance);

        assert_eq!(
            plan.decode_returns([&no, &state])
                .expect_err("mismatched flags"),
            LiquidationDecodeError::InconsistentLiquidationFlag {
                is_liquidatable_return: false,
                liquidation_state_return: true,
            }
        );
    }
}
