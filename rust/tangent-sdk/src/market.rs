//! High-level market read helpers built from raw ABI call builders.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    AbiDecodeError, DeploymentManifest, MarketRegistryCalls, OrderConstraints, UnsignedCall,
};

/// Read-side Tangent market discovery calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketReadPlan {
    pub market_registry: Address,
    pub market_id: u128,
}

/// Decoded `MarketRegistry.market(marketId)` metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketDetails {
    pub symbol: String,
    pub price_feed: Address,
    pub initial_margin_bps: u16,
    pub maint_margin_bps: u16,
    pub max_leverage: u8,
    pub tick_size: u128,
    pub lot_size: u128,
    pub max_price_age: u32,
    pub paused: bool,
}

impl MarketDetails {
    /// Build order validation constraints from decoded market metadata.
    #[must_use]
    pub const fn order_constraints(&self) -> OrderConstraints {
        OrderConstraints::new(self.tick_size, self.lot_size)
    }
}

/// Decoded market reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketReadSummary {
    pub total_markets: u128,
    pub mark_price: u128,
    pub market: Option<MarketDetails>,
}

impl MarketReadSummary {
    /// Build order validation constraints when market metadata was decoded.
    #[must_use]
    pub fn order_constraints(&self) -> Option<OrderConstraints> {
        self.market.as_ref().map(MarketDetails::order_constraints)
    }
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

    pub fn decode_summary_returns(
        &self,
        total_markets_return: &[u8],
        mark_price_return: &[u8],
    ) -> Result<MarketReadSummary, AbiDecodeError> {
        Ok(MarketReadSummary {
            total_markets: MarketRegistryCalls::decode_total_markets_return(total_markets_return)?,
            mark_price: MarketRegistryCalls::decode_mark_price_return(mark_price_return)?,
            market: None,
        })
    }

    pub fn decode_market_return(
        &self,
        market_return: &[u8],
    ) -> Result<MarketDetails, AbiDecodeError> {
        let (
            symbol,
            price_feed,
            initial_margin_bps,
            maint_margin_bps,
            max_leverage,
            tick_size,
            lot_size,
            max_price_age,
            paused,
        ) = MarketRegistryCalls::decode_market_return(market_return)?;

        Ok(MarketDetails {
            symbol,
            price_feed,
            initial_margin_bps,
            maint_margin_bps,
            max_leverage,
            tick_size,
            lot_size,
            max_price_age,
            paused,
        })
    }

