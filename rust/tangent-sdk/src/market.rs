//! High-level market read helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{DeploymentManifest, MarketRegistryCalls, UnsignedCall};

/// Read-side Tangent market discovery calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketReadPlan {
    pub market_registry: Address,
    pub market_id: u128,
}

impl MarketReadPlan {
    #[must_use]
    pub const fn new(market_registry: Address, market_id: u128) -> Self {
        Self {
            market_registry,
            market_id,
        }
    }

    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest, market_id: u128) -> Self {
        Self::new(manifest.contracts.market_registry, market_id)
    }

    #[must_use]
    pub fn total_markets_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.market_registry,
            data: MarketRegistryCalls::total_markets_calldata(),
        }
    }

    #[must_use]
    pub fn market_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.market_registry,
            data: MarketRegistryCalls::market_calldata(self.market_id),
        }
    }

    #[must_use]
    pub fn mark_price_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.market_registry,
            data: MarketRegistryCalls::mark_price_calldata(self.market_id),
        }
    }

    #[must_use]
    pub fn calls(&self) -> [UnsignedCall; 3] {
        [
            self.total_markets_call(),
            self.market_call(),
            self.mark_price_call(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    #[test]
    fn builds_market_read_calls() {
        let plan = MarketReadPlan::new(addr(0x20), 7);
        let [total_markets, market, mark_price] = plan.calls();

        assert_eq!(total_markets.to, addr(0x20));
        assert_eq!(
            total_markets.data,
            MarketRegistryCalls::total_markets_selector()
        );

        assert_eq!(market.to, addr(0x20));
        assert_eq!(&market.data[..4], &MarketRegistryCalls::market_selector());
        assert_eq!(hex::encode(&market.data[4..36]), format!("{:064x}", 7));

        assert_eq!(mark_price.to, addr(0x20));
        assert_eq!(
            &mark_price.data[..4],
            &MarketRegistryCalls::mark_price_selector()
        );
        assert_eq!(hex::encode(&mark_price.data[4..36]), format!("{:064x}", 7));
    }

    #[test]
    fn builds_market_read_plan_from_deployment_manifest() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        let plan = MarketReadPlan::from_manifest(&manifest, 1);

        assert_eq!(plan.market_registry, manifest.contracts.market_registry);
        assert_eq!(plan.market_id, 1);
        assert_eq!(
            plan.total_markets_call().to,
            manifest.contracts.market_registry
        );
        assert_eq!(
            plan.mark_price_call().to,
            manifest.contracts.market_registry
        );
    }
}
