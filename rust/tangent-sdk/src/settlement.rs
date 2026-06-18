//! High-level settlement read helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    AbiDecodeError, CallReturnBatch, DeploymentManifest, SettlementCalls, UnsignedCall,
    UnsignedCallBatchSummary, UnsignedCallSummary,
};

/// Read-side settlement state calls for one account/market pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementReadPlan {
    pub settlement_engine: Address,
    pub account_id: u128,
    pub market_id: u128,
}

/// Decoded account position in one market.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionStatus {
    pub size: i128,
    pub entry_price: u128,
    pub locked_margin: u128,
}

impl PositionStatus {
    /// True when the account has non-zero signed size in this market.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.size != 0
    }
}

/// Decoded aggregate account margin state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarginStatus {
    pub equity: i128,
    pub maintenance_margin: u128,
}

impl MarginStatus {
    /// True when non-negative equity covers the maintenance margin.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.equity >= 0 && self.equity as u128 >= self.maintenance_margin
    }

    /// True when equity after a local withdrawal would still cover maintenance.
    ///
    /// This mirrors the `SettlementEngine.validateWithdrawal` margin predicate
    /// for a decoded `marginState` snapshot. The vault can still reject for
    /// ownership or free-balance reasons.
    #[must_use]
    pub fn covers_withdrawal_amount(&self, amount: u128) -> bool {
        if self.equity < 0 {
            return false;
        }

        (self.equity as u128)
            .checked_sub(amount)
            .is_some_and(|equity_after| equity_after >= self.maintenance_margin)
    }
}

/// Decoded settlement status for one account/market pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementStatus {
    pub position: PositionStatus,
    pub margin: MarginStatus,
}

/// Local withdrawal-validation readiness from decoded settlement reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementWithdrawalReadiness {
    /// Decoded margin would remain healthy after withdrawing the amount.
    Ready,
    /// Decoded margin would fall below maintenance, or equity is negative.
    WouldBreachMaintenance,
}

/// Next read call for settlement-side withdrawal validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementWithdrawalNextStep {
    /// Run `SettlementEngine.validateWithdrawal(accountId, amount)` as an `eth_call`.
    Validate(UnsignedCall),
    /// Do not call validation until margin state changes.
    Blocked(SettlementWithdrawalReadiness),
}

/// Compact review shape for settlement-side withdrawal validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementWithdrawalSummary {
    pub settlement_engine: Address,
    pub account_id: u128,
    pub market_id: u128,
    pub amount: u128,
    pub readiness: SettlementWithdrawalReadiness,
    pub position: PositionStatus,
    #[serde(default)]
    pub has_open_position: bool,
    pub margin: MarginStatus,
    #[serde(default)]
    pub is_margin_healthy: bool,
    #[serde(default)]
    pub covers_withdrawal_amount: bool,
    pub read_summary: UnsignedCallBatchSummary,
    #[serde(default)]
    pub has_validation_call: bool,
    pub validation_call: Option<UnsignedCallSummary>,
}

impl SettlementStatus {
    /// True when the decoded position has non-zero signed size.
    #[must_use]
    pub const fn has_open_position(&self) -> bool {
        self.position.is_open()
    }

    /// True when the decoded aggregate margin state is above maintenance.
    #[must_use]
    pub const fn is_margin_healthy(&self) -> bool {
        self.margin.is_healthy()
    }

    /// True when decoded margin would remain healthy after withdrawing `amount`.
    #[must_use]
    pub fn covers_withdrawal_amount(&self, amount: u128) -> bool {
        self.margin.covers_withdrawal_amount(amount)
    }

    /// Classify whether settlement margin can cover a withdrawal.
    #[must_use]
    pub fn withdrawal_readiness(&self, amount: u128) -> SettlementWithdrawalReadiness {
        if self.covers_withdrawal_amount(amount) {
            SettlementWithdrawalReadiness::Ready
        } else {
            SettlementWithdrawalReadiness::WouldBreachMaintenance
        }
    }
}

