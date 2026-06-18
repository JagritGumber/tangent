//! Manifest-bound SDK context and plan factory.
//!
//! `TangentContext` carries deployment addresses and chain id in one place so
//! downstream callers can build typed SDK plans without threading raw manifest
//! fields through every workflow.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    AccountOnboardingPlan, AccountStatusPlan, CollateralDepositPlan, CollateralStatusPlan,
    CollateralWithdrawPlan, DeploymentManifest, DomainSeparatorInput, EventFilterSet,
    KeeperCapability, KeeperRuntimePlan, LiquidationReadPlan, MarketReadPlan, Order,
    OrderBookMaintenancePlan, OrderLifecyclePlan, OrderParams, OrderPlacementPlan,
    PerpStackAvailability, PreparedOrder, SettlementReadPlan, SignedOrder,
};

/// Manifest-bound factory for Tangent SDK workflow plans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentContext {
    manifest: DeploymentManifest,
}

/// Compact manifest-bound capability summary for fork/reference clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentContextSummary {
    pub project: String,
    pub version: String,
    pub chain_id: u64,
    pub network: String,
    pub deployer: Address,
    pub account_manager: Address,
    pub usdc_vault: Address,
    pub market_registry: Address,
    pub usdc: Address,
    pub order_book: Option<Address>,
    #[serde(default)]
    pub has_order_book: bool,
    pub settlement_engine: Option<Address>,
    #[serde(default)]
    pub has_settlement_engine: bool,
    pub liquidation_keeper: Option<Address>,
    #[serde(default)]
    pub has_liquidation_keeper: bool,
    pub verified_on_arcscan: bool,
    pub perp_stack_availability: PerpStackAvailability,
    #[serde(default)]
    pub has_perp_stack: bool,
    pub missing_perp_contracts: Vec<String>,
    #[serde(default)]
    pub has_missing_perp_contracts: bool,
    pub order_book_domain: Option<DomainSeparatorInput>,
    #[serde(default)]
    pub has_order_book_domain: bool,
    pub event_filter_count: usize,
    #[serde(default)]
    pub has_event_filters: bool,
    pub keeper_capabilities: Vec<KeeperCapability>,
    #[serde(default)]
    pub has_keeper_capabilities: bool,
}

/// Errors surfaced while building context-bound full-stack plans.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TangentContextError {
    #[error("deployment manifest does not include an OrderBook address")]
    MissingOrderBook,
    #[error("deployment manifest does not include a LiquidationKeeper address")]
    MissingLiquidationKeeper,
}

impl TangentContext {
    #[must_use]
    pub const fn new(manifest: DeploymentManifest) -> Self {
        Self { manifest }
    }

    #[must_use]
    pub const fn manifest(&self) -> &DeploymentManifest {
        &self.manifest
    }

    #[must_use]
    pub fn into_manifest(self) -> DeploymentManifest {
        self.manifest
    }

    #[must_use]
    pub fn summary(&self) -> TangentContextSummary {
        let availability = self.manifest.perp_stack_availability();
        let keeper_runtime = self.keeper_runtime();
        let order_book_domain = self.order_book_domain();
        let event_filter_count = self.event_filters().len();
        let keeper_capabilities = keeper_runtime.capabilities();
        let missing_perp_contracts = availability
            .missing_contracts()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        TangentContextSummary {
            project: self.manifest.project.clone(),
            version: self.manifest.version.clone(),
            chain_id: self.manifest.chain_id,
            network: self.manifest.network.clone(),
            deployer: self.manifest.deployer,
            account_manager: self.manifest.contracts.account_manager,
            usdc_vault: self.manifest.contracts.usdc_vault,
            market_registry: self.manifest.contracts.market_registry,
            usdc: self.manifest.constants.usdc,
            order_book: self.manifest.contracts.order_book,
            has_order_book: self.manifest.contracts.order_book.is_some(),
            settlement_engine: self.manifest.contracts.settlement_engine,
            has_settlement_engine: self.manifest.contracts.settlement_engine.is_some(),
            liquidation_keeper: self.manifest.contracts.liquidation_keeper,
            has_liquidation_keeper: self.manifest.contracts.liquidation_keeper.is_some(),
            verified_on_arcscan: self.manifest.verified_on_arcscan,
            perp_stack_availability: availability,
            has_perp_stack: availability.is_complete(),
            has_missing_perp_contracts: !missing_perp_contracts.is_empty(),
            missing_perp_contracts,
            has_order_book_domain: order_book_domain.is_some(),
            order_book_domain,
            event_filter_count,
            has_event_filters: event_filter_count > 0,
            has_keeper_capabilities: !keeper_capabilities.is_empty(),
            keeper_capabilities,
        }
    }

