//! Keeper runtime planning helpers.
//!
//! The SDK does not run a daemon, poll blocks, or choose liquidation
//! profitability policy. This module only composes the manifest-derived pieces
//! a keeper needs at startup and for one account/market candidate.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::{
    DeploymentManifest, EventFilterSet, EventLogRpcQuery, EventLogRpcQueryBatchSummary,
    EventQueryError, LiquidationReadPlan, OrderBookMaintenancePlan, RawLogCursor,
    SettlementReadPlan, UnsignedCall, UnsignedTx,
};

/// Manifest-derived keeper capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeeperCapability {
    EventIndexing,
    OrderBookMaintenance,
    SettlementReads,
    LiquidationReads,
    FullPerpStack,
}

/// Manifest-bound startup plan for a keeper-like runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeeperRuntimePlan {
    pub chain_id: u64,
    pub network: String,
    pub event_filters: EventFilterSet,
    pub orderbook_maintenance: Option<OrderBookMaintenancePlan>,
    pub has_settlement_engine: bool,
    pub has_liquidation_keeper: bool,
}

/// Read/transaction plans for one liquidation candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeeperLiquidationCandidatePlan {
    pub settlement: Option<SettlementReadPlan>,
    pub liquidation: Option<LiquidationReadPlan>,
}

/// Block-window policy for caller-managed keeper polling loops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeeperPollingPolicy {
    pub max_event_window_blocks: u64,
    pub tick_interval_blocks: u64,
    pub liquidation_scan_interval_blocks: u64,
}

/// Caller-supplied keeper progress snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeeperPollingSnapshot {
    pub current_block: u64,
    pub event_cursor: Option<RawLogCursor>,
    pub event_from_block: Option<u64>,
    pub last_tick_block: Option<u64>,
    pub last_liquidation_scan_block: Option<u64>,
}

/// Deterministic keeper work to execute for one polling pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeeperPollingPlan {
    pub event_queries: Vec<EventLogRpcQuery>,
    pub maintenance_transactions: Vec<UnsignedTx>,
    pub should_scan_liquidations: bool,
}

/// Compact review shape for one maintenance transaction in a polling plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeeperMaintenanceTransactionSummary {
    pub to: Address,
    pub selector: Option<String>,
    #[serde(default)]
    pub has_selector: bool,
    pub calldata_bytes: usize,
    #[serde(default)]
    pub has_calldata: bool,
}

/// Serializable summary of a caller-managed keeper polling pass.
///
/// This is intended for daemon logs, dry-run output, and fork reference UIs that
/// need to inspect planned work without carrying full calldata or filter sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeeperPollingPlanSummary {
    pub event_query_count: usize,
    #[serde(default)]
    pub has_event_queries: bool,
    #[serde(default)]
    pub event_query_summary: EventLogRpcQueryBatchSummary,
    pub first_event_from_block: Option<String>,
    #[serde(default)]
    pub has_first_event_from_block: bool,
    pub last_event_to_block: Option<String>,
    #[serde(default)]
    pub has_last_event_to_block: bool,
    pub maintenance_transaction_count: usize,
    #[serde(default)]
    pub has_maintenance_transactions: bool,
    pub maintenance_transactions: Vec<KeeperMaintenanceTransactionSummary>,
    pub maintenance_calldata_bytes: usize,
    #[serde(default)]
    pub has_maintenance_calldata: bool,
    pub should_scan_liquidations: bool,
    #[serde(default)]
    pub has_liquidation_scan: bool,
    pub has_work: bool,
}

/// Caller-reported result from executing one polling pass.
///
/// The SDK does not persist this or decide whether a transaction is profitable.
/// It only maps observed progress into the next snapshot a caller can store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeeperPollingOutcome {
    pub current_block: u64,
    pub latest_event_cursor: Option<RawLogCursor>,
    pub completed_maintenance: bool,
    pub completed_liquidation_scan: bool,
}

impl KeeperPollingPlan {
    #[must_use]
    pub fn has_work(&self) -> bool {
        !self.event_queries.is_empty()
            || !self.maintenance_transactions.is_empty()
            || self.should_scan_liquidations
    }

