//! High-level account onboarding helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    AbiDecodeError, AccountManagerCalls, CallReturnBatch, DeploymentManifest, UnsignedCall,
    UnsignedCallBatchSummary, UnsignedCallSummary, UnsignedTx,
};

/// Permissionless Tangent account onboarding workflow.
///
/// Submit `register_tx` from the owner address through the caller's transport,
/// then either decode the `registerAccount()` return value or use
/// `account_id_of_call()` as an `eth_call` to recover the registered account id
/// later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountOnboardingPlan {
    pub account_manager: Address,
    pub owner: Address,
}

/// Read-side account status calls for one owner/account pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountStatusPlan {
    pub account_manager: Address,
    pub owner: Address,
    pub account_id: u128,
}

/// Decoded account status for one owner/account pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountStatus {
    pub owner_of_account: Address,
    pub account_id_of_owner: u128,
    pub total_accounts: u128,
}

/// Local classification for decoded owner/account binding reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountBindingStatus {
    /// The expected owner/account pair is registered and both lookup directions match.
    Registered,
    /// `accountIdOf(owner)` returned zero, so the owner should register first.
    OwnerUnregistered,
    /// The owner is already bound to a different account id.
    OwnerRegisteredToDifferentAccount { actual_account_id: u128 },
    /// The requested account id is zero or above `totalAccounts`.
    AccountIdNotRegistered { total_accounts: u128 },
    /// `ownerOf(accountId)` returned a different owner.
    AccountOwnedByDifferentOwner { actual_owner: Address },
}

/// Next action for account onboarding from decoded status reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountOnboardingNextStep {
    /// Submit `registerAccount()`.
    Register(UnsignedTx),
    /// The owner is already registered to this account id.
    UseAccount(u128),
    /// Do not submit onboarding until the mismatch is resolved.
    Blocked(AccountBindingStatus),
}

/// Compact next-action category for account onboarding summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountOnboardingAction {
    Register,
    UseAccount,
    Blocked,
}

/// Compact review shape for an account onboarding decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountOnboardingSummary {
    pub account_manager: Address,
    pub owner: Address,
    pub expected_account_id: u128,
    pub binding_status: AccountBindingStatus,
    pub action: AccountOnboardingAction,
    #[serde(default)]
    pub should_register: bool,
    #[serde(default)]
    pub can_use_account: bool,
    #[serde(default)]
    pub is_blocked: bool,
    pub registered_account_id: Option<u128>,
    pub total_accounts: u128,
    pub status_read_summary: UnsignedCallBatchSummary,
    #[serde(default)]
    pub has_register_transaction: bool,
    pub register_transaction: Option<UnsignedCallSummary>,
}

impl AccountStatus {
    /// True when both account lookup directions match the expected binding.
    #[must_use]
    pub fn matches(&self, owner: Address, account_id: u128) -> bool {
        self.owner_of_account == owner && self.account_id_of_owner == account_id
    }

    /// True when the decoded binding matches and the account id is registered.
    ///
    /// Tangent account ids start at 1 and `accountIdOf(unknownOwner)` returns
    /// zero, so a registered account must be non-zero and no larger than the
    /// decoded `totalAccounts` value.
    #[must_use]
    pub fn is_registered_binding(&self, owner: Address, account_id: u128) -> bool {
        self.matches(owner, account_id) && account_id != 0 && account_id <= self.total_accounts
    }

    /// Classify decoded owner/account binding reads against an expected pair.
    #[must_use]
    pub fn binding_status(&self, owner: Address, account_id: u128) -> AccountBindingStatus {
        if self.is_registered_binding(owner, account_id) {
            return AccountBindingStatus::Registered;
        }
        if self.account_id_of_owner == 0 {
            return AccountBindingStatus::OwnerUnregistered;
        }
        if self.account_id_of_owner != account_id {
            return AccountBindingStatus::OwnerRegisteredToDifferentAccount {
                actual_account_id: self.account_id_of_owner,
            };
        }
        if account_id == 0 || account_id > self.total_accounts {
            return AccountBindingStatus::AccountIdNotRegistered {
                total_accounts: self.total_accounts,
            };
        }

        AccountBindingStatus::AccountOwnedByDifferentOwner {
            actual_owner: self.owner_of_account,
        }
    }
}

