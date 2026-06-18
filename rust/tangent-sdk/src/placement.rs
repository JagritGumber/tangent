//! Order placement planning across market reads, signing, and lifecycle calls.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    DomainSeparatorInput, MarketReadPlan, MarketReadSummary, Order, OrderError, OrderLifecyclePlan,
    OrderParams, OrderSigner, PreparedOrder, SignedOrder, UnsignedCall, UnsignedCallBatchSummary,
    UnsignedCallSummary, UnsignedTx,
};

/// Transport-neutral plan for building, validating, and signing one order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderPlacementPlan {
    pub order_book: Address,
    pub market_plan: MarketReadPlan,
    pub domain: DomainSeparatorInput,
    pub params: OrderParams,
    pub current_timestamp: u64,
}

/// Signed placement ready to submit/cancel/read through `OrderBook`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderPlacement {
    pub market_plan: MarketReadPlan,
    pub lifecycle: OrderLifecyclePlan,
}

/// Compact review shape for an order placement plan before market reads/signing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderPlacementPlanSummary {
    pub order_book: Address,
    pub market_registry: Address,
    pub domain_chain_id: u64,
    pub market_id: u128,
    pub account_id: u128,
    pub is_buy: bool,
    pub limit_price: u128,
    pub size: u128,
    pub nonce: u128,
    pub expiry: u64,
    pub reduce_only: bool,
    pub current_timestamp: u64,
    pub market_read_summary: UnsignedCallBatchSummary,
}

/// Compact review shape for a signed order placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderPlacementSummary {
    pub order_book: Address,
    pub market_registry: Address,
    pub market_id: u128,
    pub account_id: u128,
    pub order_hash: String,
    pub submit_transaction: UnsignedCallSummary,
    pub cancel_transaction: UnsignedCallSummary,
    pub lifecycle_read_summary: UnsignedCallBatchSummary,
}

/// Errors surfaced while signing an order placement.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OrderPlacementSignError<E> {
    #[error(transparent)]
    Order(#[from] OrderError),
    #[error("order placement signer failed")]
    Signer(E),
}

impl OrderPlacementPlan {
    #[must_use]
    pub fn new(
        order_book: Address,
        market_registry: Address,
        chain_id: u64,
        params: OrderParams,
        current_timestamp: u64,
    ) -> Self {
        let market_id = params.market_id;
        Self {
            order_book,
            market_plan: MarketReadPlan::new(market_registry, market_id),
            domain: DomainSeparatorInput::new(chain_id, order_book),
            params,
            current_timestamp,
        }
    }

    #[must_use]
    pub const fn with_domain(
        order_book: Address,
        market_plan: MarketReadPlan,
        domain: DomainSeparatorInput,
        params: OrderParams,
        current_timestamp: u64,
    ) -> Self {
        Self {
            order_book,
            market_plan,
            domain,
            params,
            current_timestamp,
        }
    }

    #[must_use]
    pub fn market_calls(&self) -> [UnsignedCall; 3] {
        self.market_plan.calls()
    }

    #[must_use]
    pub fn summary(&self) -> OrderPlacementPlanSummary {
        let market_calls = self.market_calls();
        OrderPlacementPlanSummary {
            order_book: self.order_book,
            market_registry: self.market_plan.market_registry,
            domain_chain_id: self.domain.chain_id,
            market_id: self.params.market_id,
            account_id: self.params.account_id,
            is_buy: self.params.side.is_buy(),
            limit_price: self.params.limit_price,
            size: self.params.size,
            nonce: self.params.nonce,
            expiry: self.params.expiry,
            reduce_only: self.params.reduce_only,
            current_timestamp: self.current_timestamp,
            market_read_summary: UnsignedCall::summarize_batch(&market_calls),
        }
    }

