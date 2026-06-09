//! High-level settlement read helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{AbiDecodeError, DeploymentManifest, SettlementCalls, UnsignedCall};

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
}

/// Decoded settlement status for one account/market pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementStatus {
    pub position: PositionStatus,
    pub margin: MarginStatus,
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
    }
}