impl AccountOnboardingPlan {
    #[must_use]
    pub const fn new(account_manager: Address, owner: Address) -> Self {
        Self {
            account_manager,
            owner,
        }
    }

    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest, owner: Address) -> Self {
        Self::new(manifest.contracts.account_manager, owner)
    }

    #[must_use]
    pub fn register_tx(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.account_manager,
            data: AccountManagerCalls::register_account_calldata(),
        }
    }

    #[must_use]
    pub fn transactions(&self) -> [UnsignedTx; 1] {
        [self.register_tx()]
    }

    #[must_use]
    pub fn account_id_of_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.account_manager,
            data: AccountManagerCalls::account_id_of_calldata(self.owner),
        }
    }

    pub fn decode_register_return(&self, register_return: &[u8]) -> Result<u128, AbiDecodeError> {
        AccountManagerCalls::decode_register_account_return(register_return)
    }

    #[must_use]
    pub const fn status_plan(&self, expected_account_id: u128) -> AccountStatusPlan {
        AccountStatusPlan::new(self.account_manager, self.owner, expected_account_id)
    }

    #[must_use]
    pub fn status_read_summary(&self, expected_account_id: u128) -> UnsignedCallBatchSummary {
        let calls = self.status_plan(expected_account_id).calls();
        UnsignedCall::summarize_batch(&calls)
    }

    /// Decide whether this owner should register or use an existing account.
    #[must_use]
    pub fn next_step(
        &self,
        status: &AccountStatus,
        expected_account_id: u128,
    ) -> AccountOnboardingNextStep {
        match status.binding_status(self.owner, expected_account_id) {
            AccountBindingStatus::Registered => {
                AccountOnboardingNextStep::UseAccount(expected_account_id)
            }
            AccountBindingStatus::OwnerUnregistered => {
                AccountOnboardingNextStep::Register(self.register_tx())
            }
            blocked => AccountOnboardingNextStep::Blocked(blocked),
        }
    }

    /// Return a compact serializable onboarding decision for logs and UIs.
    #[must_use]
    pub fn summary(
        &self,
        status: &AccountStatus,
        expected_account_id: u128,
    ) -> AccountOnboardingSummary {
        let binding_status = status.binding_status(self.owner, expected_account_id);
        let action = match binding_status {
            AccountBindingStatus::Registered => AccountOnboardingAction::UseAccount,
            AccountBindingStatus::OwnerUnregistered => AccountOnboardingAction::Register,
            _ => AccountOnboardingAction::Blocked,
        };

        AccountOnboardingSummary {
            account_manager: self.account_manager,
            owner: self.owner,
            expected_account_id,
            binding_status,
            action,
            should_register: action == AccountOnboardingAction::Register,
            can_use_account: action == AccountOnboardingAction::UseAccount,
            is_blocked: action == AccountOnboardingAction::Blocked,
            registered_account_id: match action {
                AccountOnboardingAction::UseAccount => Some(expected_account_id),
                _ => None,
            },
            total_accounts: status.total_accounts,
            status_read_summary: self.status_read_summary(expected_account_id),
            has_register_transaction: action == AccountOnboardingAction::Register,
            register_transaction: match action {
                AccountOnboardingAction::Register => Some(self.register_tx().summary()),
                _ => None,
            },
        }
    }
}