    #[must_use]
    pub fn summary(&self) -> KeeperPollingPlanSummary {
        let maintenance_transactions = self
            .maintenance_transactions
            .iter()
            .map(KeeperMaintenanceTransactionSummary::from_transaction)
            .collect::<Vec<_>>();
        let maintenance_calldata_bytes = maintenance_transactions
            .iter()
            .map(|tx| tx.calldata_bytes)
            .sum();
        let first_event_from_block = self
            .event_queries
            .first()
            .and_then(|query| query.from_block.clone());
        let last_event_to_block = self
            .event_queries
            .last()
            .and_then(|query| query.to_block.clone());

        KeeperPollingPlanSummary {
            event_query_count: self.event_queries.len(),
            has_event_queries: !self.event_queries.is_empty(),
            event_query_summary: EventLogRpcQuery::summarize_batch(&self.event_queries),
            has_first_event_from_block: first_event_from_block.is_some(),
            first_event_from_block,
            has_last_event_to_block: last_event_to_block.is_some(),
            last_event_to_block,
            maintenance_transaction_count: maintenance_transactions.len(),
            has_maintenance_transactions: !maintenance_transactions.is_empty(),
            maintenance_transactions,
            maintenance_calldata_bytes,
            has_maintenance_calldata: maintenance_calldata_bytes > 0,
            should_scan_liquidations: self.should_scan_liquidations,
            has_liquidation_scan: self.should_scan_liquidations,
            has_work: self.has_work(),
        }
    }

    #[must_use]
    pub fn empty_at(current_block: u64) -> KeeperPollingOutcome {
        KeeperPollingOutcome::at_block(current_block)
    }
}

impl KeeperMaintenanceTransactionSummary {
    #[must_use]
    pub fn from_transaction(tx: &UnsignedTx) -> Self {
        let selector = tx.selector_hex();
        let calldata_bytes = tx.data_len();
        Self {
            to: tx.to,
            has_selector: selector.is_some(),
            selector,
            calldata_bytes,
            has_calldata: calldata_bytes > 0,
        }
    }
}

impl KeeperPollingOutcome {
    #[must_use]
    pub const fn at_block(current_block: u64) -> Self {
        Self {
            current_block,
            latest_event_cursor: None,
            completed_maintenance: false,
            completed_liquidation_scan: false,
        }
    }

    #[must_use]
    pub const fn with_latest_event_cursor(mut self, latest_event_cursor: RawLogCursor) -> Self {
        self.latest_event_cursor = Some(latest_event_cursor);
        self
    }

    #[must_use]
    pub const fn with_completed_maintenance(mut self) -> Self {
        self.completed_maintenance = true;
        self
    }

    #[must_use]
    pub const fn with_completed_liquidation_scan(mut self) -> Self {
        self.completed_liquidation_scan = true;
        self
    }

    #[must_use]
    pub fn next_snapshot(self, previous: KeeperPollingSnapshot) -> KeeperPollingSnapshot {
        let event_cursor = match (previous.event_cursor, self.latest_event_cursor) {
            (Some(previous), Some(latest)) => Some(previous.max(latest)),
            (None, Some(latest)) => Some(latest),
            (Some(previous), None) => Some(previous),
            (None, None) => None,
        };

        KeeperPollingSnapshot {
            current_block: self.current_block,
            event_cursor,
            event_from_block: if self.latest_event_cursor.is_some() {
                None
            } else {
                previous.event_from_block
            },
            last_tick_block: if self.completed_maintenance {
                Some(self.current_block)
            } else {
                previous.last_tick_block
            },
            last_liquidation_scan_block: if self.completed_liquidation_scan {
                Some(self.current_block)
            } else {
                previous.last_liquidation_scan_block
            },
        }
    }
}

