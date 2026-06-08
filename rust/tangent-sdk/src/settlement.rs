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

/// Decoded aggregate account margin state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarginStatus {
    pub equity: i128,
    pub maintenance_margin: u128,
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
    }
}
