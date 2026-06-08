//! High-level order lifecycle helpers built from signed orders and raw ABI calls.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    AbiDecodeError, DeploymentManifest, Order, OrderBookCalls, SignedOrder, UnsignedCall,
    UnsignedTx,
};

/// Permissionless OrderBook maintenance calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBookMaintenancePlan {
    pub order_book: Address,
}

impl OrderBookMaintenancePlan {
    #[must_use]
    pub const fn new(order_book: Address) -> Self {
        Self { order_book }
    }

    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest) -> Option<Self> {
        manifest.contracts.order_book.map(Self::new)
    }

    #[must_use]
    pub fn tick_tx(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.order_book,
            data: OrderBookCalls::tick_calldata(),
        }
    }

    #[must_use]
    pub fn transactions(&self) -> [UnsignedTx; 1] {
        [self.tick_tx()]
    }
}

/// Submit/read/cancel calls for one signed order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderLifecyclePlan {
    pub order_book: Address,
    pub signed_order: SignedOrder,
}

/// Decoded order lifecycle reads.
///
/// `is_live` is the current live predicate. `stored_order` is present when
/// `orderOf(orderHash)` reports that the order was known to the book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderLifecycleStatus {
    pub is_live: bool,
    pub stored_order: Option<Order>,
}