impl KeeperRuntimePlan {
    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest) -> Self {
        Self {
            chain_id: manifest.chain_id,
            network: manifest.network.clone(),
            event_filters: EventFilterSet::from_manifest(manifest),
            orderbook_maintenance: OrderBookMaintenancePlan::from_manifest(manifest),
            has_settlement_engine: manifest.has_settlement_engine(),
            has_liquidation_keeper: manifest.has_liquidation_keeper(),
        }
    }

    #[must_use]
    pub fn capabilities(&self) -> Vec<KeeperCapability> {
        let mut capabilities = Vec::new();

        if !self.event_filters.is_empty() {
            capabilities.push(KeeperCapability::EventIndexing);
        }
        if self.orderbook_maintenance.is_some() {
            capabilities.push(KeeperCapability::OrderBookMaintenance);
        }
        if self.has_settlement_engine {
            capabilities.push(KeeperCapability::SettlementReads);
        }
        if self.has_liquidation_keeper {
            capabilities.push(KeeperCapability::LiquidationReads);
        }
        if self.is_full_perp_stack_available() {
            capabilities.push(KeeperCapability::FullPerpStack);
        }

        capabilities
    }

    #[must_use]
    pub const fn can_tick_orderbook(&self) -> bool {
        self.orderbook_maintenance.is_some()
    }

    #[must_use]
    pub const fn can_read_liquidations(&self) -> bool {
        self.has_liquidation_keeper
    }

    #[must_use]
    pub const fn is_full_perp_stack_available(&self) -> bool {
        self.orderbook_maintenance.is_some()
            && self.has_settlement_engine
            && self.has_liquidation_keeper
    }

    #[must_use]
    pub fn maintenance_transactions(&self) -> Vec<UnsignedTx> {
        self.orderbook_maintenance
            .as_ref()
            .map(|plan| plan.transactions().to_vec())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn liquidation_candidate(
        &self,
        manifest: &DeploymentManifest,
        account_id: u128,
        market_id: u128,
    ) -> KeeperLiquidationCandidatePlan {
        KeeperLiquidationCandidatePlan {
            settlement: SettlementReadPlan::from_manifest(manifest, account_id, market_id),
            liquidation: LiquidationReadPlan::from_manifest(manifest, account_id, market_id),
        }
    }

    pub fn polling_plan(
        &self,
        snapshot: KeeperPollingSnapshot,
        policy: KeeperPollingPolicy,
    ) -> Result<KeeperPollingPlan, EventQueryError> {
        let event_queries = if self.event_filters.is_empty() {
            Vec::new()
        } else {
            let from_block = snapshot.event_cursor.map_or_else(
                || snapshot.event_from_block.unwrap_or(snapshot.current_block),
                |cursor| cursor.resume_from_block(),
            );

            if from_block > snapshot.current_block {
                Vec::new()
            } else {
                self.event_filters.chunked_rpc_queries(
                    from_block,
                    snapshot.current_block,
                    policy.max_event_window_blocks(),
                )?
            }
        };

        let maintenance_transactions = if self.can_tick_orderbook()
            && is_interval_due(
                snapshot.last_tick_block,
                snapshot.current_block,
                policy.tick_interval_blocks(),
            ) {
            self.maintenance_transactions()
        } else {
            Vec::new()
        };

        let should_scan_liquidations = self.can_read_liquidations()
            && is_interval_due(
                snapshot.last_liquidation_scan_block,
                snapshot.current_block,
                policy.liquidation_scan_interval_blocks(),
            );

        Ok(KeeperPollingPlan {
            event_queries,
            maintenance_transactions,
            should_scan_liquidations,
        })
    }
}

impl KeeperLiquidationCandidatePlan {
    #[must_use]
    pub const fn can_submit_liquidation(&self) -> bool {
        self.liquidation.is_some()
    }

    #[must_use]
    pub fn read_calls(&self) -> Vec<UnsignedCall> {
        let mut calls = Vec::new();

        if let Some(settlement) = self.settlement {
            calls.extend(settlement.calls());
        }
        if let Some(liquidation) = self.liquidation {
            calls.extend(liquidation.calls());
        }

        calls
    }

    #[must_use]
    pub fn liquidation_tx(&self) -> Option<UnsignedTx> {
        self.liquidation
            .as_ref()
            .map(LiquidationReadPlan::liquidate_tx)
    }
}

impl Default for KeeperPollingPolicy {
    fn default() -> Self {
        Self::new(2_000, 1, 1)
    }
}

impl KeeperPollingPolicy {
    #[must_use]
    pub const fn new(
        max_event_window_blocks: u64,
        tick_interval_blocks: u64,
        liquidation_scan_interval_blocks: u64,
    ) -> Self {
        Self {
            max_event_window_blocks,
            tick_interval_blocks,
            liquidation_scan_interval_blocks,
        }
    }

    #[must_use]
    pub const fn max_event_window_blocks(self) -> u64 {
        if self.max_event_window_blocks == 0 {
            1
        } else {
            self.max_event_window_blocks
        }
    }

    #[must_use]
    pub const fn tick_interval_blocks(self) -> u64 {
        if self.tick_interval_blocks == 0 {
            1
        } else {
            self.tick_interval_blocks
        }
    }

