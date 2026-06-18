//! Deterministic event projections for indexers and reference runtimes.
//!
//! This module does not choose a database, cache, GraphQL shape, or retention
//! policy. It folds decoded Tangent events into compact state a caller can
//! persist or compare in tests.

use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{Address, B256};
use serde::{Deserialize, Serialize};

use crate::{
    DecodedTangentLogRecord, DecodedTangentLogRecords, LiquidatedEvent, MatchedEvent, RawLogCursor,
    SettledEvent, TangentEvent,
};

/// In-memory state derived from decoded Tangent events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentEventProjection {
    pub accounts: BTreeMap<u128, AccountEventProjection>,
    pub markets: BTreeMap<u128, MarketEventProjection>,
    pub orders: BTreeMap<B256, OrderEventProjection>,
    pub matched_fills: Vec<MatchedEvent>,
    pub settled_fills: Vec<SettledEvent>,
    pub liquidations: Vec<LiquidatedEvent>,
    pub applied_records: usize,
    pub unknown_logs: usize,
    pub last_cursor: Option<RawLogCursor>,
}

/// Account-oriented state derived from account, collateral, margin, and PnL events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountEventProjection {
    pub account_id: u128,
    pub owner: Option<Address>,
    pub registered_at: Option<u64>,
    pub deposited: u128,
    pub withdrawn: u128,
    pub margin_locked: u128,
    pub margin_released: u128,
    pub pnl: i128,
}

/// Market-oriented state derived from registry events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketEventProjection {
    pub market_id: u128,
    pub symbol: Option<String>,
    pub price_feed: Option<Address>,
    pub paused: Option<bool>,
    pub params_updates: u64,
}

/// Account/market pair observed in projected order or liquidation history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AccountMarketProjectionKey {
    pub account_id: u128,
    pub market_id: u128,
}

/// Order state derived from orderbook, match, and settlement events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderEventProjection {
    pub order_hash: B256,
    pub account_id: Option<u128>,
    pub market_id: Option<u128>,
    pub is_buy: Option<bool>,
    pub limit_price: Option<u128>,
    pub submitted_size: Option<u128>,
    pub matched_size: u128,
    pub settled_size: u128,
    pub status: OrderEventStatus,
    pub cancel_reason: Option<String>,
}

/// Compact counts for consumers that need a cheap projection overview.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentEventProjectionSummary {
    pub accounts: usize,
    #[serde(default)]
    pub has_accounts: bool,
    pub markets: usize,
    #[serde(default)]
    pub has_markets: bool,
    pub orders: usize,
    #[serde(default)]
    pub has_orders: bool,
    pub active_orders: usize,
    #[serde(default)]
    pub has_active_orders: bool,
    pub cancelled_orders: usize,
    #[serde(default)]
    pub has_cancelled_orders: bool,
    #[serde(default)]
    pub account_market_candidates: usize,
    #[serde(default)]
    pub has_account_market_candidates: bool,
    #[serde(default)]
    pub active_account_market_candidates: usize,
    #[serde(default)]
    pub has_active_account_market_candidates: bool,
    pub matched_fills: usize,
    #[serde(default)]
    pub has_matched_fills: bool,
    pub settled_fills: usize,
    #[serde(default)]
    pub has_settled_fills: bool,
    pub liquidations: usize,
    #[serde(default)]
    pub has_liquidations: bool,
    pub applied_records: usize,
    #[serde(default)]
    pub has_applied_records: bool,
    pub unknown_logs: usize,
    #[serde(default)]
    pub has_unknown_logs: bool,
    pub last_cursor: Option<RawLogCursor>,
    #[serde(default)]
    pub has_last_cursor: bool,
}

/// Latest lifecycle state inferred from event order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderEventStatus {
    #[default]
    Unknown,
    Submitted,
    Matched,
    Settled,
    Cancelled,
}

/// Errors that can occur while folding event-derived state.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TangentEventProjectionError {
    #[error("event projection arithmetic overflow in {field}")]
    ArithmeticOverflow { field: &'static str },
}

