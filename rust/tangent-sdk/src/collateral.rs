//! High-level collateral workflow helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::settlement::SettlementStatus;
use crate::{
    AbiDecodeError, CallReturnBatch, DeploymentManifest, ERC20Calls, USDCVaultCalls, UnsignedCall,
    UnsignedCallBatchSummary, UnsignedCallSummary, UnsignedTx,
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

    #[must_use]
    pub fn status_plan(&self, owner: Address) -> CollateralStatusPlan {
        CollateralStatusPlan::new(self.usdc, self.vault, owner, self.account_id)
    }

    #[must_use]
    pub fn status_read_summary(&self, owner: Address) -> UnsignedCallBatchSummary {
        let calls = self.status_plan(owner).calls();
        UnsignedCall::summarize_batch(&calls)
    }

    /// True when decoded wallet balance and allowance cover this deposit.
    #[must_use]
    pub const fn is_ready(&self, status: &CollateralStatus) -> bool {
        status.covers_deposit_amount(self.amount)
    }

    /// Classify whether this deposit can proceed from decoded status.
    #[must_use]
    pub const fn readiness(&self, status: &CollateralStatus) -> CollateralDepositReadiness {
        status.deposit_readiness(self.amount)
    }

    /// Return the next transaction a caller should submit for this deposit.
    #[must_use]
    pub fn next_step(&self, status: &CollateralStatus) -> CollateralDepositNextStep {
        match self.readiness(status) {
            CollateralDepositReadiness::Ready => {
                CollateralDepositNextStep::Deposit(self.deposit_tx())
            }
            CollateralDepositReadiness::InsufficientAllowance => {
                CollateralDepositNextStep::Approve(self.approve_tx())
            }
            blocked => CollateralDepositNextStep::Blocked(blocked),
        }
    }

    #[must_use]
    pub fn summary(&self, owner: Address, status: &CollateralStatus) -> CollateralDepositSummary {
        let readiness = self.readiness(status);
        let next_transaction = match readiness {
            CollateralDepositReadiness::Ready => Some(self.deposit_tx().summary()),
            CollateralDepositReadiness::InsufficientAllowance => Some(self.approve_tx().summary()),
            CollateralDepositReadiness::InsufficientWalletBalance => None,
        };
        CollateralDepositSummary {
            usdc: self.usdc,
            vault: self.vault,
            owner,
            account_id: self.account_id,
            amount: self.amount,
            readiness,
            wallet_balance: status.usdc_balance,
            vault_allowance: status.vault_allowance,
            status_read_summary: self.status_read_summary(owner),
            has_next_transaction: next_transaction.is_some(),
            next_transaction,
        }
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

    #[must_use]
    pub fn status_plan(&self, usdc: Address, owner: Address) -> CollateralStatusPlan {
        CollateralStatusPlan::new(usdc, self.vault, owner, self.account_id)
    }

    #[must_use]
    pub fn status_read_summary(&self, usdc: Address, owner: Address) -> UnsignedCallBatchSummary {
        let calls = self.status_plan(usdc, owner).calls();
        UnsignedCall::summarize_batch(&calls)
    }

    /// Classify whether this withdrawal is locally ready to submit.
    #[must_use]
    pub fn readiness(
        &self,
        status: &CollateralStatus,
        settlement: Option<&SettlementStatus>,
    ) -> WithdrawalReadiness {
        status.withdrawal_readiness(self.amount, settlement)
    }

    /// Return the next transaction a caller should submit for this withdrawal.
    #[must_use]
    pub fn next_step(
        &self,
        status: &CollateralStatus,
        settlement: Option<&SettlementStatus>,
    ) -> CollateralWithdrawNextStep {
        match self.readiness(status, settlement) {
            WithdrawalReadiness::Ready => CollateralWithdrawNextStep::Withdraw(self.withdraw_tx()),
            blocked => CollateralWithdrawNextStep::Blocked(blocked),
        }
    }

    #[must_use]
    pub fn summary(
        &self,
        usdc: Address,
        owner: Address,
        status: &CollateralStatus,
        settlement: Option<&SettlementStatus>,
    ) -> CollateralWithdrawSummary {
        let readiness = self.readiness(status, settlement);
        let next_transaction = match readiness {
            WithdrawalReadiness::Ready => Some(self.withdraw_tx().summary()),
            WithdrawalReadiness::InsufficientFreeBalance
            | WithdrawalReadiness::WouldBreachMaintenance => None,
        };
        CollateralWithdrawSummary {
            vault: self.vault,
            usdc,
            owner,
            account_id: self.account_id,
            amount: self.amount,
            to: self.to,
            readiness,
            free_balance: status.free_balance,
            locked_balance: status.locked_balance,
            total_balance: status.total_balance,
            settlement_checked: settlement.is_some(),
            status_read_summary: self.status_read_summary(usdc, owner),
            has_next_transaction: next_transaction.is_some(),
            next_transaction,
        }
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

/// Local deposit preflight result from decoded wallet balance and allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollateralDepositReadiness {
    /// Wallet balance and vault allowance cover the requested deposit.
    Ready,
    /// Wallet USDC balance is lower than the requested deposit.
    InsufficientWalletBalance,
    /// Wallet balance is sufficient, but the vault allowance is too low.
    InsufficientAllowance,
}

/// Next unsigned transaction for a collateral deposit workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollateralDepositNextStep {
    /// Submit ERC-20 approval before depositing.
    Approve(UnsignedTx),
    /// Submit the vault deposit transaction.
    Deposit(UnsignedTx),
    /// No transaction should be submitted until the blocking condition changes.
    Blocked(CollateralDepositReadiness),
}

/// Local withdrawal preflight result from decoded collateral and settlement reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WithdrawalReadiness {
    /// Free balance covers the withdrawal and any supplied settlement margin check passes.
    Ready,
    /// Decoded vault free balance is lower than the requested withdrawal.
    InsufficientFreeBalance,
    /// Settlement margin would fall below maintenance after this withdrawal.
    WouldBreachMaintenance,
}

