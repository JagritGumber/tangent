//! EIP-712 `Order` type mirroring `src/types/OrderTypes.sol` on-chain.
//!
//! The struct field order, types, and EIP-712 type string must remain
//! byte-identical to the Solidity side. Any change here is a wire-breaking
//! change and the on-chain `ORDER_TYPEHASH` must rev in lockstep. The
//! Solidity-side test [`test/OrderTypes.t.sol::test_typeHash_isFrozen`]
//! catches Solidity-side drift; the Rust-side check is the
//! [`Order::EIP712_TYPE_STRING`] constant + the `ORDER_TYPEHASH`
//! comparison below.

use alloy_primitives::{keccak256, B256};
use serde::{Deserialize, Serialize};

use crate::DomainSeparatorInput;

/// Tangent price scale: 1e8 == $1.
pub const PRICE_SCALE: u128 = 100_000_000;

/// Tangent base-size scale: 1e18 == 1 base unit.
pub const BASE_SCALE: u128 = 1_000_000_000_000_000_000;

/// Market-side convenience enum for readable order construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// Buy-side order: long entry or short close.
    Buy,
    /// Sell-side order: short entry or long close.
    Sell,
}

impl Side {
    /// Convert to the Solidity `Order.isBuy` boolean.
    #[must_use]
    pub const fn is_buy(self) -> bool {
        matches!(self, Self::Buy)
    }

    /// Convert from the Solidity `Order.isBuy` boolean.
    #[must_use]
    pub const fn from_is_buy(is_buy: bool) -> Self {
        if is_buy {
            Self::Buy
        } else {
            Self::Sell
        }
    }
}

/// Submit-time market constraints mirrored from `OrderBook.submitOrder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderConstraints {
    /// Minimum price increment in PRICE_SCALE units.
    pub tick_size: u128,
    /// Minimum size increment in 1e18 base units.
    pub lot_size: u128,
}

impl OrderConstraints {
    /// Construct constraints from `MarketRegistry.market(...)`.
    pub const fn new(tick_size: u128, lot_size: u128) -> Self {
        Self {
            tick_size,
            lot_size,
        }
    }
}

/// A single perpetual-futures order, EIP-712-signed by an account's owner.
///
/// Mirrors the `Order` struct in [`src/types/OrderTypes.sol`]. Field shape,
/// order, and types are wire-frozen and must not drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    /// `AccountManager`-assigned identifier of the trader.
    pub account_id: u128,
    /// `MarketRegistry`-assigned identifier of the perp market.
    pub market_id: u128,
    /// `true` = long entry / short close, `false` = short entry / long close.
    pub is_buy: bool,
    /// Worst-acceptable price in `PRICE_SCALE` units (1e8 = $1).
    pub limit_price: u128,
    /// Base quantity in 1e18 units.
    pub size: u128,
    /// Monotonic per-account counter; settled orders consume their nonce.
    pub nonce: u128,
    /// `block.timestamp` cutoff. Orders past expiry are rejected at submit.
    pub expiry: u64,
    /// `true` = order may only reduce an existing position.
    pub reduce_only: bool,
}

impl Order {
    /// The canonical EIP-712 type string for `Order`. Must match
    /// `OrderTypes.sol::ORDER_TYPEHASH`'s input exactly.
    pub const EIP712_TYPE_STRING: &'static str = "Order(uint256 accountId,uint256 marketId,bool isBuy,uint256 limitPrice,uint256 size,uint256 nonce,uint256 expiry,bool reduceOnly)";

    /// The canonical EIP-712 type hash for Tangent orders.
    #[must_use]
    pub fn type_hash() -> B256 {
        keccak256(Self::EIP712_TYPE_STRING.as_bytes())
    }