impl AccountStatusPlan {
    #[must_use]
    pub const fn new(account_manager: Address, owner: Address, account_id: u128) -> Self {
        Self {
            account_manager,
            owner,
            account_id,
        }
    }

    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest, owner: Address, account_id: u128) -> Self {
        Self::new(manifest.contracts.account_manager, owner, account_id)
    }

    #[must_use]
    pub fn owner_of_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.account_manager,
            data: AccountManagerCalls::owner_of_calldata(self.account_id),
        }
    }

    #[must_use]
    pub fn account_id_of_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.account_manager,
            data: AccountManagerCalls::account_id_of_calldata(self.owner),
        }
    }

    #[must_use]
    pub fn total_accounts_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.account_manager,
            data: AccountManagerCalls::total_accounts_calldata(),
        }
    }

    #[must_use]
    pub fn calls(&self) -> [UnsignedCall; 3] {
        [
            self.owner_of_call(),
            self.account_id_of_call(),
            self.total_accounts_call(),
        ]
    }

    /// True when decoded status matches this plan's owner/account pair.
    #[must_use]
    pub fn matches(&self, status: &AccountStatus) -> bool {
        status.matches(self.owner, self.account_id)
    }

    /// True when decoded status confirms this plan's registered binding.
    #[must_use]
    pub fn is_registered_binding(&self, status: &AccountStatus) -> bool {
        status.is_registered_binding(self.owner, self.account_id)
    }

    /// Classify decoded status against this plan's owner/account pair.
    #[must_use]
    pub fn binding_status(&self, status: &AccountStatus) -> AccountBindingStatus {
        status.binding_status(self.owner, self.account_id)
    }

    /// Decode returns from [`Self::calls`] in the same fixed order.
    pub fn decode_returns(&self, returns: [&[u8]; 3]) -> Result<AccountStatus, AbiDecodeError> {
        Ok(AccountStatus {
            owner_of_account: AccountManagerCalls::decode_owner_of_return(returns[0])?,
            account_id_of_owner: AccountManagerCalls::decode_account_id_of_return(returns[1])?,
            total_accounts: AccountManagerCalls::decode_total_accounts_return(returns[2])?,
        })
    }

    /// Decode a transport-returned batch from [`Self::calls`].
    pub fn decode_return_slices<T: AsRef<[u8]>>(
        &self,
        returns: &[T],
    ) -> Result<AccountStatus, AbiDecodeError> {
        let returns = crate::abi::expect_return_count(returns, 3)?;
        self.decode_returns([returns[0], returns[1], returns[2]])
    }

    /// Decode an ordered transport-returned batch from [`Self::calls`].
    pub fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<AccountStatus, AbiDecodeError> {
        self.decode_return_slices(returns.as_returns())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    #[test]
    fn builds_register_and_account_lookup_calls() {
        let plan = AccountOnboardingPlan::new(addr(0x20), addr(0x30));

        let register = plan.register_tx();
        let [register_from_batch] = plan.transactions();
        assert_eq!(register.to, addr(0x20));
        assert_eq!(register_from_batch, register);
        assert_eq!(
            register.data,
            AccountManagerCalls::register_account_selector()
        );

        let lookup = plan.account_id_of_call();
        assert_eq!(lookup.to, addr(0x20));
        assert_eq!(
            &lookup.data[..4],
            &AccountManagerCalls::account_id_of_selector()
        );
        assert_eq!(&lookup.data[16..36], addr(0x30).as_slice());
    }

    #[test]
    fn decodes_register_account_return() {
        let plan = AccountOnboardingPlan::new(addr(0x20), addr(0x30));
        let mut account_id = [0u8; 32];
        account_id[31] = 7;

        assert_eq!(
            plan.decode_register_return(&account_id)
                .expect("account id decodes"),
            7
        );

        let unregistered = AccountStatus {
            owner_of_account: Address::ZERO,
            account_id_of_owner: 0,
            total_accounts: 9,
        };
        assert_eq!(
            plan.next_step(&unregistered, 7),
            AccountOnboardingNextStep::Register(plan.register_tx())
        );
        let unregistered_summary = plan.summary(&unregistered, 7);
        assert_eq!(unregistered_summary.account_manager, addr(0x20));
        assert_eq!(unregistered_summary.owner, addr(0x30));
        assert_eq!(unregistered_summary.expected_account_id, 7);
        assert_eq!(
            unregistered_summary.binding_status,
            AccountBindingStatus::OwnerUnregistered
        );
        assert_eq!(
            unregistered_summary.action,
            AccountOnboardingAction::Register
        );
        assert!(unregistered_summary.should_register);
        assert!(!unregistered_summary.can_use_account);
        assert!(!unregistered_summary.is_blocked);
        assert_eq!(unregistered_summary.registered_account_id, None);
        assert_eq!(unregistered_summary.total_accounts, 9);
        assert_eq!(unregistered_summary.status_read_summary.len, 3);
        assert!(unregistered_summary.has_register_transaction);
        assert_eq!(
            unregistered_summary
                .register_transaction
                .as_ref()
                .expect("register tx summary")
                .selector,
            plan.register_tx().summary().selector
        );
        assert_eq!(
            plan.status_plan(7),
            AccountStatusPlan::new(addr(0x20), addr(0x30), 7)
        );
        assert_eq!(plan.status_read_summary(7).len, 3);
        let registered = AccountStatus {
            owner_of_account: addr(0x30),
            account_id_of_owner: 7,
            total_accounts: 9,
        };
        assert_eq!(
            plan.next_step(&registered, 7),
            AccountOnboardingNextStep::UseAccount(7)
        );
        let registered_summary = plan.summary(&registered, 7);
        assert_eq!(
            registered_summary.binding_status,
            AccountBindingStatus::Registered
        );
        assert_eq!(
            registered_summary.action,
            AccountOnboardingAction::UseAccount
        );
        assert!(!registered_summary.should_register);
        assert!(registered_summary.can_use_account);
        assert!(!registered_summary.is_blocked);
        assert_eq!(registered_summary.registered_account_id, Some(7));
        assert!(!registered_summary.has_register_transaction);
        assert_eq!(registered_summary.register_transaction, None);
        let mismatched = AccountStatus {
            owner_of_account: addr(0x30),
            account_id_of_owner: 8,
            total_accounts: 9,
        };
        assert_eq!(
            plan.next_step(&mismatched, 7),
            AccountOnboardingNextStep::Blocked(
                AccountBindingStatus::OwnerRegisteredToDifferentAccount {
                    actual_account_id: 8
                }
            )
        );
        let mismatched_summary = plan.summary(&mismatched, 7);
        assert_eq!(mismatched_summary.action, AccountOnboardingAction::Blocked);
        assert!(!mismatched_summary.should_register);
        assert!(!mismatched_summary.can_use_account);
        assert!(mismatched_summary.is_blocked);
        assert_eq!(
            mismatched_summary.binding_status,
            AccountBindingStatus::OwnerRegisteredToDifferentAccount {
                actual_account_id: 8
            }
        );
        assert!(!mismatched_summary.has_register_transaction);
        assert_eq!(mismatched_summary.register_transaction, None);
        let json =
            serde_json::to_string(&unregistered_summary).expect("onboarding summary serializes");
        let restored: AccountOnboardingSummary =
            serde_json::from_str(&json).expect("onboarding summary deserializes");
        assert_eq!(restored, unregistered_summary);
        let mut legacy_json =
            serde_json::to_value(&unregistered_summary).expect("onboarding summary value");
        let legacy_object = legacy_json
            .as_object_mut()
            .expect("onboarding summary object");
        legacy_object.remove("should_register");
        legacy_object.remove("can_use_account");
        legacy_object.remove("is_blocked");
        legacy_object.remove("has_register_transaction");
        let legacy: AccountOnboardingSummary =
            serde_json::from_value(legacy_json).expect("legacy onboarding summary");
        assert!(!legacy.should_register);
        assert!(!legacy.can_use_account);
        assert!(!legacy.is_blocked);
        assert!(!legacy.has_register_transaction);
    }

    #[test]
    fn builds_plan_from_deployment_manifest() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        let plan = AccountOnboardingPlan::from_manifest(&manifest, addr(0x30));

        assert_eq!(plan.account_manager, manifest.contracts.account_manager);
        assert_eq!(plan.owner, addr(0x30));
        assert_eq!(plan.register_tx().to, manifest.contracts.account_manager);
        assert_eq!(
            plan.account_id_of_call().to,
            manifest.contracts.account_manager
        );
    }

    #[test]
    fn builds_account_status_calls() {
        let plan = AccountStatusPlan::new(addr(0x20), addr(0x30), 7);
        let [owner_of, account_id_of, total_accounts] = plan.calls();

        assert_eq!(owner_of.to, addr(0x20));
        assert_eq!(
            &owner_of.data[..4],
            &AccountManagerCalls::owner_of_selector()
        );
        assert_eq!(hex::encode(&owner_of.data[4..36]), format!("{:064x}", 7));

        assert_eq!(account_id_of.to, addr(0x20));
        assert_eq!(
            &account_id_of.data[..4],
            &AccountManagerCalls::account_id_of_selector()
        );
        assert_eq!(&account_id_of.data[16..36], addr(0x30).as_slice());

        assert_eq!(total_accounts.to, addr(0x20));
        assert_eq!(
            total_accounts.data,
            AccountManagerCalls::total_accounts_selector()
        );
    }

    #[test]
    fn builds_status_plan_from_deployment_manifest() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        let plan = AccountStatusPlan::from_manifest(&manifest, addr(0x30), 1);

        assert_eq!(plan.account_manager, manifest.contracts.account_manager);
        assert_eq!(plan.owner, addr(0x30));
        assert_eq!(plan.account_id, 1);
        assert_eq!(plan.owner_of_call().to, manifest.contracts.account_manager);
    }

    #[test]
    fn decodes_account_status_returns() {
        fn word(value: u8) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[31] = value;
            out
        }

        let plan = AccountStatusPlan::new(addr(0x20), addr(0x30), 7);
        let mut owner = [0u8; 32];
        owner[12..].copy_from_slice(addr(0x30).as_slice());
        let account_id = word(7);
        let total = word(9);

        let decoded = plan
            .decode_returns([&owner, &account_id, &total])
            .expect("status decodes");
        assert_eq!(
            plan.decode_return_slices(&[owner.to_vec(), account_id.to_vec(), total.to_vec()])
                .expect("status decodes from slices"),
            decoded
        );
        let batch = CallReturnBatch::new(vec![
            crate::CallReturn::new(owner.to_vec()),
            crate::CallReturn::new(account_id.to_vec()),
            crate::CallReturn::new(total.to_vec()),
        ]);
        assert_eq!(
            plan.decode_return_batch(&batch)
                .expect("status decodes from batch"),
            decoded
        );

        assert_eq!(
            decoded,
            AccountStatus {
                owner_of_account: addr(0x30),
                account_id_of_owner: 7,
                total_accounts: 9,
            }
        );
        assert!(decoded.matches(addr(0x30), 7));
        assert!(decoded.is_registered_binding(addr(0x30), 7));
        assert!(plan.matches(&decoded));
        assert!(plan.is_registered_binding(&decoded));
        assert_eq!(
            decoded.binding_status(addr(0x30), 7),
            AccountBindingStatus::Registered
        );
        assert_eq!(
            plan.binding_status(&decoded),
            AccountBindingStatus::Registered
        );
        assert!(!decoded.matches(addr(0x31), 7));
        assert!(!decoded.matches(addr(0x30), 8));
        let zero_account = AccountStatus {
            owner_of_account: addr(0x30),
            account_id_of_owner: 0,
            total_accounts: 9,
        };
        assert!(!zero_account.is_registered_binding(addr(0x30), 0));
        assert_eq!(
            zero_account.binding_status(addr(0x30), 0),
            AccountBindingStatus::OwnerUnregistered
        );
        let out_of_range = AccountStatus {
            owner_of_account: addr(0x30),
            account_id_of_owner: 10,
            total_accounts: 9,
        };
        assert!(!out_of_range.is_registered_binding(addr(0x30), 10));
        assert_eq!(
            out_of_range.binding_status(addr(0x30), 10),
            AccountBindingStatus::AccountIdNotRegistered { total_accounts: 9 }
        );
        assert_eq!(
            AccountStatus {
                owner_of_account: addr(0x31),
                account_id_of_owner: 7,
                total_accounts: 9,
            }
            .binding_status(addr(0x30), 7),
            AccountBindingStatus::AccountOwnedByDifferentOwner {
                actual_owner: addr(0x31)
            }
        );
    }
}