impl TangentEventProjection {
    pub fn apply_event(&mut self, event: &TangentEvent) -> Result<(), TangentEventProjectionError> {
        match event {
            TangentEvent::AccountRegistered(event) => {
                let account = self.account_mut(event.account_id);
                account.owner = Some(event.owner);
                account.registered_at = Some(event.registered_at);
            }
            TangentEvent::Deposited(event) => {
                checked_add_u128(
                    &mut self.account_mut(event.account_id).deposited,
                    event.amount,
                    "account.deposited",
                )?;
            }
            TangentEvent::Withdrawn(event) => {
                checked_add_u128(
                    &mut self.account_mut(event.account_id).withdrawn,
                    event.amount,
                    "account.withdrawn",
                )?;
            }
            TangentEvent::MarginLocked(event) => {
                checked_add_u128(
                    &mut self.account_mut(event.account_id).margin_locked,
                    event.amount,
                    "account.margin_locked",
                )?;
            }
            TangentEvent::MarginReleased(event) => {
                checked_add_u128(
                    &mut self.account_mut(event.account_id).margin_released,
                    event.amount,
                    "account.margin_released",
                )?;
            }
            TangentEvent::PnlApplied(event) => {
                checked_add_i128(
                    &mut self.account_mut(event.account_id).pnl,
                    event.pnl,
                    "account.pnl",
                )?;
            }
            TangentEvent::OrderSubmitted(event) => {
                let order = self.order_mut(event.order_hash);
                order.account_id = Some(event.account_id);
                order.market_id = Some(event.market_id);
                order.is_buy = Some(event.is_buy);
                order.limit_price = Some(event.limit_price);
                order.submitted_size = Some(event.size);
                order.status = OrderEventStatus::Submitted;
                order.cancel_reason = None;
            }
            TangentEvent::OrderCancelled(event) => {
                let order = self.order_mut(event.order_hash);
                order.account_id = Some(event.account_id);
                order.status = OrderEventStatus::Cancelled;
                order.cancel_reason = Some(event.reason.clone());
            }
            TangentEvent::Matched(event) => {
                self.matched_fills.push(event.clone());
                self.apply_fill(event.buy_order_hash, event.market_id, event.size, true)?;
                self.apply_fill(event.sell_order_hash, event.market_id, event.size, true)?;
            }
            TangentEvent::Settled(event) => {
                self.settled_fills.push(event.clone());
                self.apply_fill(event.buy_order_hash, event.market_id, event.size, false)?;
                self.apply_fill(event.sell_order_hash, event.market_id, event.size, false)?;
            }
            TangentEvent::MarketRegistered(event) => {
                let market = self.market_mut(event.market_id);
                market.symbol = Some(event.symbol.clone());
                market.price_feed = Some(event.price_feed);
            }
            TangentEvent::MarketParamsUpdated(event) => {
                let market = self.market_mut(event.market_id);
                market.params_updates = market.params_updates.checked_add(1).ok_or(
                    TangentEventProjectionError::ArithmeticOverflow {
                        field: "market.params_updates",
                    },
                )?;
            }
            TangentEvent::MarketPaused(event) => {
                self.market_mut(event.market_id).paused = Some(event.paused);
            }
            TangentEvent::Liquidated(event) => {
                self.liquidations.push(event.clone());
            }
        }

        Ok(())
    }

    pub fn apply_record(
        &mut self,
        record: &DecodedTangentLogRecord,
    ) -> Result<(), TangentEventProjectionError> {
        self.apply_event(&record.event)?;
        self.applied_records = self.applied_records.checked_add(1).ok_or(
            TangentEventProjectionError::ArithmeticOverflow {
                field: "projection.applied_records",
            },
        )?;

        if let Some(cursor) = record.cursor() {
            self.last_cursor = Some(self.last_cursor.map_or(cursor, |last| last.max(cursor)));
        }

        Ok(())
    }