    /// Construct a new order. All fields validated at signing time
    /// downstream, not here.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account_id: u128,
        market_id: u128,
        is_buy: bool,
        limit_price: u128,
        size: u128,
        nonce: u128,
        expiry: u64,
        reduce_only: bool,
    ) -> Self {
        Self {
            account_id,
            market_id,
            is_buy,
            limit_price,
            size,
            nonce,
            expiry,
            reduce_only,
        }
    }

    /// Start an ergonomic order builder.
    #[must_use]
    pub fn builder() -> OrderBuilder {
        OrderBuilder::default()
    }

    /// Return the readable side represented by the wire `is_buy` flag.
    #[must_use]
    pub const fn side(&self) -> Side {
        Side::from_is_buy(self.is_buy)
    }

    /// Validate local fields and known market constraints before signing.
    pub fn validate(
        &self,
        constraints: OrderConstraints,
        current_timestamp: u64,
    ) -> Result<(), OrderError> {
        if self.account_id == 0 {
            return Err(OrderError::Invalid("account_id must be non-zero".into()));
        }
        if self.market_id == 0 {
            return Err(OrderError::Invalid("market_id must be non-zero".into()));
        }
        if self.limit_price == 0 {
            return Err(OrderError::Invalid("limit_price must be non-zero".into()));
        }
        if self.size == 0 {
            return Err(OrderError::Invalid("size must be non-zero".into()));
        }
        if self.nonce == 0 {
            return Err(OrderError::Invalid("nonce must be non-zero".into()));
        }
        if self.expiry <= current_timestamp {
            return Err(OrderError::Invalid("expiry must be in the future".into()));
        }
        if constraints.tick_size == 0 {
            return Err(OrderError::Invalid("tick_size must be non-zero".into()));
        }
        if constraints.lot_size == 0 {
            return Err(OrderError::Invalid("lot_size must be non-zero".into()));
        }
        if self.limit_price % constraints.tick_size != 0 {
            return Err(OrderError::Invalid("limit_price violates tick_size".into()));
        }
        if self.size % constraints.lot_size != 0 {
            return Err(OrderError::Invalid("size violates lot_size".into()));
        }
        Ok(())
    }

    /// Compute the Solidity-compatible EIP-712 struct hash.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        let mut encoded = Vec::with_capacity(288);
        crate::eip712::encode_bytes32(&mut encoded, Self::type_hash());
        crate::eip712::encode_u128(&mut encoded, self.account_id);
        crate::eip712::encode_u128(&mut encoded, self.market_id);
        crate::eip712::encode_bool(&mut encoded, self.is_buy);
        crate::eip712::encode_u128(&mut encoded, self.limit_price);
        crate::eip712::encode_u128(&mut encoded, self.size);
        crate::eip712::encode_u128(&mut encoded, self.nonce);
        crate::eip712::encode_u64(&mut encoded, self.expiry);
        crate::eip712::encode_bool(&mut encoded, self.reduce_only);
        crate::eip712::hash_words(encoded)
    }

    /// Compute the on-chain `orderHash` used by `OrderBook`.
    ///
    /// Solidity computes this through `OrderTypes.hash(order)`, which is the
    /// EIP-712 struct hash.
    #[must_use]
    pub fn order_hash(&self) -> B256 {
        self.struct_hash()
    }

    /// Compute the final EIP-712 digest an account owner signs.
    #[must_use]
    pub fn digest(&self, domain: &DomainSeparatorInput) -> B256 {
        crate::eip712::digest(domain.separator(), self.struct_hash())
    }

    /// Package this order with its domain and digest for an external signer.
    #[must_use]
    pub fn prepare(self, domain: DomainSeparatorInput) -> crate::PreparedOrder {
        crate::PreparedOrder::new(self, domain)
    }
}

/// User-facing order construction parameters.
///
/// This keeps readable side semantics at the SDK boundary while preserving
/// the wire-frozen [`Order`] shape internally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderParams {
    pub account_id: u128,
    pub market_id: u128,
    pub side: Side,
    pub limit_price: u128,
    pub size: u128,
    pub nonce: u128,
    pub expiry: u64,
    pub reduce_only: bool,
}

impl OrderParams {
    /// Build and validate a wire-compatible [`Order`].
    pub fn build(
        self,
        constraints: OrderConstraints,
        current_timestamp: u64,
    ) -> Result<Order, OrderError> {
        Order::builder()
            .account_id(self.account_id)
            .market_id(self.market_id)
            .side(self.side)
            .limit_price(self.limit_price)
            .size(self.size)
            .nonce(self.nonce)
            .expiry(self.expiry)
            .reduce_only(self.reduce_only)
            .build(constraints, current_timestamp)
    }
}

