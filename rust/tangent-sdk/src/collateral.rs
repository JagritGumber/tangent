//! High-level collateral workflow helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    AbiDecodeError, DeploymentManifest, ERC20Calls, USDCVaultCalls, UnsignedCall, UnsignedTx,
};

/// Two-step USDC collateral deposit workflow.
///
/// Submit `approve` first through the caller's transport, wait for it to be
/// accepted/finalized according to the caller's policy, then submit `deposit`.
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

/// One-step USDC collateral withdrawal workflow.
///
/// The vault performs ownership and health checks on-chain. This plan only
/// builds the unsigned `USDCVault.withdraw(accountId, amount, to)` call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralWithdrawPlan {
    pub vault: Address,
    pub account_id: u128,
    pub amount: u128,
    pub to: Address,
}

impl CollateralWithdrawPlan {
    #[must_use]
    pub const fn new(vault: Address, account_id: u128, amount: u128, to: Address) -> Self {
        Self {
            vault,
            account_id,
            amount,
            to,
        }
    }

    #[must_use]
    pub fn from_manifest(
        manifest: &DeploymentManifest,
        account_id: u128,
        amount: u128,
        to: Address,
    ) -> Self {
        Self::new(manifest.contracts.usdc_vault, account_id, amount, to)
    }

    #[must_use]
    pub fn withdraw_tx(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.vault,
            data: USDCVaultCalls::withdraw_calldata(self.account_id, self.amount, self.to),
        }
    }

    #[must_use]
    pub fn transactions(&self) -> [UnsignedTx; 1] {
        [self.withdraw_tx()]
    }
}

/// Read-side USDC collateral status calls for one owner/account pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralStatusPlan {
    pub usdc: Address,
    pub vault: Address,
    pub owner: Address,
    pub account_id: u128,
}

/// Decoded collateral status for one owner/account pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralStatus {
    pub usdc_balance: u128,
    pub vault_allowance: u128,
    pub free_balance: u128,
    pub locked_balance: u128,
    pub total_balance: u128,
}

impl CollateralStatus {
    /// True when decoded vault balances satisfy `free + locked == total`.
    #[must_use]
    pub fn vault_balances_match(&self) -> bool {
        self.free_balance
            .checked_add(self.locked_balance)
            .is_some_and(|sum| sum == self.total_balance)
    }
}