    /// Decode returns from [`Self::calls`] in the same fixed order.
    pub fn decode_returns(&self, returns: [&[u8]; 3]) -> Result<MarketReadSummary, AbiDecodeError> {
        Ok(MarketReadSummary {
            total_markets: MarketRegistryCalls::decode_total_markets_return(returns[0])?,
            market: Some(self.decode_market_return(returns[1])?),
            mark_price: MarketRegistryCalls::decode_mark_price_return(returns[2])?,
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

    #[test]
    fn decodes_market_read_summary_returns() {
        fn word(value: u8) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[31] = value;
            out
        }

        let plan = MarketReadPlan::new(addr(0x20), 7);
        let total = word(2);
        let mark = word(9);

        let decoded = plan
            .decode_summary_returns(&total, &mark)
            .expect("summary decodes");

        assert_eq!(
            decoded,
            MarketReadSummary {
                total_markets: 2,
                mark_price: 9,
                market: None,
            }
        );
        assert_eq!(decoded.order_constraints(), None);
    }

    #[test]
    fn decodes_market_read_returns_in_call_order() {
        fn word(value: u8) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[31] = value;
            out
        }

        let plan = MarketReadPlan::new(addr(0x20), 7);
        let total = word(2);
        let market = encoded_market(false);
        let mark = word(9);

        let decoded = plan
            .decode_returns([&total, &market, &mark])
            .expect("summary decodes");

        assert_eq!(
            decoded,
            MarketReadSummary {
                total_markets: 2,
                mark_price: 9,
                market: Some(MarketDetails {
                    symbol: "BTC".to_owned(),
                    price_feed: addr(0x11),
                    initial_margin_bps: 1_000,
                    maint_margin_bps: 500,
                    max_leverage: 10,
                    tick_size: 100,
                    lot_size: 1_000_000_000_000_000,
                    max_price_age: 60,
                    paused: false,
                }),
            }
        );
        assert_eq!(
            decoded.order_constraints(),
            Some(OrderConstraints::new(100, 1_000_000_000_000_000))
        );
    }

    #[test]
    fn decodes_market_metadata_return() {
        let plan = MarketReadPlan::new(addr(0x20), 7);
        let details = plan
            .decode_market_return(&encoded_market(true))
            .expect("market decodes");

        assert_eq!(
            details,
            MarketDetails {
                symbol: "BTC".to_owned(),
                price_feed: addr(0x11),
                initial_margin_bps: 1_000,
                maint_margin_bps: 500,
                max_leverage: 10,
                tick_size: 100,
                lot_size: 1_000_000_000_000_000,
                max_price_age: 60,
                paused: true,
            }
        );
        assert_eq!(
            details.order_constraints(),
            OrderConstraints::new(100, 1_000_000_000_000_000)
        );
    }

    #[test]
    fn rejects_truncated_market_symbol_return() {
        let plan = MarketReadPlan::new(addr(0x20), 7);
        let mut data = encoded_market(false);
        data.truncate(data.len() - 1);

        assert_eq!(
            plan.decode_market_return(&data)
                .expect_err("truncated symbol"),
            AbiDecodeError::InvalidLength {
                expected: 352,
                actual: 351,
            }
        );
    }

    #[test]
    fn rejects_trailing_market_symbol_return_bytes() {
        let plan = MarketReadPlan::new(addr(0x20), 7);
        let mut data = encoded_market(false);
        data.push(0);

        assert_eq!(
            plan.decode_market_return(&data)
                .expect_err("trailing bytes after padded symbol"),
            AbiDecodeError::InvalidLength {
                expected: 352,
                actual: 353,
            }
        );
    }

    #[test]
    fn rejects_invalid_market_symbol_utf8() {
        let plan = MarketReadPlan::new(addr(0x20), 7);
        let mut data = encoded_market(false);
        data[320] = 0xff;

        assert_eq!(
            plan.decode_market_return(&data).expect_err("bad utf8"),
            AbiDecodeError::InvalidStringUtf8,
        );
    }

    #[test]
    fn rejects_market_symbol_offsets_inside_tuple_head() {
        let plan = MarketReadPlan::new(addr(0x20), 7);
        let mut data = encoded_market(false);
        data[0..32].fill(0);
        data[31] = 32;

        assert_eq!(
            plan.decode_market_return(&data)
                .expect_err("symbol offset inside fixed tuple head"),
            AbiDecodeError::InvalidOffset(32),
        );
    }

    #[test]
    fn rejects_unaligned_market_symbol_offsets() {
        let plan = MarketReadPlan::new(addr(0x20), 7);
        let mut data = encoded_market(false);
        data[30] = 1;
        data[31] = 33;

        assert_eq!(
            plan.decode_market_return(&data)
                .expect_err("unaligned symbol offset"),
            AbiDecodeError::InvalidOffset(289),
        );
    }

    #[test]
    fn rejects_market_bps_and_age_overflows() {
        let plan = MarketReadPlan::new(addr(0x20), 7);

        for (name, word_index, value) in [
            ("initial margin overflow", 2usize, u16::MAX as u128 + 1),
            ("maintenance margin overflow", 3usize, u16::MAX as u128 + 1),
            ("max leverage overflow", 4usize, u8::MAX as u128 + 1),
            ("max price age overflow", 7usize, u32::MAX as u128 + 1),
        ] {
            let mut data = encoded_market(false);
            data[word_index * 32 + 16..word_index * 32 + 32].copy_from_slice(&value.to_be_bytes());

            assert_eq!(
                plan.decode_market_return(&data).expect_err(name),
                AbiDecodeError::UintOverflow,
            );
        }
    }

    fn encoded_market(paused: bool) -> Vec<u8> {
        fn word_u128(value: u128) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[16..].copy_from_slice(&value.to_be_bytes());
            out
        }

        let mut data = Vec::new();
        data.extend_from_slice(&word_u128(288));
        let mut price_feed = [0u8; 32];
        price_feed[12..].copy_from_slice(addr(0x11).as_slice());
        data.extend_from_slice(&price_feed);
        data.extend_from_slice(&word_u128(1_000));
        data.extend_from_slice(&word_u128(500));
        data.extend_from_slice(&word_u128(10));
        data.extend_from_slice(&word_u128(100));
        data.extend_from_slice(&word_u128(1_000_000_000_000_000));
        data.extend_from_slice(&word_u128(60));
        data.extend_from_slice(&word_u128(if paused { 1 } else { 0 }));
        data.extend_from_slice(&word_u128(3));
        let mut symbol = [0u8; 32];
        symbol[..3].copy_from_slice(b"BTC");
        data.extend_from_slice(&symbol);
        data
    }
}