/// Builder for constructing and validating an [`Order`] before signing.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OrderBuilder {
    account_id: Option<u128>,
    market_id: Option<u128>,
    side: Option<Side>,
    limit_price: Option<u128>,
    size: Option<u128>,
    nonce: Option<u128>,
    expiry: Option<u64>,
    reduce_only: bool,
}

impl OrderBuilder {
    /// Set the `AccountManager` account id.
    #[must_use]
    pub fn account_id(mut self, account_id: u128) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Set the `MarketRegistry` market id.
    #[must_use]
    pub fn market_id(mut self, market_id: u128) -> Self {
        self.market_id = Some(market_id);
        self
    }

    /// Set buy or sell side.
    #[must_use]
    pub fn side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Set limit price in PRICE_SCALE units.
    #[must_use]
    pub fn limit_price(mut self, limit_price: u128) -> Self {
        self.limit_price = Some(limit_price);
        self
    }

    /// Set base quantity in 1e18 units.
    #[must_use]
    pub fn size(mut self, size: u128) -> Self {
        self.size = Some(size);
        self
    }

    /// Set monotonic per-account nonce.
    #[must_use]
    pub fn nonce(mut self, nonce: u128) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Set unix timestamp expiry.
    #[must_use]
    pub fn expiry(mut self, expiry: u64) -> Self {
        self.expiry = Some(expiry);
        self
    }

    /// Mark as reduce-only.
    #[must_use]
    pub fn reduce_only(mut self, reduce_only: bool) -> Self {
        self.reduce_only = reduce_only;
        self
    }

    /// Build and validate against market constraints and a local timestamp.
    pub fn build(
        self,
        constraints: OrderConstraints,
        current_timestamp: u64,
    ) -> Result<Order, OrderError> {
        let order = Order {
            account_id: self
                .account_id
                .ok_or_else(|| OrderError::Invalid("account_id is required".into()))?,
            market_id: self
                .market_id
                .ok_or_else(|| OrderError::Invalid("market_id is required".into()))?,
            is_buy: self
                .side
                .ok_or_else(|| OrderError::Invalid("side is required".into()))?
                .is_buy(),
            limit_price: self
                .limit_price
                .ok_or_else(|| OrderError::Invalid("limit_price is required".into()))?,
            size: self
                .size
                .ok_or_else(|| OrderError::Invalid("size is required".into()))?,
            nonce: self
                .nonce
                .ok_or_else(|| OrderError::Invalid("nonce is required".into()))?,
            expiry: self
                .expiry
                .ok_or_else(|| OrderError::Invalid("expiry is required".into()))?,
            reduce_only: self.reduce_only,
        };
        order.validate(constraints, current_timestamp)?;
        Ok(order)
    }
}