    #[must_use]
    pub const fn liquidation_scan_interval_blocks(self) -> u64 {
        if self.liquidation_scan_interval_blocks == 0 {
            1
        } else {
            self.liquidation_scan_interval_blocks
        }
    }
}

impl KeeperPollingSnapshot {
    #[must_use]
    pub const fn at_block(current_block: u64) -> Self {
        Self {
            current_block,
            event_cursor: None,
            event_from_block: None,
            last_tick_block: None,
            last_liquidation_scan_block: None,
        }
    }

    #[must_use]
    pub const fn with_event_cursor(mut self, event_cursor: RawLogCursor) -> Self {
        self.event_cursor = Some(event_cursor);
        self
    }

    #[must_use]
    pub const fn with_event_from_block(mut self, event_from_block: u64) -> Self {
        self.event_from_block = Some(event_from_block);
        self
    }

    #[must_use]
    pub const fn with_last_tick_block(mut self, last_tick_block: u64) -> Self {
        self.last_tick_block = Some(last_tick_block);
        self
    }

    #[must_use]
    pub const fn with_last_liquidation_scan_block(
        mut self,
        last_liquidation_scan_block: u64,
    ) -> Self {
        self.last_liquidation_scan_block = Some(last_liquidation_scan_block);
        self
    }
}

