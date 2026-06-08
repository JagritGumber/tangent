//! High-level liquidation read helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{AbiDecodeError, DeploymentManifest, LiquidationKeeperCalls, UnsignedCall, UnsignedTx};

/// Read-side liquidation state calls for one account/market pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationReadPlan {
    pub liquidation_keeper: Address,
    pub account_id: u128,
    pub market_id: u128,
}

/// Decoded liquidation state for one account/market pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationStatus {
    pub is_liquidatable: bool,
    pub equity: i128,
    pub maintenance_margin: u128,
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
        assert_eq!(liquidate.to, addr(0x20));
        assert_eq!(
            &liquidate.data[..4],
            &LiquidationKeeperCalls::liquidate_selector()
        );
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

        assert_eq!(
            plan.decode_returns([&yes, &data])
                .expect("batched status decodes"),
            LiquidationStatus {
                is_liquidatable: true,
                equity: -7,
                maintenance_margin: 9,
            }
        );
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
