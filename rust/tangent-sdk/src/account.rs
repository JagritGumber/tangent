//! High-level account onboarding helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{AccountManagerCalls, DeploymentManifest, UnsignedTx};

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
    pub fn account_id_of_call(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.account_manager,
            data: AccountManagerCalls::account_id_of_calldata(self.owner),
        }
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
}