fn is_interval_due(last_block: Option<u64>, current_block: u64, interval_blocks: u64) -> bool {
    last_block
        .is_none_or(|last_block| current_block.saturating_sub(last_block) >= interval_blocks.max(1))
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use super::*;
    use crate::{ContractAddresses, NetworkConstants};

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

    #[test]
    fn current_manifest_keeper_plan_only_exposes_available_capabilities() {
        let manifest = current_manifest();
        let plan = KeeperRuntimePlan::from_manifest(&manifest);

        assert_eq!(plan.chain_id, manifest.chain_id);
        assert_eq!(plan.network, manifest.network);
        assert!(!plan.event_filters.is_empty());
        assert_eq!(plan.orderbook_maintenance, None);
        assert!(!plan.can_tick_orderbook());
        assert!(!plan.can_read_liquidations());
        assert!(!plan.is_full_perp_stack_available());
        assert_eq!(plan.capabilities(), vec![KeeperCapability::EventIndexing]);
        assert!(plan.maintenance_transactions().is_empty());

        let candidate = plan.liquidation_candidate(&manifest, 7, 1);
        assert_eq!(candidate.settlement, None);
        assert_eq!(candidate.liquidation, None);
        assert!(candidate.read_calls().is_empty());
        assert_eq!(candidate.liquidation_tx(), None);
    }

    #[test]
    fn current_manifest_polling_plan_builds_chunked_event_queries_only() {
        let manifest = current_manifest();
        let plan = KeeperRuntimePlan::from_manifest(&manifest);
        let snapshot = KeeperPollingSnapshot::at_block(105).with_event_from_block(100);
        let policy = KeeperPollingPolicy::new(3, 1, 1);

        let polling = plan
            .polling_plan(snapshot, policy)
            .expect("polling plan builds");

        assert_eq!(polling.event_queries.len(), 2);
        assert_eq!(polling.event_queries[0].from_block.as_deref(), Some("0x64"));
        assert_eq!(polling.event_queries[0].to_block.as_deref(), Some("0x66"));
        assert_eq!(polling.event_queries[1].from_block.as_deref(), Some("0x67"));
        assert_eq!(polling.event_queries[1].to_block.as_deref(), Some("0x69"));
        assert!(polling.maintenance_transactions.is_empty());
        assert!(!polling.should_scan_liquidations);

        let summary = polling.summary();
        assert_eq!(summary.event_query_count, 2);
        assert!(summary.has_event_queries);
        assert_eq!(summary.event_query_summary.len, 2);
        assert_eq!(summary.event_query_summary.total_address_filters, 6);
        assert_eq!(summary.event_query_summary.total_topic0_filters, 18);
        assert_eq!(summary.event_query_summary.open_ended_queries, 0);
        assert_eq!(summary.first_event_from_block.as_deref(), Some("0x64"));
        assert_eq!(summary.last_event_to_block.as_deref(), Some("0x69"));
        assert_eq!(summary.maintenance_transaction_count, 0);
        assert!(!summary.has_maintenance_transactions);
        assert!(summary.maintenance_transactions.is_empty());
        assert_eq!(summary.maintenance_calldata_bytes, 0);
        assert!(!summary.has_liquidation_scan);
        assert!(summary.has_work);
    }

    #[test]
    fn full_manifest_keeper_plan_composes_maintenance_and_candidate_work() {
        let manifest = full_manifest();
        let plan = KeeperRuntimePlan::from_manifest(&manifest);

        assert!(plan.can_tick_orderbook());
        assert!(plan.can_read_liquidations());
        assert!(plan.is_full_perp_stack_available());
        assert_eq!(
            plan.capabilities(),
            vec![
                KeeperCapability::EventIndexing,
                KeeperCapability::OrderBookMaintenance,
                KeeperCapability::SettlementReads,
                KeeperCapability::LiquidationReads,
                KeeperCapability::FullPerpStack,
            ]
        );
        assert_eq!(plan.maintenance_transactions().len(), 1);

        let candidate = plan.liquidation_candidate(&manifest, 7, 1);
        assert!(candidate.can_submit_liquidation());
        assert_eq!(candidate.read_calls().len(), 4);
        assert_eq!(
            candidate.liquidation_tx().expect("liquidation tx").to,
            manifest.contracts.liquidation_keeper.expect("keeper")
        );
    }

    #[test]
    fn full_manifest_polling_plan_marks_due_work_from_snapshot() {
        let manifest = full_manifest();
        let plan = KeeperRuntimePlan::from_manifest(&manifest);
        let snapshot = KeeperPollingSnapshot::at_block(130)
            .with_event_cursor(RawLogCursor::new(120, 4))
            .with_last_tick_block(128)
            .with_last_liquidation_scan_block(120);
        let policy = KeeperPollingPolicy::new(5, 2, 20);

        let polling = plan
            .polling_plan(snapshot, policy)
            .expect("polling plan builds");

        assert_eq!(polling.event_queries.len(), 3);
        assert_eq!(polling.event_queries[0].from_block.as_deref(), Some("0x78"));
        assert_eq!(polling.event_queries[2].to_block.as_deref(), Some("0x82"));
        assert_eq!(polling.maintenance_transactions.len(), 1);
        assert!(!polling.should_scan_liquidations);
        assert!(polling.has_work());

        let summary = polling.summary();
        assert_eq!(summary.event_query_count, 3);
        assert!(summary.has_event_queries);
        assert_eq!(summary.event_query_summary.len, 3);
        assert_eq!(summary.event_query_summary.queries.len(), 3);
        assert_eq!(summary.event_query_summary.open_ended_queries, 0);
        assert_eq!(summary.first_event_from_block.as_deref(), Some("0x78"));
        assert!(summary.has_first_event_from_block);
        assert_eq!(summary.last_event_to_block.as_deref(), Some("0x82"));
        assert!(summary.has_last_event_to_block);
        assert_eq!(summary.maintenance_transaction_count, 1);
        assert!(summary.has_maintenance_transactions);
        assert_eq!(summary.maintenance_transactions.len(), 1);
        assert_eq!(
            summary.maintenance_transactions[0].to,
            manifest.contracts.order_book.expect("orderbook")
        );
        assert_eq!(
            summary.maintenance_transactions[0].selector,
            polling.maintenance_transactions[0].selector_hex()
        );
        assert!(summary.maintenance_transactions[0].has_selector);
        assert_eq!(
            summary.maintenance_calldata_bytes,
            polling.maintenance_transactions[0].data_len()
        );
        assert!(summary.maintenance_transactions[0].has_calldata);
        assert!(summary.has_maintenance_calldata);
        assert!(!summary.has_liquidation_scan);
        assert!(summary.has_work);
        let json = serde_json::to_string(&summary).expect("keeper summary serializes");
        assert!(json.contains("\"event_query_summary\""));
        assert!(json.contains("\"has_event_queries\":true"));
        assert!(json.contains("\"has_first_event_from_block\":true"));
        assert!(json.contains("\"has_maintenance_transactions\":true"));
        assert!(json.contains("\"has_maintenance_calldata\":true"));
        let restored: KeeperPollingPlanSummary =
            serde_json::from_str(&json).expect("keeper summary deserializes");
        assert_eq!(restored, summary);
        let mut legacy_json = serde_json::to_value(summary).expect("keeper summary value");
        let legacy_object = legacy_json.as_object_mut().expect("keeper summary object");
        legacy_object.remove("has_event_queries");
        legacy_object.remove("event_query_summary");
        legacy_object.remove("has_first_event_from_block");
        legacy_object.remove("has_last_event_to_block");
        legacy_object.remove("has_maintenance_transactions");
        legacy_object.remove("has_maintenance_calldata");
        legacy_object.remove("has_liquidation_scan");
        let legacy_maintenance_transactions = legacy_object
            .get_mut("maintenance_transactions")
            .and_then(serde_json::Value::as_array_mut)
            .expect("legacy maintenance transactions");
        for transaction in legacy_maintenance_transactions {
            let transaction_object = transaction
                .as_object_mut()
                .expect("legacy maintenance transaction");
            transaction_object.remove("has_selector");
            transaction_object.remove("has_calldata");
        }
        let legacy: KeeperPollingPlanSummary =
            serde_json::from_value(legacy_json).expect("legacy keeper summary deserializes");
        assert!(!legacy.has_event_queries);
        assert_eq!(
            legacy.event_query_summary,
            EventLogRpcQueryBatchSummary::default()
        );
        assert!(!legacy.has_first_event_from_block);
        assert!(!legacy.has_last_event_to_block);
        assert!(!legacy.has_maintenance_transactions);
        assert!(!legacy.has_maintenance_calldata);
        assert!(!legacy.has_liquidation_scan);
        assert!(legacy
            .maintenance_transactions
            .iter()
            .all(|transaction| !transaction.has_selector && !transaction.has_calldata));
    }

    #[test]
    fn polling_outcome_advances_completed_keeper_checkpoint() {
        let previous = KeeperPollingSnapshot::at_block(120)
            .with_event_from_block(100)
            .with_last_tick_block(118)
            .with_last_liquidation_scan_block(110);
        let outcome = KeeperPollingOutcome::at_block(130)
            .with_latest_event_cursor(RawLogCursor::new(130, 7))
            .with_completed_maintenance()
            .with_completed_liquidation_scan();

        let next = outcome.next_snapshot(previous);

        assert_eq!(next.current_block, 130);
        assert_eq!(next.event_cursor, Some(RawLogCursor::new(130, 7)));
        assert_eq!(next.event_from_block, None);
        assert_eq!(next.last_tick_block, Some(130));
        assert_eq!(next.last_liquidation_scan_block, Some(130));
    }

    #[test]
    fn polling_outcome_preserves_unfinished_checkpoint_progress() {
        let previous = KeeperPollingSnapshot::at_block(120)
            .with_event_cursor(RawLogCursor::new(120, 4))
            .with_last_tick_block(118)
            .with_last_liquidation_scan_block(110);
        let outcome =
            KeeperPollingOutcome::at_block(130).with_latest_event_cursor(RawLogCursor::new(119, 9));

        let next = outcome.next_snapshot(previous);

        assert_eq!(next.current_block, 130);
        assert_eq!(next.event_cursor, Some(RawLogCursor::new(120, 4)));
        assert_eq!(next.last_tick_block, Some(118));
        assert_eq!(next.last_liquidation_scan_block, Some(110));

        let empty = KeeperPollingPlan {
            event_queries: Vec::new(),
            maintenance_transactions: Vec::new(),
            should_scan_liquidations: false,
        };
        assert!(!empty.has_work());
        assert_eq!(
            KeeperPollingPlan::empty_at(131),
            KeeperPollingOutcome::at_block(131)
        );
    }

    #[test]
    fn polling_policy_normalizes_zero_intervals() {
        let manifest = full_manifest();
        let plan = KeeperRuntimePlan::from_manifest(&manifest);
        let snapshot = KeeperPollingSnapshot::at_block(10)
            .with_event_from_block(10)
            .with_last_tick_block(9)
            .with_last_liquidation_scan_block(9);
        let policy = KeeperPollingPolicy::new(0, 0, 0);

        let polling = plan
            .polling_plan(snapshot, policy)
            .expect("polling plan builds");

        assert_eq!(policy.max_event_window_blocks(), 1);
        assert_eq!(policy.tick_interval_blocks(), 1);
        assert_eq!(policy.liquidation_scan_interval_blocks(), 1);
        assert_eq!(polling.event_queries.len(), 1);
        assert_eq!(polling.maintenance_transactions.len(), 1);
        assert!(polling.should_scan_liquidations);
    }
}
