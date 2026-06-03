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
    }
}