/// Next unsigned transaction for a collateral withdrawal workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollateralWithdrawNextStep {
    /// Submit the vault withdrawal transaction.
    Withdraw(UnsignedTx),
    /// No transaction should be submitted until the blocking condition changes.
    Blocked(WithdrawalReadiness),
}

/// Compact review shape for a collateral deposit decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralDepositSummary {
    pub usdc: Address,
    pub vault: Address,
    pub owner: Address,
    pub account_id: u128,
    pub amount: u128,
    pub readiness: CollateralDepositReadiness,
    pub wallet_balance: u128,
    pub vault_allowance: u128,
    pub status_read_summary: UnsignedCallBatchSummary,
    #[serde(default)]
    pub has_next_transaction: bool,
    pub next_transaction: Option<UnsignedCallSummary>,
}

/// Compact review shape for a collateral withdrawal decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollateralWithdrawSummary {
    pub vault: Address,
    pub usdc: Address,
    pub owner: Address,
    pub account_id: u128,
    pub amount: u128,
    pub to: Address,
    pub readiness: WithdrawalReadiness,
    pub free_balance: u128,
    pub locked_balance: u128,
    pub total_balance: u128,
    pub settlement_checked: bool,
    pub status_read_summary: UnsignedCallBatchSummary,
    #[serde(default)]
    pub has_next_transaction: bool,
    pub next_transaction: Option<UnsignedCallSummary>,
}

impl CollateralStatus {
    /// True when decoded vault balances satisfy `free + locked == total`.
    #[must_use]
    pub fn vault_balances_match(&self) -> bool {
        self.free_balance
            .checked_add(self.locked_balance)
            .is_some_and(|sum| sum == self.total_balance)
    }

    /// True when wallet USDC balance and vault allowance both cover `amount`.
    #[must_use]
    pub const fn covers_deposit_amount(&self, amount: u128) -> bool {
        self.usdc_balance >= amount && self.vault_allowance >= amount
    }

    /// Classify whether wallet balance and allowance can fund `amount`.
    #[must_use]
    pub const fn deposit_readiness(&self, amount: u128) -> CollateralDepositReadiness {
        if self.usdc_balance < amount {
            return CollateralDepositReadiness::InsufficientWalletBalance;
        }
        if self.vault_allowance < amount {
            return CollateralDepositReadiness::InsufficientAllowance;
        }

        CollateralDepositReadiness::Ready
    }