    pub fn apply_records(
        &mut self,
        records: &DecodedTangentLogRecords,
    ) -> Result<(), TangentEventProjectionError> {
        for record in &records.records {
            self.apply_record(record)?;
        }
        self.unknown_logs = self.unknown_logs.checked_add(records.unknown_logs).ok_or(
            TangentEventProjectionError::ArithmeticOverflow {
                field: "projection.unknown_logs",
            },
        )?;
        Ok(())
    }

    pub fn apply_record_after_cursor(
        &mut self,
        record: &DecodedTangentLogRecord,
        cursor: RawLogCursor,
    ) -> Result<bool, TangentEventProjectionError> {
        if record
            .cursor()
            .is_some_and(|record_cursor| record_cursor <= cursor)
        {
            return Ok(false);
        }

        self.apply_record(record)?;
        Ok(true)
    }

    pub fn apply_records_after_cursor(
        &mut self,
        records: &DecodedTangentLogRecords,
        cursor: RawLogCursor,
    ) -> Result<usize, TangentEventProjectionError> {
        let mut applied = 0usize;
        for record in &records.records {
            if self.apply_record_after_cursor(record, cursor)? {
                applied = applied.checked_add(1).ok_or(
                    TangentEventProjectionError::ArithmeticOverflow {
                        field: "projection.applied_records",
                    },
                )?;
            }
        }
        self.unknown_logs = self.unknown_logs.checked_add(records.unknown_logs).ok_or(
            TangentEventProjectionError::ArithmeticOverflow {
                field: "projection.unknown_logs",
            },
        )?;
        Ok(applied)
    }

    pub fn apply_records_since_last_cursor(
        &mut self,
        records: &DecodedTangentLogRecords,
    ) -> Result<usize, TangentEventProjectionError> {
        match self.last_cursor {
            Some(cursor) => self.apply_records_after_cursor(records, cursor),
            None => {
                let applied = records.known_logs();
                self.apply_records(records)?;
                Ok(applied)
            }
        }
    }

    #[must_use]
    pub fn summary(&self) -> TangentEventProjectionSummary {
        let account_market_candidates = self.account_market_keys().len();
        let active_account_market_candidates = self.active_account_market_keys().len();
        let accounts = self.accounts.len();
        let markets = self.markets.len();
        let orders = self.orders.len();
        let active_orders = self
            .orders
            .values()
            .filter(|order| order.status.is_active())
            .count();
        let cancelled_orders = self
            .orders
            .values()
            .filter(|order| order.status == OrderEventStatus::Cancelled)
            .count();
        let matched_fills = self.matched_fills.len();
        let settled_fills = self.settled_fills.len();
        let liquidations = self.liquidations.len();

        TangentEventProjectionSummary {
            accounts,
            has_accounts: accounts > 0,
            markets,
            has_markets: markets > 0,
            orders,
            has_orders: orders > 0,
            active_orders,
            has_active_orders: active_orders > 0,
            cancelled_orders,
            has_cancelled_orders: cancelled_orders > 0,
            account_market_candidates,
            has_account_market_candidates: account_market_candidates > 0,
            active_account_market_candidates,
            has_active_account_market_candidates: active_account_market_candidates > 0,
            matched_fills,
            has_matched_fills: matched_fills > 0,
            settled_fills,
            has_settled_fills: settled_fills > 0,
            liquidations,
            has_liquidations: liquidations > 0,
            applied_records: self.applied_records,
            has_applied_records: self.applied_records > 0,
            unknown_logs: self.unknown_logs,
            has_unknown_logs: self.unknown_logs > 0,
            last_cursor: self.last_cursor,
            has_last_cursor: self.last_cursor.is_some(),
        }
    }

    #[must_use]
    pub fn account_market_keys(&self) -> Vec<AccountMarketProjectionKey> {
        let mut keys = BTreeSet::new();

        for order in self.orders.values() {
            if let Some(key) = order.account_market_key() {
                keys.insert(key);
            }
        }
        for liquidation in &self.liquidations {
            keys.insert(AccountMarketProjectionKey {
                account_id: liquidation.account_id,
                market_id: liquidation.market_id,
            });
        }

        keys.into_iter().collect()
    }

