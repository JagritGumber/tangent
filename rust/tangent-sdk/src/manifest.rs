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

    /// Return full-stack addresses or a typed missing-stack error.
    pub fn require_perp_stack(&self) -> Result<FullPerpStackAddresses, ManifestError> {
        let availability = self.perp_stack_availability();
        Ok(FullPerpStackAddresses {
            account_manager: self.contracts.account_manager,
            usdc_vault: self.contracts.usdc_vault,
            market_registry: self.contracts.market_registry,
            order_book: self
                .contracts
                .order_book
                .ok_or(ManifestError::MissingPerpStack { availability })?,
            settlement_engine: self
                .contracts
                .settlement_engine
                .ok_or(ManifestError::MissingPerpStack { availability })?,
            liquidation_keeper: self
                .contracts
                .liquidation_keeper
                .ok_or(ManifestError::MissingPerpStack { availability })?,
        })
    }
}

/// Presence flags for optional full-stack perp contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerpStackAvailability {
    pub order_book: bool,
    pub settlement_engine: bool,
    pub liquidation_keeper: bool,
}

/// Compact summary for optional full-stack perp contract availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerpStackAvailabilitySummary {
    pub order_book: bool,
    pub settlement_engine: bool,
    pub liquidation_keeper: bool,
    pub present_contracts: usize,
    pub missing_contracts: usize,
    #[serde(default)]
    pub has_order_book: bool,
    #[serde(default)]
    pub has_settlement_engine: bool,
    #[serde(default)]
    pub has_liquidation_keeper: bool,
    #[serde(default)]
    pub has_any_perp_contracts: bool,
    #[serde(default)]
    pub has_missing_contracts: bool,
    #[serde(default)]
    pub is_complete: bool,
    pub missing_contract_names: Vec<String>,
}

impl PerpStackAvailability {
    /// True when all optional full-stack perp contracts are present.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.order_book && self.settlement_engine && self.liquidation_keeper
    }

    /// Return the optional full-stack contract names missing from this manifest.
    #[must_use]
    pub fn missing_contracts(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.order_book {
            missing.push("OrderBook");
        }
        if !self.settlement_engine {
            missing.push("SettlementEngine");
        }
        if !self.liquidation_keeper {
            missing.push("LiquidationKeeper");
        }
        missing
    }

    /// Return a serializable summary for startup gates and operator UIs.
    #[must_use]
    pub fn summary(&self) -> PerpStackAvailabilitySummary {
        let present_contracts = usize::from(self.order_book)
            + usize::from(self.settlement_engine)
            + usize::from(self.liquidation_keeper);
        let missing_contract_names = self
            .missing_contracts()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        PerpStackAvailabilitySummary {
            order_book: self.order_book,
            settlement_engine: self.settlement_engine,
            liquidation_keeper: self.liquidation_keeper,
            present_contracts,
            missing_contracts: missing_contract_names.len(),
            has_order_book: self.order_book,
            has_settlement_engine: self.settlement_engine,
            has_liquidation_keeper: self.liquidation_keeper,
            has_any_perp_contracts: present_contracts > 0,
            has_missing_contracts: !missing_contract_names.is_empty(),
            is_complete: self.is_complete(),
            missing_contract_names,
        }
    }
}

/// Non-optional full-stack contract addresses for keeper/client consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullPerpStackAddresses {
    pub account_manager: Address,
    pub usdc_vault: Address,
    pub market_registry: Address,
    pub order_book: Address,
    pub settlement_engine: Address,
    pub liquidation_keeper: Address,
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
    #[error("deployment manifest is missing full perp stack contracts: {availability:?}")]
    MissingPerpStack { availability: PerpStackAvailability },
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
        assert_eq!(
            manifest.perp_stack_availability().missing_contracts(),
            vec!["OrderBook", "SettlementEngine", "LiquidationKeeper"]
        );
        let availability_summary = manifest.perp_stack_availability().summary();
        assert_eq!(availability_summary.present_contracts, 0);
        assert_eq!(availability_summary.missing_contracts, 3);
        assert!(!availability_summary.has_any_perp_contracts);
        assert!(availability_summary.has_missing_contracts);
        assert!(!availability_summary.is_complete);
        assert_eq!(
            availability_summary.missing_contract_names,
            vec![
                "OrderBook".to_owned(),
                "SettlementEngine".to_owned(),
                "LiquidationKeeper".to_owned()
            ]
        );
        let summary_json =
            serde_json::to_string(&availability_summary).expect("availability summary serializes");
        assert!(summary_json.contains("\"missing_contracts\":3"));
        assert!(summary_json.contains("\"has_missing_contracts\":true"));
        let restored_summary: PerpStackAvailabilitySummary =
            serde_json::from_str(&summary_json).expect("availability summary deserializes");
        assert_eq!(restored_summary, availability_summary);
        let mut legacy_summary_json =
            serde_json::to_value(&availability_summary).expect("summary value");
        let legacy_summary_object = legacy_summary_json.as_object_mut().expect("summary object");
        legacy_summary_object.remove("has_order_book");
        legacy_summary_object.remove("has_settlement_engine");
        legacy_summary_object.remove("has_liquidation_keeper");
        legacy_summary_object.remove("has_any_perp_contracts");
        legacy_summary_object.remove("has_missing_contracts");
        legacy_summary_object.remove("is_complete");
        let legacy_summary: PerpStackAvailabilitySummary =
            serde_json::from_value(legacy_summary_json).expect("legacy summary deserializes");
        assert!(!legacy_summary.has_order_book);
        assert!(!legacy_summary.has_settlement_engine);
        assert!(!legacy_summary.has_liquidation_keeper);
        assert!(!legacy_summary.has_any_perp_contracts);
        assert!(!legacy_summary.has_missing_contracts);
        assert!(!legacy_summary.is_complete);
        assert!(matches!(
            manifest.require_perp_stack(),
            Err(ManifestError::MissingPerpStack {
                availability: PerpStackAvailability {
                    order_book: false,
                    settlement_engine: false,
                    liquidation_keeper: false,
                }
            })
        ));
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
        assert!(manifest
            .perp_stack_availability()
            .missing_contracts()
            .is_empty());
        let availability_summary = manifest.perp_stack_availability().summary();
        assert_eq!(availability_summary.present_contracts, 3);
        assert_eq!(availability_summary.missing_contracts, 0);
        assert!(availability_summary.has_order_book);
        assert!(availability_summary.has_settlement_engine);
        assert!(availability_summary.has_liquidation_keeper);
        assert!(availability_summary.has_any_perp_contracts);
        assert!(!availability_summary.has_missing_contracts);
        assert!(availability_summary.is_complete);
        assert!(availability_summary.missing_contract_names.is_empty());
        assert_eq!(
            manifest.require_perp_stack().expect("full stack"),
            FullPerpStackAddresses {
                account_manager: Address::repeat_byte(0x11),
                usdc_vault: Address::repeat_byte(0x12),
                market_registry: Address::repeat_byte(0x13),
                order_book: Address::repeat_byte(0x14),
                settlement_engine: Address::repeat_byte(0x15),
                liquidation_keeper: Address::repeat_byte(0x16),
            }
        );
        assert_eq!(
            manifest
                .order_book_domain()
                .expect("order book domain")
                .verifying_contract,
            Address::repeat_byte(0x14)
        );
    }
}