    /// True when decoded free vault balance covers `amount`.
    ///
    /// This is only a local balance check. Settlement health checks can still
    /// reject a withdrawal on-chain after positions and margin are considered.
    #[must_use]
    pub const fn covers_withdrawal_amount(&self, amount: u128) -> bool {
        self.free_balance >= amount
    }

    /// Classify whether a withdrawal is locally ready to submit.
    ///
    /// Pass `None` for primitive deployments where `USDCVault.settlementEngine`
    /// is not yet bound. Pass decoded settlement status for full-stack
    /// deployments so this mirrors the vault's settlement-health hook.
    #[must_use]
    pub fn withdrawal_readiness(
        &self,
        amount: u128,
        settlement: Option<&SettlementStatus>,
    ) -> WithdrawalReadiness {
        if !self.covers_withdrawal_amount(amount) {
            return WithdrawalReadiness::InsufficientFreeBalance;
        }

        if settlement.is_some_and(|status| !status.covers_withdrawal_amount(amount)) {
            return WithdrawalReadiness::WouldBreachMaintenance;
        }

        WithdrawalReadiness::Ready
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

    /// Decode a transport-returned batch from [`Self::calls`].
    pub fn decode_return_slices<T: AsRef<[u8]>>(
        &self,
        returns: &[T],
    ) -> Result<CollateralStatus, AbiDecodeError> {
        let returns = crate::abi::expect_return_count(returns, 5)?;
        self.decode_returns([returns[0], returns[1], returns[2], returns[3], returns[4]])
    }

    /// Decode an ordered transport-returned batch from [`Self::calls`].
    pub fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<CollateralStatus, AbiDecodeError> {
        self.decode_return_slices(returns.as_returns())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::{MarginStatus, PositionStatus};

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    #[test]
    fn builds_approve_then_deposit_transactions() {
        let plan = CollateralDepositPlan::new(addr(0x10), addr(0x20), 7, 1_000_000);
        let [approve, deposit] = plan.transactions();
        let owner = addr(0x30);

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
        assert!(plan.is_ready(&CollateralStatus {
            usdc_balance: 1_000_000,
            vault_allowance: 1_000_000,
            free_balance: 0,
            locked_balance: 0,
            total_balance: 0,
        }));
        let insufficient_wallet = CollateralStatus {
            usdc_balance: 999_999,
            vault_allowance: 1_000_000,
            free_balance: 0,
            locked_balance: 0,
            total_balance: 0,
        };
        assert!(!plan.is_ready(&insufficient_wallet));
        assert_eq!(
            plan.readiness(&insufficient_wallet),
            CollateralDepositReadiness::InsufficientWalletBalance
        );
        assert_eq!(
            plan.next_step(&insufficient_wallet),
            CollateralDepositNextStep::Blocked(
                CollateralDepositReadiness::InsufficientWalletBalance
            )
        );
        let blocked_summary = plan.summary(owner, &insufficient_wallet);
        assert_eq!(
            blocked_summary.readiness,
            CollateralDepositReadiness::InsufficientWalletBalance
        );
        assert_eq!(blocked_summary.owner, owner);
        assert_eq!(blocked_summary.status_read_summary.len, 5);
        assert!(!blocked_summary.has_next_transaction);
        assert_eq!(blocked_summary.next_transaction, None);

        let insufficient_allowance = CollateralStatus {
            usdc_balance: 1_000_000,
            vault_allowance: 999_999,
            free_balance: 0,
            locked_balance: 0,
            total_balance: 0,
        };
        assert_eq!(
            plan.readiness(&insufficient_allowance),
            CollateralDepositReadiness::InsufficientAllowance
        );
        assert_eq!(
            plan.next_step(&insufficient_allowance),
            CollateralDepositNextStep::Approve(plan.approve_tx())
        );
        let approval_summary = plan.summary(owner, &insufficient_allowance);
        assert_eq!(
            approval_summary.readiness,
            CollateralDepositReadiness::InsufficientAllowance
        );
        assert_eq!(
            approval_summary
                .next_transaction
                .as_ref()
                .expect("approval tx summary")
                .to,
            addr(0x10)
        );
        assert!(approval_summary.has_next_transaction);
        assert_eq!(
            plan.status_plan(owner),
            CollateralStatusPlan::new(addr(0x10), addr(0x20), owner, 7)
        );
        assert_eq!(plan.status_read_summary(owner).len, 5);

        let ready = CollateralStatus {
            usdc_balance: 1_000_000,
            vault_allowance: 1_000_000,
            free_balance: 0,
            locked_balance: 0,
            total_balance: 0,
        };
        assert_eq!(plan.readiness(&ready), CollateralDepositReadiness::Ready);
        assert_eq!(
            plan.next_step(&ready),
            CollateralDepositNextStep::Deposit(plan.deposit_tx())
        );
        let ready_summary = plan.summary(owner, &ready);
        assert_eq!(ready_summary.readiness, CollateralDepositReadiness::Ready);
        assert_eq!(ready_summary.wallet_balance, 1_000_000);
        assert_eq!(ready_summary.vault_allowance, 1_000_000);
        assert!(ready_summary.has_next_transaction);
        assert_eq!(
            ready_summary
                .next_transaction
                .as_ref()
                .expect("deposit tx summary")
                .to,
            addr(0x20)
        );
        let json = serde_json::to_string(&ready_summary).expect("deposit summary serializes");
        let restored: CollateralDepositSummary =
            serde_json::from_str(&json).expect("deposit summary deserializes");
        assert_eq!(restored, ready_summary);
        let mut legacy_json = serde_json::to_value(&ready_summary).expect("deposit summary value");
        let legacy_object = legacy_json.as_object_mut().expect("deposit summary object");
        legacy_object.remove("has_next_transaction");
        let legacy: CollateralDepositSummary =
            serde_json::from_value(legacy_json).expect("legacy deposit summary");
        assert!(!legacy.has_next_transaction);
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
        let usdc = addr(0x10);
        let owner = addr(0x40);

        assert_eq!(withdraw.to, addr(0x20));
        assert_eq!(&withdraw.data[..4], &USDCVaultCalls::withdraw_selector());
        assert_eq!(hex::encode(&withdraw.data[4..36]), format!("{:064x}", 7));
        assert_eq!(
            hex::encode(&withdraw.data[36..68]),
            format!("{:064x}", 1_000_000)
        );
        assert_eq!(&withdraw.data[80..100], addr(0x30).as_slice());
        assert_eq!(
            plan.readiness(
                &CollateralStatus {
                    usdc_balance: 0,
                    vault_allowance: 0,
                    free_balance: 1_000_000,
                    locked_balance: 0,
                    total_balance: 1_000_000,
                },
                None,
            ),
            WithdrawalReadiness::Ready
        );
        assert_eq!(
            plan.next_step(
                &CollateralStatus {
                    usdc_balance: 0,
                    vault_allowance: 0,
                    free_balance: 1_000_000,
                    locked_balance: 0,
                    total_balance: 1_000_000,
                },
                None,
            ),
            CollateralWithdrawNextStep::Withdraw(plan.withdraw_tx())
        );
        let ready_status = CollateralStatus {
            usdc_balance: 0,
            vault_allowance: 0,
            free_balance: 1_000_000,
            locked_balance: 0,
            total_balance: 1_000_000,
        };
        let ready_summary = plan.summary(usdc, owner, &ready_status, None);
        assert_eq!(ready_summary.readiness, WithdrawalReadiness::Ready);
        assert!(!ready_summary.settlement_checked);
        assert_eq!(ready_summary.status_read_summary.len, 5);
        assert!(ready_summary.has_next_transaction);
        assert_eq!(
            ready_summary
                .next_transaction
                .as_ref()
                .expect("withdraw tx summary")
                .to,
            addr(0x20)
        );
        assert_eq!(
            plan.status_plan(usdc, owner),
            CollateralStatusPlan::new(usdc, addr(0x20), owner, 7)
        );
        assert_eq!(
            plan.next_step(
                &CollateralStatus {
                    usdc_balance: 0,
                    vault_allowance: 0,
                    free_balance: 999_999,
                    locked_balance: 0,
                    total_balance: 999_999,
                },
                None,
            ),
            CollateralWithdrawNextStep::Blocked(WithdrawalReadiness::InsufficientFreeBalance)
        );
        let blocked_status = CollateralStatus {
            usdc_balance: 0,
            vault_allowance: 0,
            free_balance: 999_999,
            locked_balance: 0,
            total_balance: 999_999,
        };
        let blocked_summary = plan.summary(usdc, owner, &blocked_status, None);
        assert_eq!(
            blocked_summary.readiness,
            WithdrawalReadiness::InsufficientFreeBalance
        );
        assert!(!blocked_summary.has_next_transaction);
        assert_eq!(blocked_summary.next_transaction, None);
        let json = serde_json::to_string(&ready_summary).expect("withdraw summary serializes");
        let restored: CollateralWithdrawSummary =
            serde_json::from_str(&json).expect("withdraw summary deserializes");
        assert_eq!(restored, ready_summary);
        let mut legacy_json = serde_json::to_value(&ready_summary).expect("withdraw summary value");
        let legacy_object = legacy_json
            .as_object_mut()
            .expect("withdraw summary object");
        legacy_object.remove("has_next_transaction");
        let legacy: CollateralWithdrawSummary =
            serde_json::from_value(legacy_json).expect("legacy withdraw summary");
        assert!(!legacy.has_next_transaction);
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
            plan.decode_return_slices(&[
                wallet.to_vec(),
                allowance.to_vec(),
                free.to_vec(),
                locked.to_vec(),
                total.to_vec()
            ])
            .expect("status decodes from slices"),
            decoded
        );
        let batch = CallReturnBatch::new(vec![
            crate::CallReturn::new(wallet.to_vec()),
            crate::CallReturn::new(allowance.to_vec()),
            crate::CallReturn::new(free.to_vec()),
            crate::CallReturn::new(locked.to_vec()),
            crate::CallReturn::new(total.to_vec()),
        ]);
        assert_eq!(
            plan.decode_return_batch(&batch)
                .expect("status decodes from batch"),
            decoded
        );

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
        assert!(decoded.covers_deposit_amount(1));
        assert!(!decoded.covers_deposit_amount(2));
        assert_eq!(
            decoded.deposit_readiness(1),
            CollateralDepositReadiness::Ready
        );
        assert_eq!(
            decoded.deposit_readiness(2),
            CollateralDepositReadiness::InsufficientWalletBalance
        );
        assert_eq!(
            decoded.deposit_readiness(3),
            CollateralDepositReadiness::InsufficientWalletBalance
        );
        assert_eq!(
            CollateralStatus {
                usdc_balance: 3,
                vault_allowance: 2,
                free_balance: 0,
                locked_balance: 0,
                total_balance: 0,
            }
            .deposit_readiness(3),
            CollateralDepositReadiness::InsufficientAllowance
        );
        assert!(decoded.covers_withdrawal_amount(3));
        assert!(!decoded.covers_withdrawal_amount(4));
        assert_eq!(
            decoded.withdrawal_readiness(3, None),
            WithdrawalReadiness::Ready
        );
        assert_eq!(
            decoded.withdrawal_readiness(4, None),
            WithdrawalReadiness::InsufficientFreeBalance
        );
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

    #[test]
    fn classifies_withdrawal_readiness_with_settlement_status() {
        let collateral = CollateralStatus {
            usdc_balance: 0,
            vault_allowance: 0,
            free_balance: 10,
            locked_balance: 5,
            total_balance: 15,
        };
        let healthy_margin = SettlementStatus {
            position: PositionStatus {
                size: 1,
                entry_price: 65_000,
                locked_margin: 5,
            },
            margin: MarginStatus {
                equity: 15,
                maintenance_margin: 10,
            },
        };

        assert_eq!(
            collateral.withdrawal_readiness(5, Some(&healthy_margin)),
            WithdrawalReadiness::Ready
        );
        assert_eq!(
            collateral.withdrawal_readiness(6, Some(&healthy_margin)),
            WithdrawalReadiness::WouldBreachMaintenance
        );
        assert_eq!(
            collateral.withdrawal_readiness(11, Some(&healthy_margin)),
            WithdrawalReadiness::InsufficientFreeBalance
        );
    }
}
