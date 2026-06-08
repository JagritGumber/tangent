//! High-level account onboarding helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{AbiDecodeError, AccountManagerCalls, DeploymentManifest, UnsignedCall, UnsignedTx};

/// Permissionless Tangent account onboarding workflow.
///
/// Broadcast `register_tx` from the owner address, then either decode the
/// `registerAccount()` return value or use `account_id_of_call()` as an
/// `eth_call` to recover the registered account id later.
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
    pub fn account_id_of_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.account_manager,
            data: AccountManagerCalls::account_id_of_calldata(self.owner),
        }
    }

    pub fn decode_register_return(&self, register_return: &[u8]) -> Result<u128, AbiDecodeError> {
        AccountManagerCalls::decode_register_account_return(register_return)
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

    /// Decode returns from [`Self::calls`] in the same fixed order.
    pub fn decode_returns(&self, returns: [&[u8]; 3]) -> Result<AccountStatus, AbiDecodeError> {
        Ok(AccountStatus {
            owner_of_account: AccountManagerCalls::decode_owner_of_return(returns[0])?,
            account_id_of_owner: AccountManagerCalls::decode_account_id_of_return(returns[1])?,
            total_accounts: AccountManagerCalls::decode_total_accounts_return(returns[2])?,
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
    fn builds_register_and_account_lookup_calls() {
        let plan = AccountOnboardingPlan::new(addr(0x20), addr(0x30));

        let register = plan.register_tx();
        assert_eq!(register.to, addr(0x20));
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
            decoded,
            AccountStatus {
                owner_of_account: addr(0x30),
                account_id_of_owner: 7,
                total_accounts: 9,
            }
        );
    }
}