    #[must_use]
    pub const fn chain_id(&self) -> u64 {
        self.manifest.chain_id
    }

    #[must_use]
    pub const fn deployer(&self) -> Address {
        self.manifest.deployer
    }

    #[must_use]
    pub fn order_book_domain(&self) -> Option<DomainSeparatorInput> {
        self.manifest.order_book_domain()
    }

    pub fn require_order_book_domain(&self) -> Result<DomainSeparatorInput, TangentContextError> {
        self.order_book_domain()
            .ok_or(TangentContextError::MissingOrderBook)
    }

    pub fn prepare_order(&self, order: Order) -> Result<PreparedOrder, TangentContextError> {
        Ok(order.prepare(self.require_order_book_domain()?))
    }

    pub fn order_placement(
        &self,
        params: OrderParams,
        current_timestamp: u64,
    ) -> Result<OrderPlacementPlan, TangentContextError> {
        let order_book = self
            .manifest
            .contracts
            .order_book
            .ok_or(TangentContextError::MissingOrderBook)?;
        Ok(OrderPlacementPlan::new(
            order_book,
            self.manifest.contracts.market_registry,
            self.manifest.chain_id,
            params,
            current_timestamp,
        ))
    }

    #[must_use]
    pub fn event_filters(&self) -> EventFilterSet {
        EventFilterSet::from_manifest(&self.manifest)
    }

    #[must_use]
    pub fn keeper_runtime(&self) -> KeeperRuntimePlan {
        KeeperRuntimePlan::from_manifest(&self.manifest)
    }

    #[must_use]
    pub fn account_onboarding(&self, owner: Address) -> AccountOnboardingPlan {
        AccountOnboardingPlan::from_manifest(&self.manifest, owner)
    }

    #[must_use]
    pub fn account_status(&self, owner: Address, account_id: u128) -> AccountStatusPlan {
        AccountStatusPlan::from_manifest(&self.manifest, owner, account_id)
    }

    #[must_use]
    pub fn collateral_deposit(&self, account_id: u128, amount: u128) -> CollateralDepositPlan {
        CollateralDepositPlan::from_manifest(&self.manifest, account_id, amount)
    }

    #[must_use]
    pub fn collateral_withdraw(
        &self,
        account_id: u128,
        amount: u128,
        recipient: Address,
    ) -> CollateralWithdrawPlan {
        CollateralWithdrawPlan::from_manifest(&self.manifest, account_id, amount, recipient)
    }

    #[must_use]
    pub fn collateral_status(&self, owner: Address, account_id: u128) -> CollateralStatusPlan {
        CollateralStatusPlan::from_manifest(&self.manifest, owner, account_id)
    }

    #[must_use]
    pub fn market(&self, market_id: u128) -> MarketReadPlan {
        MarketReadPlan::from_manifest(&self.manifest, market_id)
    }

    #[must_use]
    pub fn order_lifecycle(&self, signed_order: SignedOrder) -> Option<OrderLifecyclePlan> {
        OrderLifecyclePlan::from_manifest(&self.manifest, signed_order)
    }

    #[must_use]
    pub fn orderbook_maintenance(&self) -> Option<OrderBookMaintenancePlan> {
        OrderBookMaintenancePlan::from_manifest(&self.manifest)
    }

    #[must_use]
    pub fn settlement(&self, account_id: u128, market_id: u128) -> Option<SettlementReadPlan> {
        SettlementReadPlan::from_manifest(&self.manifest, account_id, market_id)
    }

    #[must_use]
    pub fn liquidation(&self, account_id: u128, market_id: u128) -> Option<LiquidationReadPlan> {
        LiquidationReadPlan::from_manifest(&self.manifest, account_id, market_id)
    }
}

