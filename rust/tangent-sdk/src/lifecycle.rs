//! High-level order lifecycle helpers built from signed orders and raw ABI calls.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    AbiDecodeError, CallReturnBatch, DeploymentManifest, Order, OrderBookCalls, SignedOrder,
    UnsignedCall, UnsignedCallBatchSummary, UnsignedCallSummary, UnsignedTx,
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

/// Compact review shape for submit/cancel/read planning around one signed order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderLifecyclePlanSummary {
    pub order_book: Address,
    pub order_hash: alloy_primitives::B256,
    pub submit_transaction: UnsignedCallSummary,
    pub cancel_transaction: UnsignedCallSummary,
    pub read_summary: UnsignedCallBatchSummary,
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

/// Local classification for decoded order lifecycle reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderLifecycleState {
    /// The book knows the order and `isLive(orderHash)` is true.
    Live,
    /// The book knows the order, but it is no longer live.
    NotLive,
    /// The book does not know the order hash.
    Unknown,
}

/// Compact review shape for decoded lifecycle status and the next cancel action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderLifecycleSummary {
    pub order_book: Address,
    pub order_hash: alloy_primitives::B256,
    pub state: OrderLifecycleState,
    pub is_live: bool,
    pub is_known: bool,
    pub can_cancel: bool,
    pub stored_order: Option<Order>,
    pub read_summary: UnsignedCallBatchSummary,
    #[serde(default)]
    pub has_cancel_transaction: bool,
    pub cancel_transaction: Option<UnsignedCallSummary>,
}