impl CollateralStatusPlan {
    #[must_use]
    pub const fn new(usdc: Address, vault: Address, owner: Address, account_id: u128) -> Self {
        Self {
            usdc,
            vault,
            owner,
            account_id,
        }
    }

    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest, owner: Address, account_id: u128) -> Self {
        Self::new(
            manifest.constants.usdc,
            manifest.contracts.usdc_vault,
            owner,
            account_id,
        )
    }

    #[must_use]
    pub fn usdc_balance_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.usdc,
            data: ERC20Calls::balance_of_calldata(self.owner),
        }
    }

    #[must_use]
    pub fn vault_allowance_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.usdc,
            data: ERC20Calls::allowance_calldata(self.owner, self.vault),
        }
    }

    #[must_use]
    pub fn free_balance_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.vault,
            data: USDCVaultCalls::free_balance_of_calldata(self.account_id),
        }
    }

    #[must_use]
    pub fn locked_balance_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.vault,
            data: USDCVaultCalls::locked_balance_of_calldata(self.account_id),
        }
    }

    #[must_use]
    pub fn total_balance_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.vault,
            data: USDCVaultCalls::total_balance_of_calldata(self.account_id),
        }
    }

    #[must_use]
    pub fn calls(&self) -> [UnsignedCall; 5] {
        [
            self.usdc_balance_call(),
            self.vault_allowance_call(),
            self.free_balance_call(),
            self.locked_balance_call(),
            self.total_balance_call(),
        ]
    }

    /// Decode returns from [`Self::calls`] in the same fixed order.
    pub fn decode_returns(&self, returns: [&[u8]; 5]) -> Result<CollateralStatus, AbiDecodeError> {
        Ok(CollateralStatus {
            usdc_balance: ERC20Calls::decode_balance_of_return(returns[0])?,
            vault_allowance: ERC20Calls::decode_allowance_return(returns[1])?,
            free_balance: USDCVaultCalls::decode_free_balance_of_return(returns[2])?,
            locked_balance: USDCVaultCalls::decode_locked_balance_of_return(returns[3])?,
            total_balance: USDCVaultCalls::decode_total_balance_of_return(returns[4])?,
        })
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

    #[test]
    fn builds_withdraw_transaction() {
        let plan = CollateralWithdrawPlan::new(addr(0x20), 7, 1_000_000, addr(0x30));
        let [withdraw] = plan.transactions();

        assert_eq!(withdraw.to, addr(0x20));
        assert_eq!(&withdraw.data[..4], &USDCVaultCalls::withdraw_selector());
        assert_eq!(hex::encode(&withdraw.data[4..36]), format!("{:064x}", 7));
        assert_eq!(
            hex::encode(&withdraw.data[36..68]),
            format!("{:064x}", 1_000_000)
        );
        assert_eq!(&withdraw.data[80..100], addr(0x30).as_slice());
    }

    #[test]
    fn builds_withdraw_plan_from_deployment_manifest() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        let plan = CollateralWithdrawPlan::from_manifest(&manifest, 1, 500, addr(0x30));

        assert_eq!(plan.vault, manifest.contracts.usdc_vault);
        assert_eq!(plan.to, addr(0x30));
        assert_eq!(plan.withdraw_tx().to, manifest.contracts.usdc_vault);
    }

    #[test]
    fn builds_collateral_status_calls() {
        let plan = CollateralStatusPlan::new(addr(0x10), addr(0x20), addr(0x30), 7);
        let [wallet_balance, allowance, free, locked, total] = plan.calls();

        assert_eq!(wallet_balance.to, addr(0x10));
        assert_eq!(
            &wallet_balance.data[..4],
            &ERC20Calls::balance_of_selector()
        );
        assert_eq!(&wallet_balance.data[16..36], addr(0x30).as_slice());

        assert_eq!(allowance.to, addr(0x10));
        assert_eq!(&allowance.data[..4], &ERC20Calls::allowance_selector());
        assert_eq!(&allowance.data[16..36], addr(0x30).as_slice());
        assert_eq!(&allowance.data[48..68], addr(0x20).as_slice());

        assert_eq!(free.to, addr(0x20));
        assert_eq!(&free.data[..4], &USDCVaultCalls::free_balance_of_selector());
        assert_eq!(hex::encode(&free.data[4..36]), format!("{:064x}", 7));

        assert_eq!(locked.to, addr(0x20));
        assert_eq!(
            &locked.data[..4],
            &USDCVaultCalls::locked_balance_of_selector()
        );

        assert_eq!(total.to, addr(0x20));
        assert_eq!(
            &total.data[..4],
            &USDCVaultCalls::total_balance_of_selector()
        );
    }

    #[test]
    fn builds_collateral_status_plan_from_deployment_manifest() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        let plan = CollateralStatusPlan::from_manifest(&manifest, addr(0x30), 1);

        assert_eq!(plan.usdc, manifest.constants.usdc);
        assert_eq!(plan.vault, manifest.contracts.usdc_vault);
        assert_eq!(plan.owner, addr(0x30));
        assert_eq!(plan.usdc_balance_call().to, manifest.constants.usdc);
        assert_eq!(plan.free_balance_call().to, manifest.contracts.usdc_vault);
    }

    #[test]
    fn decodes_collateral_status_returns() {
        fn word(value: u8) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[31] = value;
            out
        }

        let plan = CollateralStatusPlan::new(addr(0x10), addr(0x20), addr(0x30), 7);
        let wallet = word(1);
        let allowance = word(2);
        let free = word(3);
        let locked = word(4);
        let total = word(7);

        let decoded = plan
            .decode_returns([&wallet, &allowance, &free, &locked, &total])
            .expect("status decodes");

        assert_eq!(
            decoded,
            CollateralStatus {
                usdc_balance: 1,
                vault_allowance: 2,
                free_balance: 3,
                locked_balance: 4,
                total_balance: 7,
            }
        );
        assert!(decoded.vault_balances_match());
    }

    #[test]
    fn detects_inconsistent_collateral_status_totals() {
        assert!(!CollateralStatus {
            usdc_balance: 1,
            vault_allowance: 2,
            free_balance: 3,
            locked_balance: 4,
            total_balance: 8,
        }
        .vault_balances_match());

        assert!(!CollateralStatus {
            usdc_balance: 1,
            vault_allowance: 2,
            free_balance: u128::MAX,
            locked_balance: 1,
            total_balance: 0,
        }
        .vault_balances_match());
    }
}