    pub fn build_order(&self, summary: &MarketReadSummary) -> Result<Order, OrderError> {
        let constraints = summary
            .order_constraints()
            .ok_or_else(|| OrderError::Invalid("market metadata is missing".into()))?;
        let order = self
            .params
            .clone()
            .build(constraints, self.current_timestamp)?;
        self.market_plan
            .validate_order_with_summary(summary, &order, self.current_timestamp)?;
        Ok(order)
    }

    pub fn prepare(&self, summary: &MarketReadSummary) -> Result<PreparedOrder, OrderError> {
        Ok(self.build_order(summary)?.prepare(self.domain.clone()))
    }

    pub fn sign_with<S: OrderSigner>(
        &self,
        summary: &MarketReadSummary,
        signer: &mut S,
    ) -> Result<OrderPlacement, OrderPlacementSignError<S::Error>> {
        let signed_order = self
            .prepare(summary)?
            .sign_with(signer)
            .map_err(OrderPlacementSignError::Signer)?;
        Ok(self.with_signed_order(signed_order))
    }

    #[must_use]
    pub fn with_signed_order(&self, signed_order: SignedOrder) -> OrderPlacement {
        OrderPlacement {
            market_plan: self.market_plan.clone(),
            lifecycle: OrderLifecyclePlan::new(self.order_book, signed_order),
        }
    }
}

impl OrderPlacement {
    #[must_use]
    pub fn signed_order(&self) -> &SignedOrder {
        &self.lifecycle.signed_order
    }

    #[must_use]
    pub fn submit_tx(&self) -> UnsignedTx {
        self.lifecycle.submit_tx()
    }

    #[must_use]
    pub fn cancel_tx(&self) -> UnsignedTx {
        self.lifecycle.cancel_tx()
    }

    #[must_use]
    pub fn lifecycle_calls(&self) -> [UnsignedCall; 2] {
        self.lifecycle.calls()
    }