impl OrderLifecycleStatus {
    /// True when `orderOf(orderHash)` reported that the order exists.
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.stored_order.is_some()
    }

    /// Classify the decoded order state from `isLive` and `orderOf`.
    #[must_use]
    pub fn state(&self) -> OrderLifecycleState {
        match (self.is_live, self.stored_order.is_some()) {
            (true, true) => OrderLifecycleState::Live,
            (false, true) => OrderLifecycleState::NotLive,
            (false, false) => OrderLifecycleState::Unknown,
            (true, false) => OrderLifecycleState::Unknown,
        }
    }

    /// True when this order can still be cancelled according to decoded reads.
    #[must_use]
    pub fn can_cancel(&self) -> bool {
        self.state() == OrderLifecycleState::Live
    }
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
        self.signed_order.order_hash()
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

    #[must_use]
    pub fn read_summary(&self) -> UnsignedCallBatchSummary {
        let calls = self.calls();
        UnsignedCall::summarize_batch(&calls)
    }

    #[must_use]
    pub fn summary(&self) -> OrderLifecyclePlanSummary {
        OrderLifecyclePlanSummary {
            order_book: self.order_book,
            order_hash: self.order_hash(),
            submit_transaction: self.submit_tx().summary(),
            cancel_transaction: self.cancel_tx().summary(),
            read_summary: self.read_summary(),
        }
    }

    #[must_use]
    pub fn status_summary(&self, status: &OrderLifecycleStatus) -> OrderLifecycleSummary {
        let cancel_transaction = status.can_cancel().then(|| self.cancel_tx().summary());
        OrderLifecycleSummary {
            order_book: self.order_book,
            order_hash: self.order_hash(),
            state: status.state(),
            is_live: status.is_live,
            is_known: status.is_known(),
            can_cancel: status.can_cancel(),
            stored_order: status.stored_order.clone(),
            read_summary: self.read_summary(),
            has_cancel_transaction: cancel_transaction.is_some(),
            cancel_transaction,
        }
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
        let is_live = OrderBookCalls::decode_is_live_return(returns[0])?;
        let stored_order = self.decode_order_of_return(returns[1])?;

        if is_live && stored_order.is_none() {
            return Err(AbiDecodeError::InconsistentData(
                "isLive returned true but orderOf returned missing order",
            ));
        }

        Ok(OrderLifecycleStatus {
            is_live,
            stored_order,
        })
    }

    /// Decode a transport-returned batch from [`Self::calls`].
    pub fn decode_return_slices<T: AsRef<[u8]>>(
        &self,
        returns: &[T],
    ) -> Result<OrderLifecycleStatus, AbiDecodeError> {
        let returns = crate::abi::expect_return_count(returns, 2)?;
        self.decode_returns([returns[0], returns[1]])
    }

    /// Decode an ordered transport-returned batch from [`Self::calls`].
    pub fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<OrderLifecycleStatus, AbiDecodeError> {
        self.decode_return_slices(returns.as_returns())
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
        let summary = plan.summary();
        assert_eq!(summary.order_book, Address::repeat_byte(0x20));
        assert_eq!(summary.order_hash, order_hash);
        assert_eq!(summary.submit_transaction.to, Address::repeat_byte(0x20));
        assert_eq!(summary.cancel_transaction.to, Address::repeat_byte(0x20));
        assert_eq!(summary.read_summary.len, 2);
        let json = serde_json::to_string(&summary).expect("summary serializes");
        let restored: OrderLifecyclePlanSummary =
            serde_json::from_str(&json).expect("summary deserializes");
        assert_eq!(restored, summary);
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
            plan.decode_return_slices(&[yes.to_vec(), order_return.clone()])
                .expect("status decodes from slices"),
            decoded
        );
        let batch = CallReturnBatch::new(vec![
            crate::CallReturn::new(yes.to_vec()),
            crate::CallReturn::new(order_return.clone()),
        ]);
        assert_eq!(
            plan.decode_return_batch(&batch)
                .expect("status decodes from batch"),
            decoded
        );

        assert_eq!(
            decoded,
            OrderLifecycleStatus {
                is_live: true,
                stored_order: Some(Order::new(7, 1, true, 9, 8, 6, 5, false)),
            }
        );
        assert!(decoded.is_known());
        assert_eq!(decoded.state(), OrderLifecycleState::Live);
        assert!(decoded.can_cancel());
        let summary = plan.status_summary(&decoded);
        assert_eq!(summary.order_book, Address::repeat_byte(0x20));
        assert_eq!(summary.order_hash, plan.order_hash());
        assert_eq!(summary.state, OrderLifecycleState::Live);
        assert!(summary.is_live);
        assert!(summary.is_known);
        assert!(summary.can_cancel);
        assert_eq!(summary.stored_order, decoded.stored_order);
        assert_eq!(summary.read_summary.len, 2);
        assert!(summary.has_cancel_transaction);
        assert_eq!(
            summary
                .cancel_transaction
                .as_ref()
                .expect("cancel summary")
                .to,
            Address::repeat_byte(0x20)
        );
        let json = serde_json::to_string(&summary).expect("status summary serializes");
        let restored: OrderLifecycleSummary =
            serde_json::from_str(&json).expect("status summary deserializes");
        assert_eq!(restored, summary);
        let mut legacy_json = serde_json::to_value(&summary).expect("status summary value");
        let legacy_object = legacy_json.as_object_mut().expect("status summary object");
        legacy_object.remove("has_cancel_transaction");
        let legacy: OrderLifecycleSummary =
            serde_json::from_value(legacy_json).expect("legacy status summary");
        assert!(!legacy.has_cancel_transaction);
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
        assert!(!decoded.is_known());
        assert_eq!(decoded.state(), OrderLifecycleState::Unknown);
        assert!(!decoded.can_cancel());
        let summary = plan.status_summary(&decoded);
        assert_eq!(summary.state, OrderLifecycleState::Unknown);
        assert!(!summary.is_known);
        assert!(!summary.has_cancel_transaction);
        assert_eq!(summary.cancel_transaction, None);
    }

    #[test]
    fn classifies_known_not_live_order_lifecycle_return() {
        let plan = OrderLifecyclePlan::new(Address::repeat_byte(0x20), signed_order());
        let no = [0u8; 32];
        let mut order_return = Vec::new();
        for word in [7u8, 1, 1, 9, 8, 6, 5, 0, 1] {
            let mut encoded = [0u8; 32];
            encoded[31] = word;
            order_return.extend_from_slice(&encoded);
        }

        let decoded = plan
            .decode_returns([&no, &order_return])
            .expect("status decodes");

        assert!(decoded.is_known());
        assert_eq!(decoded.state(), OrderLifecycleState::NotLive);
        assert!(!decoded.can_cancel());
        let summary = plan.status_summary(&decoded);
        assert_eq!(summary.state, OrderLifecycleState::NotLive);
        assert!(summary.is_known);
        assert!(!summary.has_cancel_transaction);
        assert_eq!(summary.cancel_transaction, None);
    }

    #[test]
    fn rejects_live_missing_order_lifecycle_return() {
        let plan = OrderLifecyclePlan::new(Address::repeat_byte(0x20), signed_order());
        let mut yes = [0u8; 32];
        yes[31] = 1;
        let missing_order = [0u8; 288];

        assert_eq!(
            plan.decode_returns([&yes, &missing_order])
                .expect_err("live order must be known"),
            AbiDecodeError::InconsistentData(
                "isLive returned true but orderOf returned missing order",
            )
        );
    }

    #[test]
    fn rejects_inconsistent_missing_order_lifecycle_return() {
        let plan = OrderLifecyclePlan::new(Address::repeat_byte(0x20), signed_order());
        let no = [0u8; 32];
        let mut inconsistent_missing_order = [0u8; 288];
        inconsistent_missing_order[31] = 7;

        assert_eq!(
            plan.decode_returns([&no, &inconsistent_missing_order])
                .expect_err("inconsistent missing order"),
            AbiDecodeError::InconsistentData(
                "orderOf returned exists=false with non-zero order fields",
            )
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