/// Errors that can occur constructing or signing an order.
///
/// v0.1 surface is small; v0.8 adds variants for signer-backend errors
/// (Circle Dev Wallet API failures, AWS KMS errors, etc.) when the
/// signing backends land.
#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    /// Order is missing a required field, has an invalid combination, or
    /// has expired. Specific reason in the inner message.
    #[error("invalid order: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eip712_type_string_matches_solidity() {
        // This string must remain byte-identical to the Solidity-side
        // ORDER_TYPEHASH input. A drift here is a wire-breaking change.
        assert_eq!(
            Order::EIP712_TYPE_STRING,
            "Order(uint256 accountId,uint256 marketId,bool isBuy,uint256 limitPrice,uint256 size,uint256 nonce,uint256 expiry,bool reduceOnly)"
        );
        assert_eq!(
            hex::encode(Order::type_hash()),
            "da43521c783b1bbaf61db64338940703bb2aae681813e15d1c44e31074e9060f"
        );
    }

    #[test]
    fn order_is_constructable_and_serde_roundtrips() {
        let order = Order::new(
            7,
            1,
            true,
            6_500_000_000_000,
            1_000_000_000_000_000_000,
            42,
            1_717_000_000,
            false,
        );
        let json = serde_json::to_string(&order).expect("serialize");
        let back: Order = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(order, back);
    }

    #[test]
    fn builder_constructs_valid_order() {
        let constraints = OrderConstraints::new(100, 1_000_000_000_000_000);
        let order = Order::builder()
            .account_id(7)
            .market_id(1)
            .side(Side::Buy)
            .limit_price(65_000 * PRICE_SCALE)
            .size(BASE_SCALE)
            .nonce(42)
            .expiry(1_717_000_000)
            .build(constraints, 1_716_999_000)
            .expect("valid order");

        assert!(order.is_buy);
        assert_eq!(order.side(), Side::Buy);
        assert!(!order.reduce_only);
    }

    #[test]
    fn side_roundtrips_wire_boolean() {
        assert!(Side::Buy.is_buy());
        assert!(!Side::Sell.is_buy());
        assert_eq!(Side::from_is_buy(true), Side::Buy);
        assert_eq!(Side::from_is_buy(false), Side::Sell);
        assert_eq!(
            Order::new(
                7,
                1,
                false,
                65_000 * PRICE_SCALE,
                BASE_SCALE,
                42,
                100,
                false
            )
            .side(),
            Side::Sell
        );
    }

    #[test]
    fn order_params_builds_same_order_as_builder() {
        let constraints = OrderConstraints::new(100, 1_000_000_000_000_000);
        let params = OrderParams {
            account_id: 7,
            market_id: 1,
            side: Side::Buy,
            limit_price: 65_000 * PRICE_SCALE,
            size: BASE_SCALE,
            nonce: 42,
            expiry: 1_717_000_000,
            reduce_only: true,
        };
        let from_params = params
            .build(constraints, 1_716_999_000)
            .expect("valid order");
        let from_builder = Order::builder()
            .account_id(7)
            .market_id(1)
            .side(Side::Buy)
            .limit_price(65_000 * PRICE_SCALE)
            .size(BASE_SCALE)
            .nonce(42)
            .expiry(1_717_000_000)
            .reduce_only(true)
            .build(constraints, 1_716_999_000)
            .expect("valid order");

        assert_eq!(from_params, from_builder);
    }

    #[test]
    fn order_hash_and_digest_match_frozen_fixture() {
        let domain = DomainSeparatorInput::new(11111, alloy_primitives::Address::ZERO);
        let order = Order::new(
            7,
            1,
            true,
            65_000 * PRICE_SCALE,
            BASE_SCALE,
            1,
            1_717_000_000,
            false,
        );

        assert_eq!(
            hex::encode(domain.separator()),
            "7a56aaa9c62a007bd4ad2bb83215db0d7bbebadab42d61484a18d062e9f99a72"
        );
        assert_eq!(
            hex::encode(order.struct_hash()),
            "b0b9bd99f3734201d225297621c4a3a15cbdb0c6381dc7789dc0b85d94a08cc0"
        );
        assert_eq!(order.order_hash(), order.struct_hash());
        assert_eq!(
            hex::encode(order.digest(&domain)),
            "28e8b0b1104d7872301ab044c7b2106a4df3759a110949d6658cf7a704a79447"
        );
    }

    #[test]
    fn validation_rejects_tick_lot_and_expiry_errors() {
        let constraints = OrderConstraints::new(100, 1_000_000_000_000_000);

        let bad_tick = Order::new(
            7,
            1,
            true,
            65_000 * PRICE_SCALE + 1,
            BASE_SCALE,
            42,
            100,
            false,
        );
        assert!(bad_tick.validate(constraints, 99).is_err());

        let bad_lot = Order::new(
            7,
            1,
            true,
            65_000 * PRICE_SCALE,
            BASE_SCALE + 1,
            42,
            100,
            false,
        );
        assert!(bad_lot.validate(constraints, 99).is_err());

        let expired = Order::new(7, 1, true, 65_000 * PRICE_SCALE, BASE_SCALE, 42, 100, false);
        assert!(expired.validate(constraints, 100).is_err());
    }
}
