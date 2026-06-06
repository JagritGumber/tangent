//! High-level collateral workflow helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{DeploymentManifest, ERC20Calls, USDCVaultCalls};

/// Unsigned transaction input produced by the SDK.
///
/// The SDK deliberately does not choose a transport, signer, nonce, gas limit,
/// or fee policy. Callers can pass these fields into Alloy, Circle Dev Wallets,
/// a relayer, or their own transaction builder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedTx {
    pub to: Address,
    pub data: Vec<u8>,
}

impl UnsignedTx {
    #[must_use]
    pub fn data_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }
}

/// Two-step USDC collateral deposit workflow.
///
/// Broadcast `approve` first, wait for it to be accepted/finalized according to
/// the caller's policy, then broadcast `deposit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralDepositPlan {
    pub usdc: Address,
    pub vault: Address,
    pub account_id: u128,
    pub amount: u128,
}

impl CollateralDepositPlan {
    #[must_use]
    pub const fn new(usdc: Address, vault: Address, account_id: u128, amount: u128) -> Self {
        Self {
            usdc,
            vault,
            account_id,
            amount,
        }
    }

    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest, account_id: u128, amount: u128) -> Self {
        Self::new(
            manifest.constants.usdc,
            manifest.contracts.usdc_vault,
            account_id,
            amount,
        )
    }

    #[must_use]
    pub fn approve_tx(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.usdc,
            data: ERC20Calls::approve_calldata(self.vault, self.amount),
        }
    }

    #[must_use]
    pub fn deposit_tx(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.vault,
            data: USDCVaultCalls::deposit_calldata(self.account_id, self.amount),
        }
    }

    #[must_use]
    pub fn transactions(&self) -> [UnsignedTx; 2] {
        [self.approve_tx(), self.deposit_tx()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    #[test]
    fn builds_approve_then_deposit_transactions() {
        let plan = CollateralDepositPlan::new(addr(0x10), addr(0x20), 7, 1_000_000);
        let [approve, deposit] = plan.transactions();

        assert_eq!(approve.to, addr(0x10));
        assert_eq!(&approve.data[..4], &ERC20Calls::approve_selector());
        assert_eq!(&approve.data[16..36], addr(0x20).as_slice());
        assert_eq!(
            &approve.data[36..68],
            &USDCVaultCalls::deposit_calldata(0, 1_000_000)[36..68]
        );

        assert_eq!(deposit.to, addr(0x20));
        assert_eq!(&deposit.data[..4], &USDCVaultCalls::deposit_selector());
        assert_eq!(hex::encode(&deposit.data[4..36]), format!("{:064x}", 7));
        assert_eq!(
            hex::encode(&deposit.data[36..68]),
            format!("{:064x}", 1_000_000)
        );
    }

    #[test]
    fn builds_plan_from_deployment_manifest() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        let plan = CollateralDepositPlan::from_manifest(&manifest, 1, 500);

        assert_eq!(plan.usdc, manifest.constants.usdc);
        assert_eq!(plan.vault, manifest.contracts.usdc_vault);
        assert_eq!(plan.approve_tx().to, manifest.constants.usdc);
        assert_eq!(plan.deposit_tx().to, manifest.contracts.usdc_vault);
    }
}