impl SettlementReadPlan {
    #[must_use]
    pub const fn new(settlement_engine: Address, account_id: u128, market_id: u128) -> Self {
        Self {
            settlement_engine,
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
            .settlement_engine
            .map(|settlement| Self::new(settlement, account_id, market_id))
    }

    #[must_use]
    pub fn position_of_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.settlement_engine,
            data: SettlementCalls::position_of_calldata(self.account_id, self.market_id),
        }
    }

    #[must_use]
    pub fn margin_state_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.settlement_engine,
            data: SettlementCalls::margin_state_calldata(self.account_id),
        }
    }

    #[must_use]
    pub fn validate_withdrawal_call(&self, amount: u128) -> UnsignedCall {
        UnsignedCall {
            to: self.settlement_engine,
            data: SettlementCalls::validate_withdrawal_calldata(self.account_id, amount),
        }
    }

    /// Classify whether settlement validation should be called for a withdrawal.
    #[must_use]
    pub fn withdrawal_readiness(
        &self,
        status: &SettlementStatus,
        amount: u128,
    ) -> SettlementWithdrawalReadiness {
        status.withdrawal_readiness(amount)
    }

    /// Return the next settlement-side validation call a caller should make.
    #[must_use]
    pub fn withdrawal_next_step(
        &self,
        status: &SettlementStatus,
        amount: u128,
    ) -> SettlementWithdrawalNextStep {
        match self.withdrawal_readiness(status, amount) {
            SettlementWithdrawalReadiness::Ready => {
                SettlementWithdrawalNextStep::Validate(self.validate_withdrawal_call(amount))
            }
            blocked => SettlementWithdrawalNextStep::Blocked(blocked),
        }
    }

    #[must_use]
    pub fn read_summary(&self) -> UnsignedCallBatchSummary {
        let calls = self.calls();
        UnsignedCall::summarize_batch(&calls)
    }

    #[must_use]
    pub fn withdrawal_summary(
        &self,
        status: &SettlementStatus,
        amount: u128,
    ) -> SettlementWithdrawalSummary {
        let readiness = self.withdrawal_readiness(status, amount);
        let validation_call = match readiness {
            SettlementWithdrawalReadiness::Ready => {
                Some(self.validate_withdrawal_call(amount).summary())
            }
            SettlementWithdrawalReadiness::WouldBreachMaintenance => None,
        };
        SettlementWithdrawalSummary {
            settlement_engine: self.settlement_engine,
            account_id: self.account_id,
            market_id: self.market_id,
            amount,
            readiness,
            position: status.position,
            has_open_position: status.has_open_position(),
            margin: status.margin,
            is_margin_healthy: status.is_margin_healthy(),
            covers_withdrawal_amount: status.covers_withdrawal_amount(amount),
            read_summary: self.read_summary(),
            has_validation_call: validation_call.is_some(),
            validation_call,
        }
    }

    #[must_use]
    pub fn calls(&self) -> [UnsignedCall; 2] {
        [self.position_of_call(), self.margin_state_call()]
    }

    pub fn decode_position_return(
        &self,
        position_return: &[u8],
    ) -> Result<PositionStatus, AbiDecodeError> {
        let (size, entry_price, locked_margin) =
            SettlementCalls::decode_position_of_return(position_return)?;

        Ok(PositionStatus {
            size,
            entry_price,
            locked_margin,
        })
    }

    pub fn decode_margin_return(
        &self,
        margin_state_return: &[u8],
    ) -> Result<MarginStatus, AbiDecodeError> {
        let (equity, maintenance_margin) =
            SettlementCalls::decode_margin_state_return(margin_state_return)?;

        Ok(MarginStatus {
            equity,
            maintenance_margin,
        })
    }

    /// Decode a successful `validateWithdrawal(accountId, amount)` return.
    ///
    /// The Solidity function returns no value; any failed validation is a
    /// revert surfaced by the caller's transport before this decoder runs.
    pub fn decode_validate_withdrawal_return(
        &self,
        validate_withdrawal_return: &[u8],
    ) -> Result<(), AbiDecodeError> {
        SettlementCalls::decode_validate_withdrawal_return(validate_withdrawal_return)
    }

    /// Decode returns from [`Self::calls`] in the same fixed order.
    pub fn decode_returns(&self, returns: [&[u8]; 2]) -> Result<SettlementStatus, AbiDecodeError> {
        Ok(SettlementStatus {
            position: self.decode_position_return(returns[0])?,
            margin: self.decode_margin_return(returns[1])?,
        })
    }

    /// Decode a transport-returned batch from [`Self::calls`].
    pub fn decode_return_slices<T: AsRef<[u8]>>(
        &self,
        returns: &[T],
    ) -> Result<SettlementStatus, AbiDecodeError> {
        let returns = crate::abi::expect_return_count(returns, 2)?;
        self.decode_returns([returns[0], returns[1]])
    }

    /// Decode an ordered transport-returned batch from [`Self::calls`].
    pub fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<SettlementStatus, AbiDecodeError> {
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

    fn signed_word(value: i128) -> [u8; 32] {
        let mut out = if value < 0 { [0xffu8; 32] } else { [0u8; 32] };
        out[16..].copy_from_slice(&value.to_be_bytes());
        out
    }

    fn word(value: u8) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[31] = value;
        out
    }

    #[test]
    fn builds_settlement_read_calls() {
        let plan = SettlementReadPlan::new(addr(0x20), 7, 1);
        let [position, margin] = plan.calls();

        assert_eq!(position.to, addr(0x20));
        assert_eq!(
            &position.data[..4],
            &SettlementCalls::position_of_selector()
        );
        assert_eq!(hex::encode(&position.data[4..36]), format!("{:064x}", 7));
        assert_eq!(hex::encode(&position.data[36..68]), format!("{:064x}", 1));

        assert_eq!(margin.to, addr(0x20));
        assert_eq!(&margin.data[..4], &SettlementCalls::margin_state_selector());
        assert_eq!(hex::encode(&margin.data[4..36]), format!("{:064x}", 7));

        let withdrawal = plan.validate_withdrawal_call(1_000);
        assert_eq!(withdrawal.to, addr(0x20));
        assert_eq!(
            &withdrawal.data[..4],
            &SettlementCalls::validate_withdrawal_selector()
        );
    }

    #[test]
    fn current_arc_manifest_has_no_settlement_plan() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        assert_eq!(SettlementReadPlan::from_manifest(&manifest, 7, 1), None);
    }

    #[test]
    fn decodes_settlement_read_returns() {
        let plan = SettlementReadPlan::new(addr(0x20), 7, 1);

        let mut position = Vec::new();
        position.extend_from_slice(&signed_word(-7));
        position.extend_from_slice(&word(8));
        position.extend_from_slice(&word(9));

        assert_eq!(
            plan.decode_position_return(&position)
                .expect("position decodes"),
            PositionStatus {
                size: -7,
                entry_price: 8,
                locked_margin: 9,
            }
        );
        assert!(plan
            .decode_position_return(&position)
            .expect("position decodes")
            .is_open());

        let mut margin = Vec::new();
        margin.extend_from_slice(&signed_word(-7));
        margin.extend_from_slice(&word(9));

        assert_eq!(
            plan.decode_margin_return(&margin).expect("margin decodes"),
            MarginStatus {
                equity: -7,
                maintenance_margin: 9,
            }
        );
        assert!(!plan
            .decode_margin_return(&margin)
            .expect("margin decodes")
            .is_healthy());

        assert_eq!(
            plan.decode_returns([&position, &margin])
                .expect("status decodes"),
            SettlementStatus {
                position: PositionStatus {
                    size: -7,
                    entry_price: 8,
                    locked_margin: 9,
                },
                margin: MarginStatus {
                    equity: -7,
                    maintenance_margin: 9,
                },
            }
        );
        assert_eq!(
            plan.decode_return_slices(&[position.clone(), margin.clone()])
                .expect("status decodes from slices"),
            SettlementStatus {
                position: PositionStatus {
                    size: -7,
                    entry_price: 8,
                    locked_margin: 9,
                },
                margin: MarginStatus {
                    equity: -7,
                    maintenance_margin: 9,
                },
            }
        );
        let batch = CallReturnBatch::new(vec![
            crate::CallReturn::new(position.clone()),
            crate::CallReturn::new(margin.clone()),
        ]);
        assert_eq!(
            plan.decode_return_batch(&batch)
                .expect("status decodes from batch"),
            SettlementStatus {
                position: PositionStatus {
                    size: -7,
                    entry_price: 8,
                    locked_margin: 9,
                },
                margin: MarginStatus {
                    equity: -7,
                    maintenance_margin: 9,
                },
            }
        );

        assert_eq!(
            plan.decode_validate_withdrawal_return(&[])
                .expect("withdrawal validation returned cleanly"),
            ()
        );
    }

    #[test]
    fn exposes_position_and_margin_status_helpers() {
        assert!(!PositionStatus {
            size: 0,
            entry_price: 0,
            locked_margin: 0,
        }
        .is_open());

        assert!(MarginStatus {
            equity: 10,
            maintenance_margin: 10,
        }
        .is_healthy());
        assert!(MarginStatus {
            equity: 15,
            maintenance_margin: 10,
        }
        .covers_withdrawal_amount(5));
        assert!(!MarginStatus {
            equity: 15,
            maintenance_margin: 10,
        }
        .covers_withdrawal_amount(6));
        assert!(!MarginStatus {
            equity: -1,
            maintenance_margin: 0,
        }
        .covers_withdrawal_amount(0));

        assert!(!MarginStatus {
            equity: 9,
            maintenance_margin: 10,
        }
        .is_healthy());

        assert!(!MarginStatus {
            equity: -1,
            maintenance_margin: 0,
        }
        .is_healthy());
    }

    #[test]
    fn exposes_settlement_status_helpers() {
        let open_and_healthy = SettlementStatus {
            position: PositionStatus {
                size: 1,
                entry_price: 65_000,
                locked_margin: 1_000,
            },
            margin: MarginStatus {
                equity: 1_000,
                maintenance_margin: 500,
            },
        };
        assert!(open_and_healthy.has_open_position());
        assert!(open_and_healthy.is_margin_healthy());
        assert!(open_and_healthy.covers_withdrawal_amount(500));
        assert!(!open_and_healthy.covers_withdrawal_amount(501));
        assert_eq!(
            open_and_healthy.withdrawal_readiness(500),
            SettlementWithdrawalReadiness::Ready
        );

        let plan = SettlementReadPlan::new(addr(0x20), 7, 1);
        assert_eq!(
            plan.withdrawal_readiness(&open_and_healthy, 500),
            SettlementWithdrawalReadiness::Ready
        );
        assert_eq!(
            plan.withdrawal_next_step(&open_and_healthy, 500),
            SettlementWithdrawalNextStep::Validate(plan.validate_withdrawal_call(500))
        );
        assert_eq!(
            plan.withdrawal_next_step(&open_and_healthy, 501),
            SettlementWithdrawalNextStep::Blocked(
                SettlementWithdrawalReadiness::WouldBreachMaintenance
            )
        );
        let ready_summary = plan.withdrawal_summary(&open_and_healthy, 500);
        assert_eq!(ready_summary.settlement_engine, addr(0x20));
        assert_eq!(ready_summary.account_id, 7);
        assert_eq!(ready_summary.market_id, 1);
        assert_eq!(ready_summary.amount, 500);
        assert_eq!(
            ready_summary.readiness,
            SettlementWithdrawalReadiness::Ready
        );
        assert_eq!(ready_summary.position, open_and_healthy.position);
        assert!(ready_summary.has_open_position);
        assert_eq!(ready_summary.margin, open_and_healthy.margin);
        assert!(ready_summary.is_margin_healthy);
        assert!(ready_summary.covers_withdrawal_amount);
        assert_eq!(ready_summary.read_summary.len, 2);
        assert!(ready_summary.has_validation_call);
        assert_eq!(
            ready_summary
                .validation_call
                .as_ref()
                .expect("validation call summary")
                .to,
            addr(0x20)
        );
        let json = serde_json::to_string(&ready_summary).expect("summary serializes");
        let restored: SettlementWithdrawalSummary =
            serde_json::from_str(&json).expect("summary deserializes");
        assert_eq!(restored, ready_summary);
        let mut legacy_json = serde_json::to_value(&ready_summary).expect("summary value");
        let legacy_object = legacy_json.as_object_mut().expect("summary object");
        legacy_object.remove("has_open_position");
        legacy_object.remove("is_margin_healthy");
        legacy_object.remove("covers_withdrawal_amount");
        legacy_object.remove("has_validation_call");
        let legacy: SettlementWithdrawalSummary =
            serde_json::from_value(legacy_json).expect("legacy summary");
        assert!(!legacy.has_open_position);
        assert!(!legacy.is_margin_healthy);
        assert!(!legacy.covers_withdrawal_amount);
        assert!(!legacy.has_validation_call);

        let flat_and_unhealthy = SettlementStatus {
            position: PositionStatus {
                size: 0,
                entry_price: 0,
                locked_margin: 0,
            },
            margin: MarginStatus {
                equity: 499,
                maintenance_margin: 500,
            },
        };
        assert!(!flat_and_unhealthy.has_open_position());
        assert!(!flat_and_unhealthy.is_margin_healthy());
        assert_eq!(
            flat_and_unhealthy.withdrawal_readiness(0),
            SettlementWithdrawalReadiness::WouldBreachMaintenance
        );
        let blocked_summary = plan.withdrawal_summary(&flat_and_unhealthy, 0);
        assert_eq!(
            blocked_summary.readiness,
            SettlementWithdrawalReadiness::WouldBreachMaintenance
        );
        assert!(!blocked_summary.has_open_position);
        assert!(!blocked_summary.is_margin_healthy);
        assert!(!blocked_summary.covers_withdrawal_amount);
        assert!(!blocked_summary.has_validation_call);
        assert_eq!(blocked_summary.validation_call, None);
    }
}