impl From<DeploymentManifest> for TangentContext {
    fn from(manifest: DeploymentManifest) -> Self {
        Self::new(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContractAddresses, NetworkConstants, OrderParams, OrderSignature, PerpStackAvailability,
        Side, BASE_SCALE, PRICE_SCALE,
    };

    fn current_manifest() -> DeploymentManifest {
        DeploymentManifest::from_json(include_str!("../../../docs/deployments/arc-testnet.json"))
            .expect("manifest parses")
    }

    fn full_manifest() -> DeploymentManifest {
        DeploymentManifest {
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
        }
    }

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

    fn signed_order(context: &TangentContext) -> SignedOrder {
        context
            .prepare_order(order())
            .expect("domain available")
            .attach_signature(OrderSignature::from_bytes([1u8; OrderSignature::LEN]).unwrap())
    }

    #[test]
    fn context_builds_primitive_plans_from_current_manifest() {
        let manifest = current_manifest();
        let context = TangentContext::new(manifest.clone());

        assert_eq!(context.chain_id(), manifest.chain_id);
        assert_eq!(context.deployer(), manifest.deployer);
        assert_eq!(context.manifest(), &manifest);
        let summary = context.summary();
        assert_eq!(summary.project, manifest.project);
        assert_eq!(summary.version, manifest.version);
        assert_eq!(summary.chain_id, manifest.chain_id);
        assert_eq!(summary.network, manifest.network);
        assert_eq!(summary.deployer, manifest.deployer);
        assert_eq!(summary.account_manager, manifest.contracts.account_manager);
        assert_eq!(summary.usdc_vault, manifest.contracts.usdc_vault);
        assert_eq!(summary.market_registry, manifest.contracts.market_registry);
        assert_eq!(summary.usdc, manifest.constants.usdc);
        assert_eq!(summary.order_book, None);
        assert!(!summary.has_order_book);
        assert_eq!(summary.settlement_engine, None);
        assert!(!summary.has_settlement_engine);
        assert_eq!(summary.liquidation_keeper, None);
        assert!(!summary.has_liquidation_keeper);
        assert_eq!(
            summary.perp_stack_availability,
            PerpStackAvailability {
                order_book: false,
                settlement_engine: false,
                liquidation_keeper: false,
            }
        );
        assert!(!summary.has_perp_stack);
        assert_eq!(
            summary.missing_perp_contracts,
            vec![
                "OrderBook".to_owned(),
                "SettlementEngine".to_owned(),
                "LiquidationKeeper".to_owned()
            ]
        );
        assert!(summary.has_missing_perp_contracts);
        assert_eq!(summary.order_book_domain, None);
        assert!(!summary.has_order_book_domain);
        assert_eq!(summary.event_filter_count, context.event_filters().len());
        assert!(summary.has_event_filters);
        assert_eq!(
            summary.keeper_capabilities,
            vec![KeeperCapability::EventIndexing]
        );
        assert!(summary.has_keeper_capabilities);
        let json = serde_json::to_string(&summary).expect("summary serializes");
        assert!(json.contains("\"has_missing_perp_contracts\":true"));
        let restored: TangentContextSummary =
            serde_json::from_str(&json).expect("summary deserializes");
        assert_eq!(restored, summary);
        let mut legacy_json = serde_json::to_value(&summary).expect("summary value");
        let legacy_object = legacy_json.as_object_mut().expect("summary object");
        legacy_object.remove("has_order_book");
        legacy_object.remove("has_settlement_engine");
        legacy_object.remove("has_liquidation_keeper");
        legacy_object.remove("has_perp_stack");
        legacy_object.remove("has_missing_perp_contracts");
        legacy_object.remove("has_order_book_domain");
        legacy_object.remove("has_event_filters");
        legacy_object.remove("has_keeper_capabilities");
        let legacy_summary: TangentContextSummary =
            serde_json::from_value(legacy_json).expect("legacy summary deserializes");
        assert!(!legacy_summary.has_order_book);
        assert!(!legacy_summary.has_settlement_engine);
        assert!(!legacy_summary.has_liquidation_keeper);
        assert!(!legacy_summary.has_perp_stack);
        assert!(!legacy_summary.has_missing_perp_contracts);
        assert!(!legacy_summary.has_order_book_domain);
        assert!(!legacy_summary.has_event_filters);
        assert!(!legacy_summary.has_keeper_capabilities);
        assert_eq!(context.order_book_domain(), None);
        assert_eq!(
            context.require_order_book_domain(),
            Err(TangentContextError::MissingOrderBook)
        );
        assert_eq!(
            context.prepare_order(order()),
            Err(TangentContextError::MissingOrderBook)
        );
        assert_eq!(
            context.order_placement(params(), 1_716_999_000),
            Err(TangentContextError::MissingOrderBook)
        );
        assert_eq!(
            context.event_filters().len(),
            EventFilterSet::from_manifest(&manifest).len()
        );
        assert_eq!(
            context.account_onboarding(manifest.deployer),
            AccountOnboardingPlan::from_manifest(&manifest, manifest.deployer)
        );
        assert_eq!(
            context.account_status(manifest.deployer, 1),
            AccountStatusPlan::from_manifest(&manifest, manifest.deployer, 1)
        );
        assert_eq!(
            context.collateral_deposit(1, 500),
            CollateralDepositPlan::from_manifest(&manifest, 1, 500)
        );
        assert_eq!(
            context.collateral_withdraw(1, 500, manifest.deployer),
            CollateralWithdrawPlan::from_manifest(&manifest, 1, 500, manifest.deployer)
        );
        assert_eq!(
            context.collateral_status(manifest.deployer, 1),
            CollateralStatusPlan::from_manifest(&manifest, manifest.deployer, 1)
        );
        assert_eq!(
            context.market(1),
            MarketReadPlan::from_manifest(&manifest, 1)
        );
        assert_eq!(context.orderbook_maintenance(), None);
        assert_eq!(context.settlement(7, 1), None);
        assert_eq!(context.liquidation(7, 1), None);
        assert_eq!(context.keeper_runtime().capabilities().len(), 1);
        assert_eq!(context.clone().into_manifest(), manifest);
    }

    #[test]
    fn context_builds_full_stack_plans_when_manifest_has_addresses() {
        let manifest = full_manifest();
        let context = TangentContext::from(manifest.clone());
        let signed = signed_order(&context);

        assert_eq!(
            context.manifest().perp_stack_availability(),
            PerpStackAvailability {
                order_book: true,
                settlement_engine: true,
                liquidation_keeper: true,
            }
        );
        assert_eq!(
            context
                .order_book_domain()
                .expect("order book domain")
                .verifying_contract,
            Address::repeat_byte(0x14)
        );
        let summary = context.summary();
        assert_eq!(summary.order_book, Some(Address::repeat_byte(0x14)));
        assert!(summary.has_order_book);
        assert_eq!(summary.settlement_engine, Some(Address::repeat_byte(0x15)));
        assert!(summary.has_settlement_engine);
        assert_eq!(summary.liquidation_keeper, Some(Address::repeat_byte(0x16)));
        assert!(summary.has_liquidation_keeper);
        assert!(summary.perp_stack_availability.is_complete());
        assert!(summary.has_perp_stack);
        assert!(summary.missing_perp_contracts.is_empty());
        assert!(!summary.has_missing_perp_contracts);
        assert_eq!(
            summary
                .order_book_domain
                .expect("summary order book domain")
                .verifying_contract,
            Address::repeat_byte(0x14)
        );
        assert!(summary.has_order_book_domain);
        assert!(summary.has_event_filters);
        assert!(summary.has_keeper_capabilities);
        assert_eq!(
            summary.keeper_capabilities,
            vec![
                KeeperCapability::EventIndexing,
                KeeperCapability::OrderBookMaintenance,
                KeeperCapability::SettlementReads,
                KeeperCapability::LiquidationReads,
                KeeperCapability::FullPerpStack
            ]
        );
        assert_eq!(
            context.order_lifecycle(signed.clone()),
            Some(OrderLifecyclePlan::new(Address::repeat_byte(0x14), signed))
        );
        let placement = context
            .order_placement(params(), 1_716_999_000)
            .expect("placement plan");
        assert_eq!(placement.order_book, Address::repeat_byte(0x14));
        assert_eq!(
            placement.market_plan.market_registry,
            Address::repeat_byte(0x13)
        );
        assert_eq!(placement.domain.chain_id, 11111);
        assert_eq!(
            placement.domain.verifying_contract,
            Address::repeat_byte(0x14)
        );
        assert_eq!(
            context.orderbook_maintenance(),
            Some(OrderBookMaintenancePlan::new(Address::repeat_byte(0x14)))
        );
        assert_eq!(
            context.settlement(7, 1),
            SettlementReadPlan::from_manifest(&manifest, 7, 1)
        );
        assert_eq!(
            context.liquidation(7, 1),
            LiquidationReadPlan::from_manifest(&manifest, 7, 1)
        );
        assert!(context.keeper_runtime().is_full_perp_stack_available());
        assert_eq!(
            context
                .keeper_runtime()
                .liquidation_candidate(&manifest, 7, 1)
                .read_calls()
                .len(),
            4
        );
    }
}