impl OrderLifecyclePlan {
    #[must_use]
    pub const fn new(order_book: Address, signed_order: SignedOrder) -> Self {
        Self {
            order_book,
            signed_order,
        }
    }

    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest, signed_order: SignedOrder) -> Option<Self> {
        manifest
            .contracts
            .order_book
            .map(|order_book| Self::new(order_book, signed_order))
    }

    #[must_use]
    pub fn order_hash(&self) -> alloy_primitives::B256 {
        self.signed_order.order.order_hash()
    }

    #[must_use]
    pub fn submit_tx(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.order_book,
            data: self.signed_order.submit_order_calldata(),
        }
    }

    #[must_use]
    pub fn cancel_tx(&self) -> UnsignedTx {
        UnsignedTx {
            to: self.order_book,
            data: OrderBookCalls::cancel_order_calldata(self.order_hash()),
        }
    }

    #[must_use]
    pub fn is_live_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.order_book,
            data: OrderBookCalls::is_live_calldata(self.order_hash()),
        }
    }

    #[must_use]
    pub fn order_of_call(&self) -> UnsignedCall {
        UnsignedCall {
            to: self.order_book,
            data: OrderBookCalls::order_of_calldata(self.order_hash()),
        }
    }

    #[must_use]
    pub fn calls(&self) -> [UnsignedCall; 2] {
        [self.is_live_call(), self.order_of_call()]
    }

    pub fn decode_is_live_return(
        &self,
        is_live_return: &[u8],
    ) -> Result<OrderLifecycleStatus, AbiDecodeError> {
        Ok(OrderLifecycleStatus {
            is_live: OrderBookCalls::decode_is_live_return(is_live_return)?,
            stored_order: None,
        })
    }

    /// Decode the `orderOf(orderHash)` return for this plan's order.
    ///
    /// Returns `Some(order)` when the book reports `exists = true`; returns
    /// `None` for the all-zero missing-order shape.
    pub fn decode_order_of_return(
        &self,
        order_of_return: &[u8],
    ) -> Result<Option<Order>, AbiDecodeError> {
        let (order, exists) = OrderBookCalls::decode_order_of_return(order_of_return)?;
        Ok(exists.then_some(order))
    }

    /// Decode returns from [`Self::calls`] in the same fixed order:
    /// `[isLive(orderHash), orderOf(orderHash)]`.
    pub fn decode_returns(
        &self,
        returns: [&[u8]; 2],
    ) -> Result<OrderLifecycleStatus, AbiDecodeError> {
        Ok(OrderLifecycleStatus {
            is_live: OrderBookCalls::decode_is_live_return(returns[0])?,
            stored_order: self.decode_order_of_return(returns[1])?,
        })
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use super::*;
    use crate::{DomainSeparatorInput, Order, OrderSignature, BASE_SCALE, PRICE_SCALE};

    fn order() -> Order {
        Order::new(
            7,
            1,
            true,
            65_000 * PRICE_SCALE,
            BASE_SCALE,
            1,
            1_717_000_000,
            false,
        )
    }

    fn signed_order() -> SignedOrder {
        let signature = OrderSignature::from_bytes([1u8; OrderSignature::LEN]).expect("valid");
        order()
            .prepare(DomainSeparatorInput::new(11111, Address::ZERO))
            .attach_signature(signature)
    }

    #[test]
    fn builds_order_lifecycle_calls() {
        let plan = OrderLifecyclePlan::new(Address::repeat_byte(0x20), signed_order());
        let order_hash = plan.order_hash();

        let submit = plan.submit_tx();
        assert_eq!(submit.to, Address::repeat_byte(0x20));
        assert_eq!(&submit.data[..4], &SignedOrder::submit_order_selector());

        let cancel = plan.cancel_tx();
        assert_eq!(cancel.to, Address::repeat_byte(0x20));
        assert_eq!(&cancel.data[..4], &OrderBookCalls::cancel_order_selector());
        assert_eq!(&cancel.data[4..36], order_hash.as_slice());

        let is_live = plan.is_live_call();
        assert_eq!(is_live.to, Address::repeat_byte(0x20));
        assert_eq!(&is_live.data[..4], &OrderBookCalls::is_live_selector());
        assert_eq!(&is_live.data[4..36], order_hash.as_slice());

        let order_of = plan.order_of_call();
        assert_eq!(order_of.to, Address::repeat_byte(0x20));
        assert_eq!(&order_of.data[..4], &OrderBookCalls::order_of_selector());
        assert_eq!(&order_of.data[4..36], order_hash.as_slice());

        assert_eq!(plan.calls(), [is_live, order_of]);
    }

    #[test]
    fn decodes_order_lifecycle_status() {
        let plan = OrderLifecyclePlan::new(Address::repeat_byte(0x20), signed_order());
        let mut yes = [0u8; 32];
        yes[31] = 1;

        let decoded = plan.decode_is_live_return(&yes).expect("status decodes");

        assert_eq!(
            decoded,
            OrderLifecycleStatus {
                is_live: true,
                stored_order: None,
            }
        );
    }

    #[test]
    fn decodes_order_lifecycle_returns_in_call_order() {
        let plan = OrderLifecyclePlan::new(Address::repeat_byte(0x20), signed_order());
        let mut yes = [0u8; 32];
        yes[31] = 1;
        let mut order_return = Vec::new();
        for word in [7u8, 1, 1, 9, 8, 6, 5, 0, 1] {
            let mut encoded = [0u8; 32];
            encoded[31] = word;
            order_return.extend_from_slice(&encoded);
        }

        let decoded = plan
            .decode_returns([&yes, &order_return])
            .expect("status decodes");

        assert_eq!(
            decoded,
            OrderLifecycleStatus {
                is_live: true,
                stored_order: Some(Order::new(7, 1, true, 9, 8, 6, 5, false)),
            }
        );
    }

    #[test]
    fn decodes_missing_order_lifecycle_return() {
        let plan = OrderLifecyclePlan::new(Address::repeat_byte(0x20), signed_order());
        let no = [0u8; 32];
        let missing_order = [0u8; 288];

        let decoded = plan
            .decode_returns([&no, &missing_order])
            .expect("status decodes");

        assert_eq!(
            decoded,
            OrderLifecycleStatus {
                is_live: false,
                stored_order: None,
            }
        );
    }

    #[test]
    fn current_arc_manifest_has_no_orderbook_plan() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        assert!(OrderLifecyclePlan::from_manifest(&manifest, signed_order()).is_none());
    }

    #[test]
    fn builds_orderbook_maintenance_calls() {
        let plan = OrderBookMaintenancePlan::new(Address::repeat_byte(0x20));
        let tick = plan.tick_tx();
        let [tick_from_batch] = plan.transactions();

        assert_eq!(tick.to, Address::repeat_byte(0x20));
        assert_eq!(tick_from_batch, tick);
        assert_eq!(tick.data, OrderBookCalls::tick_selector());
    }

    #[test]
    fn current_arc_manifest_has_no_maintenance_plan() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");

        assert!(OrderBookMaintenancePlan::from_manifest(&manifest).is_none());
    }
}
