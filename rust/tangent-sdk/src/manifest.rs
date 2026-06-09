//! Deployment manifest types for Tangent networks.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

/// Parsed deployment manifest matching `docs/deployments/*.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentManifest {
    pub project: String,
    pub version: String,
    pub chain_id: u64,
    pub network: String,
    pub deployed_at: String,
    pub deployer: Address,
    pub contracts: ContractAddresses,
    pub verified_on_arcscan: bool,
    pub constants: NetworkConstants,
}

impl DeploymentManifest {
    /// Parse a deployment manifest from JSON.
    pub fn from_json(input: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(input).map_err(ManifestError::Json)
    }

    /// Build the EIP-712 domain inputs when an OrderBook address is present.
    #[must_use]
    pub fn order_book_domain(&self) -> Option<crate::DomainSeparatorInput> {
        self.contracts
            .order_book
            .map(|address| crate::DomainSeparatorInput::new(self.chain_id, address))
    }

    /// True when the manifest includes an `OrderBook` deployment.
    #[must_use]
    pub const fn has_order_book(&self) -> bool {
        self.contracts.order_book.is_some()
    }

    /// True when the manifest includes a `SettlementEngine` deployment.
    #[must_use]
    pub const fn has_settlement_engine(&self) -> bool {
        self.contracts.settlement_engine.is_some()
    }

    /// True when the manifest includes a `LiquidationKeeper` deployment.
    #[must_use]
    pub const fn has_liquidation_keeper(&self) -> bool {
        self.contracts.liquidation_keeper.is_some()
    }

    /// Report which optional full-stack perp contracts are present.
    #[must_use]
    pub const fn perp_stack_availability(&self) -> PerpStackAvailability {
        PerpStackAvailability {
            order_book: self.has_order_book(),
            settlement_engine: self.has_settlement_engine(),
            liquidation_keeper: self.has_liquidation_keeper(),
        }
    }

    /// True when all full perp DEX contracts are present.
    #[must_use]
    pub const fn has_perp_stack(&self) -> bool {
        self.perp_stack_availability().is_complete()
    }
}

/// Presence flags for optional full-stack perp contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerpStackAvailability {
    pub order_book: bool,
    pub settlement_engine: bool,
    pub liquidation_keeper: bool,
}

impl PerpStackAvailability {
    /// True when all optional full-stack perp contracts are present.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.order_book && self.settlement_engine && self.liquidation_keeper
    }
}

/// Contract addresses known for a deployment.
///
/// Full-stack contracts are optional because the live v0.1 Arc Testnet
/// manifest only contains the primitive deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractAddresses {
    #[serde(rename = "AccountManager")]
    pub account_manager: Address,
    #[serde(rename = "USDCVault")]
    pub usdc_vault: Address,
    #[serde(rename = "MarketRegistry")]
    pub market_registry: Address,
    #[serde(rename = "OrderBook")]
    pub order_book: Option<Address>,
    #[serde(rename = "SettlementEngine")]
    pub settlement_engine: Option<Address>,
    #[serde(rename = "LiquidationKeeper")]
    pub liquidation_keeper: Option<Address>,
}

/// Network constants published alongside contract addresses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct NetworkConstants {
    pub usdc: Address,
}

/// Errors that can occur while loading a deployment manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid deployment manifest json: {0}")]
    Json(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_arc_testnet_manifest_without_order_book() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        assert_eq!(manifest.project, "Tangent");
        assert_eq!(manifest.chain_id, 11111);
        assert_eq!(manifest.contracts.order_book, None);
        assert_eq!(manifest.order_book_domain(), None);
        assert!(!manifest.has_order_book());
        assert!(!manifest.has_settlement_engine());
        assert!(!manifest.has_liquidation_keeper());
        assert!(!manifest.has_perp_stack());
        assert_eq!(
            manifest.perp_stack_availability(),
            PerpStackAvailability {
                order_book: false,
                settlement_engine: false,
                liquidation_keeper: false,
            }
        );
        assert!(!manifest.perp_stack_availability().is_complete());
    }

    #[test]
    fn reports_full_perp_stack_when_optional_contracts_are_present() {
        let manifest = DeploymentManifest {
            project: "Tangent".to_owned(),
            version: "0.1.0".to_owned(),
            chain_id: 11111,
            network: "arc-testnet".to_owned(),
            deployed_at: "2026-05-25T18:42:40.104Z".to_owned(),
            deployer: Address::repeat_byte(0x10),
            contracts: ContractAddresses {
                account_manager: Address::repeat_byte(0x11),
                usdc_vault: Address::repeat_byte(0x12),
                market_registry: Address::repeat_byte(0x13),
                order_book: Some(Address::repeat_byte(0x14)),
                settlement_engine: Some(Address::repeat_byte(0x15)),
                liquidation_keeper: Some(Address::repeat_byte(0x16)),
            },
            verified_on_arcscan: true,
            constants: NetworkConstants {
                usdc: Address::repeat_byte(0x17),
            },
        };

        assert!(manifest.has_order_book());
        assert!(manifest.has_settlement_engine());
        assert!(manifest.has_liquidation_keeper());
        assert!(manifest.has_perp_stack());
        assert_eq!(
            manifest.perp_stack_availability(),
            PerpStackAvailability {
                order_book: true,
                settlement_engine: true,
                liquidation_keeper: true,
            }
        );
        assert!(manifest.perp_stack_availability().is_complete());
        assert_eq!(
            manifest
                .order_book_domain()
                .expect("order book domain")
                .verifying_contract,
            Address::repeat_byte(0x14)
        );
    }
}