    #[must_use]
    pub fn active_account_market_keys(&self) -> Vec<AccountMarketProjectionKey> {
        self.orders
            .values()
            .filter(|order| order.status.is_active())
            .filter_map(OrderEventProjection::account_market_key)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    #[must_use]
    pub fn orders_for_account(&self, account_id: u128) -> Vec<&OrderEventProjection> {
        self.orders
            .values()
            .filter(|order| order.account_id == Some(account_id))
            .collect()
    }

    #[must_use]
    pub fn orders_for_market(&self, market_id: u128) -> Vec<&OrderEventProjection> {
        self.orders
            .values()
            .filter(|order| order.market_id == Some(market_id))
            .collect()
    }

    fn account_mut(&mut self, account_id: u128) -> &mut AccountEventProjection {
        self.accounts
            .entry(account_id)
            .or_insert_with(|| AccountEventProjection {
                account_id,
                ..AccountEventProjection::default()
            })
    }

    fn market_mut(&mut self, market_id: u128) -> &mut MarketEventProjection {
        self.markets
            .entry(market_id)
            .or_insert_with(|| MarketEventProjection {
                market_id,
                ..MarketEventProjection::default()
            })
    }

    fn order_mut(&mut self, order_hash: B256) -> &mut OrderEventProjection {
        self.orders
            .entry(order_hash)
            .or_insert_with(|| OrderEventProjection::new(order_hash))
    }

    fn apply_fill(
        &mut self,
        order_hash: B256,
        market_id: u128,
        size: u128,
        matched: bool,
    ) -> Result<(), TangentEventProjectionError> {
        let order = self.order_mut(order_hash);
        order.market_id = order.market_id.or(Some(market_id));

        if matched {
            checked_add_u128(&mut order.matched_size, size, "order.matched_size")?;
            if order.status != OrderEventStatus::Cancelled {
                order.status = OrderEventStatus::Matched;
            }
        } else {
            checked_add_u128(&mut order.settled_size, size, "order.settled_size")?;
            if order.status != OrderEventStatus::Cancelled {
                order.status = OrderEventStatus::Settled;
            }
        }

        Ok(())
    }
}

impl AccountEventProjection {
    #[must_use]
    pub fn net_deposits(&self) -> Option<i128> {
        i128::try_from(self.deposited)
            .ok()?
            .checked_sub(i128::try_from(self.withdrawn).ok()?)
    }

    #[must_use]
    pub fn net_margin_locked(&self) -> Option<i128> {
        i128::try_from(self.margin_locked)
            .ok()?
            .checked_sub(i128::try_from(self.margin_released).ok()?)
    }
}

impl OrderEventProjection {
    #[must_use]
    pub const fn new(order_hash: B256) -> Self {
        Self {
            order_hash,
            account_id: None,
            market_id: None,
            is_buy: None,
            limit_price: None,
            submitted_size: None,
            matched_size: 0,
            settled_size: 0,
            status: OrderEventStatus::Unknown,
            cancel_reason: None,
        }
    }