    #[must_use]
    pub fn summary(&self) -> OrderPlacementSummary {
        let lifecycle_calls = self.lifecycle_calls();
        OrderPlacementSummary {
            order_book: self.lifecycle.order_book,
            market_registry: self.market_plan.market_registry,
            market_id: self.signed_order().order.market_id,
            account_id: self.signed_order().order.account_id,
            order_hash: self.signed_order().order_hash_hex(),
            submit_transaction: self.submit_tx().summary(),
            cancel_transaction: self.cancel_tx().summary(),
            lifecycle_read_summary: UnsignedCall::summarize_batch(&lifecycle_calls),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Side, BASE_SCALE, PRICE_SCALE};

    #[derive(Debug, Default)]
    struct MockOrderSigner;

    impl OrderSigner for MockOrderSigner {
        type Error = &'static str;

        fn sign_order(
            &mut self,
            _request: &crate::OrderSigningRequest,
        ) -> Result<crate::OrderSignature, Self::Error> {
            crate::OrderSignature::from_bytes([3u8; crate::OrderSignature::LEN])
                .map_err(|_| "bad signature")
        }
    }

    fn params() -> OrderParams {
        OrderParams {
            account_id: 7,
            market_id: 1,
            side: Side::Buy,
            limit_price: 65_000 * PRICE_SCALE,
            size: BASE_SCALE,
            nonce: 1,
            expiry: 1_717_000_000,
            reduce_only: false,
        }
    }

    fn summary() -> MarketReadSummary {
        MarketReadSummary {
            total_markets: 1,
            mark_price: 65_000 * PRICE_SCALE,
            market: Some(crate::MarketDetails {
                symbol: "BTC".to_owned(),
                price_feed: Address::repeat_byte(0x20),
                initial_margin_bps: 1_000,
                maint_margin_bps: 500,
                max_leverage: 10,
                tick_size: 100,
                lot_size: 1_000_000_000_000_000,
                max_price_age: 60,
                paused: false,
            }),
        }
    }

    #[test]
    fn placement_plan_builds_prepared_and_signed_order() {
        let plan = OrderPlacementPlan::new(
            Address::repeat_byte(0x11),
            Address::repeat_byte(0x12),
            11111,
            params(),
            1_716_999_000,
        );

        assert_eq!(plan.market_plan.market_id, 1);
        assert_eq!(plan.market_calls().len(), 3);
        let plan_summary = plan.summary();
        assert_eq!(plan_summary.order_book, Address::repeat_byte(0x11));
        assert_eq!(plan_summary.market_registry, Address::repeat_byte(0x12));
        assert_eq!(plan_summary.domain_chain_id, 11111);
        assert_eq!(plan_summary.market_id, 1);
        assert_eq!(plan_summary.account_id, 7);
        assert!(plan_summary.is_buy);
        assert_eq!(plan_summary.limit_price, 65_000 * PRICE_SCALE);
        assert_eq!(plan_summary.size, BASE_SCALE);
        assert_eq!(plan_summary.market_read_summary.len, 3);
        assert_eq!(plan_summary.market_read_summary.unique_contracts, 1);

        let prepared = plan.prepare(&summary()).expect("prepared order");
        assert_eq!(prepared.order.market_id, 1);
        assert_eq!(
            prepared.domain.verifying_contract,
            Address::repeat_byte(0x11)
        );

        let placement = plan
            .sign_with(&summary(), &mut MockOrderSigner)
            .expect("signed placement");
        assert_eq!(placement.market_plan, plan.market_plan);
        assert_eq!(placement.lifecycle.order_book, Address::repeat_byte(0x11));
        assert_eq!(placement.submit_tx().to, Address::repeat_byte(0x11));
        assert_eq!(placement.cancel_tx().to, Address::repeat_byte(0x11));
        assert_eq!(placement.lifecycle_calls().len(), 2);
        let placement_summary = placement.summary();
        assert_eq!(placement_summary.order_book, Address::repeat_byte(0x11));
        assert_eq!(
            placement_summary.market_registry,
            Address::repeat_byte(0x12)
        );
        assert_eq!(placement_summary.market_id, 1);
        assert_eq!(placement_summary.account_id, 7);
        assert_eq!(
            placement_summary.order_hash,
            placement.signed_order().order_hash_hex()
        );
        assert_eq!(
            placement_summary.submit_transaction.to,
            Address::repeat_byte(0x11)
        );
        assert!(placement_summary.submit_transaction.selector.is_some());
        assert_eq!(
            placement_summary.cancel_transaction.to,
            Address::repeat_byte(0x11)
        );
        assert!(placement_summary.cancel_transaction.selector.is_some());
        assert_eq!(placement_summary.lifecycle_read_summary.len, 2);
        assert_eq!(
            placement.signed_order().signature,
            crate::OrderSignature::from_bytes([3u8; crate::OrderSignature::LEN]).unwrap()
        );
        let plan_json = serde_json::to_string(&plan_summary).expect("plan summary serializes");
        let restored_plan: OrderPlacementPlanSummary =
            serde_json::from_str(&plan_json).expect("plan summary deserializes");
        assert_eq!(restored_plan, plan_summary);
        let placement_json =
            serde_json::to_string(&placement_summary).expect("placement summary serializes");
        let restored_placement: OrderPlacementSummary =
            serde_json::from_str(&placement_json).expect("placement summary deserializes");
        assert_eq!(restored_placement, placement_summary);
    }

    #[test]
    fn placement_plan_rejects_unusable_market_summary() {
        let plan = OrderPlacementPlan::new(
            Address::repeat_byte(0x11),
            Address::repeat_byte(0x12),
            11111,
            params(),
            1_716_999_000,
        );
        let mut missing_market = summary();
        missing_market.market = None;
        assert_eq!(
            plan.prepare(&missing_market),
            Err(OrderError::Invalid("market metadata is missing".to_owned()))
        );

        let mut unregistered = summary();
        unregistered.total_markets = 0;
        assert_eq!(
            plan.prepare(&unregistered),
            Err(OrderError::Invalid(
                "market_id is not registered".to_owned()
            ))
        );
    }
}