    #[must_use]
    pub const fn account_market_key(&self) -> Option<AccountMarketProjectionKey> {
        match (self.account_id, self.market_id) {
            (Some(account_id), Some(market_id)) => Some(AccountMarketProjectionKey {
                account_id,
                market_id,
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn remaining_size(&self) -> Option<u128> {
        self.submitted_size?.checked_sub(self.matched_size)
    }

    #[must_use]
    pub fn unsettled_matched_size(&self) -> Option<u128> {
        self.matched_size.checked_sub(self.settled_size)
    }
}

impl OrderEventStatus {
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Submitted | Self::Matched)
    }
}

fn checked_add_u128(
    target: &mut u128,
    amount: u128,
    field: &'static str,
) -> Result<(), TangentEventProjectionError> {
    *target = target
        .checked_add(amount)
        .ok_or(TangentEventProjectionError::ArithmeticOverflow { field })?;
    Ok(())
}

fn checked_add_i128(
    target: &mut i128,
    amount: i128,
    field: &'static str,
) -> Result<(), TangentEventProjectionError> {
    *target = target
        .checked_add(amount)
        .ok_or(TangentEventProjectionError::ArithmeticOverflow { field })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccountRegisteredEvent, DecodedTangentLogRecord, DecodedTangentLogRecords, DepositedEvent,
        MarginAmountEvent, MarketPausedEvent, MarketRegisteredEvent, MatchedEvent,
        OrderCancelledEvent, OrderSubmittedEvent, PnlAppliedEvent, RawLogMetadata, SettledEvent,
        WithdrawnEvent,
    };

    fn record(event: TangentEvent, block_number: u64, log_index: u64) -> DecodedTangentLogRecord {
        DecodedTangentLogRecord::new(
            event,
            Some(RawLogMetadata::new(
                Some(block_number),
                Some(B256::repeat_byte(0xaa)),
                Some(log_index),
            )),
        )
    }

    #[test]
    fn projection_folds_account_market_order_and_cursor_state() {
        let owner = Address::repeat_byte(0x11);
        let price_feed = Address::repeat_byte(0x22);
        let buy_hash = B256::repeat_byte(0x33);
        let sell_hash = B256::repeat_byte(0x44);
        let records = DecodedTangentLogRecords {
            records: vec![
                record(
                    TangentEvent::AccountRegistered(AccountRegisteredEvent {
                        account_id: 7,
                        owner,
                        registered_at: 123,
                    }),
                    100,
                    1,
                ),
                record(
                    TangentEvent::Deposited(DepositedEvent {
                        account_id: 7,
                        from: owner,
                        amount: 1_000,
                    }),
                    101,
                    1,
                ),
                record(
                    TangentEvent::Withdrawn(WithdrawnEvent {
                        account_id: 7,
                        to: owner,
                        amount: 250,
                    }),
                    102,
                    1,
                ),
                record(
                    TangentEvent::MarginLocked(MarginAmountEvent {
                        account_id: 7,
                        amount: 300,
                    }),
                    103,
                    1,
                ),
                record(
                    TangentEvent::MarginReleased(MarginAmountEvent {
                        account_id: 7,
                        amount: 50,
                    }),
                    104,
                    1,
                ),
                record(
                    TangentEvent::PnlApplied(PnlAppliedEvent {
                        account_id: 7,
                        pnl: -25,
                    }),
                    105,
                    1,
                ),
                record(
                    TangentEvent::MarketRegistered(MarketRegisteredEvent {
                        market_id: 1,
                        symbol: "BTC".to_owned(),
                        price_feed,
                    }),
                    106,
                    1,
                ),
                record(
                    TangentEvent::MarketPaused(MarketPausedEvent {
                        market_id: 1,
                        paused: true,
                    }),
                    107,
                    1,
                ),
                record(
                    TangentEvent::OrderSubmitted(OrderSubmittedEvent {
                        order_hash: buy_hash,
                        account_id: 7,
                        market_id: 1,
                        is_buy: true,
                        limit_price: 65_000,
                        size: 10,
                    }),
                    108,
                    1,
                ),
                record(
                    TangentEvent::Matched(MatchedEvent {
                        buy_order_hash: buy_hash,
                        sell_order_hash: sell_hash,
                        market_id: 1,
                        size: 4,
                        price: 65_000,
                    }),
                    109,
                    1,
                ),
                record(
                    TangentEvent::Settled(SettledEvent {
                        buy_order_hash: buy_hash,
                        sell_order_hash: sell_hash,
                        market_id: 1,
                        size: 4,
                        price: 65_000,
                    }),
                    110,
                    1,
                ),
                record(
                    TangentEvent::OrderCancelled(OrderCancelledEvent {
                        order_hash: buy_hash,
                        account_id: 7,
                        reason: "owner".to_owned(),
                    }),
                    111,
                    1,
                ),
            ],
            unknown_logs: 2,
        };
        let mut projection = TangentEventProjection::default();

        projection
            .apply_records(&records)
            .expect("projection folds");

        let account = projection.accounts.get(&7).expect("account projection");
        assert_eq!(account.owner, Some(owner));
        assert_eq!(account.net_deposits(), Some(750));
        assert_eq!(account.net_margin_locked(), Some(250));
        assert_eq!(account.pnl, -25);
        let market = projection.markets.get(&1).expect("market projection");
        assert_eq!(market.symbol.as_deref(), Some("BTC"));
        assert_eq!(market.price_feed, Some(price_feed));
        assert_eq!(market.paused, Some(true));
        let buy_order = projection.orders.get(&buy_hash).expect("buy order");
        assert_eq!(buy_order.status, OrderEventStatus::Cancelled);
        assert_eq!(buy_order.matched_size, 4);
        assert_eq!(buy_order.settled_size, 4);
        assert_eq!(buy_order.cancel_reason.as_deref(), Some("owner"));
        assert_eq!(buy_order.remaining_size(), Some(6));
        assert_eq!(buy_order.unsettled_matched_size(), Some(0));
        assert_eq!(
            buy_order.account_market_key(),
            Some(AccountMarketProjectionKey {
                account_id: 7,
                market_id: 1,
            })
        );
        let sell_order = projection.orders.get(&sell_hash).expect("sell order");
        assert_eq!(sell_order.status, OrderEventStatus::Settled);
        assert_eq!(sell_order.market_id, Some(1));
        assert_eq!(sell_order.remaining_size(), None);
        assert_eq!(projection.last_cursor, Some(RawLogCursor::new(111, 1)));
        assert_eq!(
            projection.account_market_keys(),
            vec![AccountMarketProjectionKey {
                account_id: 7,
                market_id: 1,
            }]
        );
        assert!(projection.active_account_market_keys().is_empty());
        assert_eq!(projection.orders_for_account(7).len(), 1);
        assert_eq!(projection.orders_for_market(1).len(), 2);
        let summary = projection.summary();
        assert_eq!(summary.accounts, 1);
        assert!(summary.has_accounts);
        assert_eq!(summary.markets, 1);
        assert!(summary.has_markets);
        assert_eq!(summary.orders, 2);
        assert!(summary.has_orders);
        assert!(!summary.has_active_orders);
        assert_eq!(summary.cancelled_orders, 1);
        assert!(summary.has_cancelled_orders);
        assert_eq!(summary.account_market_candidates, 1);
        assert!(summary.has_account_market_candidates);
        assert_eq!(summary.active_account_market_candidates, 0);
        assert!(!summary.has_active_account_market_candidates);
        assert_eq!(summary.matched_fills, 1);
        assert!(summary.has_matched_fills);
        assert_eq!(summary.settled_fills, 1);
        assert!(summary.has_settled_fills);
        assert_eq!(summary.liquidations, 0);
        assert!(!summary.has_liquidations);
        assert_eq!(summary.applied_records, 12);
        assert!(summary.has_applied_records);
        assert_eq!(summary.unknown_logs, 2);
        assert!(summary.has_unknown_logs);
        assert_eq!(summary.last_cursor, Some(RawLogCursor::new(111, 1)));
        assert!(summary.has_last_cursor);
        let summary_json = serde_json::to_string(&summary).expect("summary serializes");
        assert!(summary_json.contains("\"account_market_candidates\":1"));
        assert!(summary_json.contains("\"has_last_cursor\":true"));
        let restored_summary: TangentEventProjectionSummary =
            serde_json::from_str(&summary_json).expect("summary deserializes");
        assert_eq!(restored_summary, summary);
        let mut legacy_json = serde_json::to_value(summary).expect("summary value serializes");
        let legacy_object = legacy_json.as_object_mut().expect("summary object");
        legacy_object.remove("has_accounts");
        legacy_object.remove("has_markets");
        legacy_object.remove("has_orders");
        legacy_object.remove("has_active_orders");
        legacy_object.remove("has_cancelled_orders");
        legacy_object.remove("account_market_candidates");
        legacy_object.remove("has_account_market_candidates");
        legacy_object.remove("active_account_market_candidates");
        legacy_object.remove("has_active_account_market_candidates");
        legacy_object.remove("has_matched_fills");
        legacy_object.remove("has_settled_fills");
        legacy_object.remove("has_liquidations");
        legacy_object.remove("has_applied_records");
        legacy_object.remove("has_unknown_logs");
        legacy_object.remove("has_last_cursor");
        let legacy_summary: TangentEventProjectionSummary =
            serde_json::from_value(legacy_json).expect("legacy summary deserializes");
        assert!(!legacy_summary.has_accounts);
        assert!(!legacy_summary.has_markets);
        assert!(!legacy_summary.has_orders);
        assert!(!legacy_summary.has_active_orders);
        assert!(!legacy_summary.has_cancelled_orders);
        assert_eq!(legacy_summary.account_market_candidates, 0);
        assert!(!legacy_summary.has_account_market_candidates);
        assert_eq!(legacy_summary.active_account_market_candidates, 0);
        assert!(!legacy_summary.has_active_account_market_candidates);
        assert!(!legacy_summary.has_matched_fills);
        assert!(!legacy_summary.has_settled_fills);
        assert!(!legacy_summary.has_liquidations);
        assert!(!legacy_summary.has_applied_records);
        assert!(!legacy_summary.has_unknown_logs);
        assert!(!legacy_summary.has_last_cursor);
    }

    #[test]
    fn projection_reports_checked_arithmetic_overflow() {
        let records = DecodedTangentLogRecords {
            records: vec![
                DecodedTangentLogRecord::new(
                    TangentEvent::Deposited(DepositedEvent {
                        account_id: 7,
                        from: Address::ZERO,
                        amount: u128::MAX,
                    }),
                    None,
                ),
                DecodedTangentLogRecord::new(
                    TangentEvent::Deposited(DepositedEvent {
                        account_id: 7,
                        from: Address::ZERO,
                        amount: 1,
                    }),
                    None,
                ),
            ],
            unknown_logs: 0,
        };
        let mut projection = TangentEventProjection::default();

        let error = projection
            .apply_records(&records)
            .expect_err("deposit overflow is reported");

        assert_eq!(
            error,
            TangentEventProjectionError::ArithmeticOverflow {
                field: "account.deposited"
            }
        );
    }

    #[test]
    fn projection_can_apply_records_since_last_cursor() {
        let owner = Address::repeat_byte(0x11);
        let mut projection = TangentEventProjection::default();
        projection
            .apply_record(&record(
                TangentEvent::Deposited(DepositedEvent {
                    account_id: 7,
                    from: owner,
                    amount: 10,
                }),
                100,
                1,
            ))
            .expect("seed projection");
        let records = DecodedTangentLogRecords {
            records: vec![
                record(
                    TangentEvent::Deposited(DepositedEvent {
                        account_id: 7,
                        from: owner,
                        amount: 10,
                    }),
                    100,
                    1,
                ),
                record(
                    TangentEvent::Deposited(DepositedEvent {
                        account_id: 7,
                        from: owner,
                        amount: 5,
                    }),
                    100,
                    2,
                ),
                DecodedTangentLogRecord::new(
                    TangentEvent::Deposited(DepositedEvent {
                        account_id: 7,
                        from: owner,
                        amount: 7,
                    }),
                    None,
                ),
            ],
            unknown_logs: 3,
        };

        let applied = projection
            .apply_records_since_last_cursor(&records)
            .expect("cursor-aware apply succeeds");

        assert_eq!(applied, 2);
        assert_eq!(projection.accounts[&7].deposited, 22);
        assert_eq!(projection.applied_records, 3);
        assert_eq!(projection.unknown_logs, 3);
        assert_eq!(projection.last_cursor, Some(RawLogCursor::new(100, 2)));
    }
}
