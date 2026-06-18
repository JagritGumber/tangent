//! Typed event-log decoding for receipt, indexer, and keeper consumers.
//!
//! The SDK keeps transport out of scope, but downstream callers still need a
//! stable way to decode logs returned by RPC clients. `RawLog` is the minimal
//! boundary shape: map a provider log into address/topics/data, then call the
//! event-specific decoder.

use alloy_primitives::{keccak256, Address, B256};
use serde::{Deserialize, Serialize};

use crate::{AbiDecodeError, DeploymentManifest};

/// Transport-neutral EVM log payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLog {
    pub address: Address,
    pub topics: Vec<B256>,
    #[serde(with = "raw_log_data")]
    pub data: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RawLogMetadata>,
}

/// Optional provider/indexer source position for a raw EVM log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLogMetadata {
    pub block_number: Option<u64>,
    pub transaction_hash: Option<B256>,
    pub log_index: Option<u64>,
}

impl RawLogMetadata {
    #[must_use]
    pub const fn new(
        block_number: Option<u64>,
        transaction_hash: Option<B256>,
        log_index: Option<u64>,
    ) -> Self {
        Self {
            block_number,
            transaction_hash,
            log_index,
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.block_number.is_none() && self.transaction_hash.is_none() && self.log_index.is_none()
    }

    #[must_use]
    pub const fn cursor(&self) -> Option<RawLogCursor> {
        match (self.block_number, self.log_index) {
            (Some(block_number), Some(log_index)) => {
                Some(RawLogCursor::new(block_number, log_index))
            }
            _ => None,
        }
    }
}

/// Deterministic source-order checkpoint for an indexed EVM log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RawLogCursor {
    pub block_number: u64,
    pub log_index: u64,
}

impl RawLogCursor {
    #[must_use]
    pub const fn new(block_number: u64, log_index: u64) -> Self {
        Self {
            block_number,
            log_index,
        }
    }

    #[must_use]
    pub const fn from_metadata(metadata: &RawLogMetadata) -> Option<Self> {
        metadata.cursor()
    }

    #[must_use]
    pub const fn from_log(log: &RawLog) -> Option<Self> {
        match log.metadata {
            Some(metadata) => metadata.cursor(),
            None => None,
        }
    }

    /// Re-query this block and post-filter logs at or before this cursor.
    #[must_use]
    pub const fn resume_from_block(&self) -> u64 {
        self.block_number
    }

    /// Start at the next block when the current block has been finalized.
    #[must_use]
    pub const fn next_block(&self) -> u64 {
        self.block_number.saturating_add(1)
    }
}

impl RawLog {
    #[must_use]
    pub fn new(address: Address, topics: Vec<B256>, data: Vec<u8>) -> Self {
        Self {
            address,
            topics,
            data,
            metadata: None,
        }
    }

    /// Construct a raw log from provider-style `0x` hex data.
    pub fn from_hex_data(
        address: Address,
        topics: Vec<B256>,
        data: impl AsRef<str>,
    ) -> Result<Self, RawLogError> {
        let data = strip_hex_prefix(data.as_ref());
        Ok(Self {
            address,
            topics,
            data: hex::decode(data)?,
            metadata: None,
        })
    }

    /// Construct a raw log from provider-style `0x` hex data with source metadata.
    pub fn from_hex_data_with_metadata(
        address: Address,
        topics: Vec<B256>,
        data: impl AsRef<str>,
        metadata: RawLogMetadata,
    ) -> Result<Self, RawLogError> {
        Ok(Self::from_hex_data(address, topics, data)?.with_metadata(metadata))
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: RawLogMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[must_use]
    pub fn without_metadata(mut self) -> Self {
        self.metadata = None;
        self
    }

    #[must_use]
    pub const fn cursor(&self) -> Option<RawLogCursor> {
        RawLogCursor::from_log(self)
    }

    /// Return the event signature topic when present.
    #[must_use]
    pub fn topic0(&self) -> Option<B256> {
        self.topics.first().copied()
    }

    /// Return log data as provider-style `0x` hex.
    #[must_use]
    pub fn data_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }
}

/// Errors that can occur while accepting provider-returned raw log data.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RawLogError {
    #[error("invalid raw log data hex: {0}")]
    InvalidDataHex(#[from] hex::FromHexError),
}

/// Errors that can occur while decoding a typed event log.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EventDecodeError {
    #[error("invalid event topic count: expected {expected}, got {actual}")]
    InvalidTopicCount { expected: usize, actual: usize },
    #[error("unexpected event topic0: expected {expected:?}, got {actual:?}")]
    UnexpectedTopic { expected: B256, actual: B256 },
    #[error(transparent)]
    Abi(#[from] AbiDecodeError),
}

/// Any core Tangent event currently understood by the SDK.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TangentEvent {
    AccountRegistered(AccountRegisteredEvent),
    Deposited(DepositedEvent),
    Withdrawn(WithdrawnEvent),
    MarginLocked(MarginAmountEvent),
    MarginReleased(MarginAmountEvent),
    PnlApplied(PnlAppliedEvent),
    OrderSubmitted(OrderSubmittedEvent),
    OrderCancelled(OrderCancelledEvent),
    Matched(MatchedEvent),
    Settled(SettledEvent),
    MarketRegistered(MarketRegisteredEvent),
    MarketParamsUpdated(MarketParamsUpdatedEvent),
    MarketPaused(MarketPausedEvent),
    Liquidated(LiquidatedEvent),
}

/// Result of decoding a batch of provider logs with [`TangentEvent::decode_logs`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedTangentLogs {
    pub events: Vec<TangentEvent>,
    pub unknown_logs: usize,
}

/// Compact summary for a decoded Tangent event batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedTangentLogsSummary {
    pub known_logs: usize,
    pub unknown_logs: usize,
    pub total_logs: usize,
    pub is_empty: bool,
    #[serde(default)]
    pub has_known_logs: bool,
    #[serde(default)]
    pub has_unknown_logs: bool,
    pub kind_counts: Vec<TangentEventKindCount>,
    pub nonzero_kind_counts: Vec<TangentEventKindCount>,
}

/// One decoded Tangent event plus its provider/indexer source position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedTangentLogRecord {
    pub event: TangentEvent,
    pub metadata: Option<RawLogMetadata>,
}

/// Result of decoding a batch of provider logs while preserving source metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedTangentLogRecords {
    pub records: Vec<DecodedTangentLogRecord>,
    pub unknown_logs: usize,
}

/// Compact summary for decoded Tangent event records with source metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedTangentLogRecordsSummary {
    pub known_logs: usize,
    pub unknown_logs: usize,
    pub total_logs: usize,
    pub is_empty: bool,
    #[serde(default)]
    pub has_known_logs: bool,
    #[serde(default)]
    pub has_unknown_logs: bool,
    pub records_with_cursor: usize,
    pub records_without_cursor: usize,
    #[serde(default)]
    pub has_cursor: bool,
    #[serde(default)]
    pub all_known_logs_have_cursor: bool,
    pub last_cursor: Option<RawLogCursor>,
    pub kind_counts: Vec<TangentEventKindCount>,
    pub nonzero_kind_counts: Vec<TangentEventKindCount>,
}

/// Count of one decoded Tangent event kind in a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentEventKindCount {
    pub kind: TangentEventKind,
    pub count: usize,
}

/// Known Tangent event identity without decoded event payload data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TangentEventKind {
    AccountRegistered,
    Deposited,
    Withdrawn,
    MarginLocked,
    MarginReleased,
    PnlApplied,
    OrderSubmitted,
    OrderCancelled,
    Matched,
    Settled,
    MarketRegistered,
    MarketParamsUpdated,
    MarketPaused,
    Liquidated,
}

/// Transport-neutral log filter spec for one Tangent event on one contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EventFilter {
    pub kind: TangentEventKind,
    pub address: Address,
    pub topic0: B256,
    pub signature: &'static str,
}

/// Exact Tangent event filters plus helpers for provider request construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventFilterSet {
    pub filters: Vec<EventFilter>,
}

/// Broad provider log-filter request shape derived from an [`EventFilterSet`].
///
/// JSON-RPC log filters can express "any of these addresses" and "any of
/// these topic0 values", but not exact address/topic pairs. Use
/// [`EventFilterSet::matches_log`] on returned logs to enforce the exact
/// Tangent event/contract mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventFilterRequest {
    pub addresses: Vec<Address>,
    pub topic0: Vec<B256>,
}

/// Provider-neutral event log query with an optional inclusive block range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogQuery {
    pub filter: EventFilterRequest,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
}

/// Compact review shape for one provider-neutral event-log query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogQuerySummary {
    pub address_count: usize,
    pub topic0_count: usize,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
    pub is_open_ended: bool,
    pub block_span: Option<u64>,
    pub has_invalid_range: bool,
}

/// Compact review shape for a fixed-order event-log query batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogQueryBatchSummary {
    pub len: usize,
    pub is_empty: bool,
    pub open_ended_queries: usize,
    pub invalid_range_queries: usize,
    pub total_block_span: Option<u64>,
    pub total_address_filters: usize,
    pub total_topic0_filters: usize,
    pub queries: Vec<EventLogQuerySummary>,
}

/// JSON-RPC-friendly `eth_getLogs` filter shape derived from [`EventLogQuery`].
///
/// The request is still broad over address/topic0 combinations. Consumers
/// should continue using [`EventFilterSet::matches_log`] or
/// [`EventFilterSet::decode_logs`] on returned logs to enforce exact
/// Tangent contract/event pairs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogRpcQuery {
    #[serde(rename = "address")]
    pub addresses: Vec<Address>,
    pub topics: Vec<Vec<B256>>,
    #[serde(rename = "fromBlock", skip_serializing_if = "Option::is_none")]
    pub from_block: Option<String>,
    #[serde(rename = "toBlock", skip_serializing_if = "Option::is_none")]
    pub to_block: Option<String>,
}

/// Compact review shape for one JSON-RPC-ready event-log query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogRpcQuerySummary {
    pub address_count: usize,
    pub topic0_count: usize,
    pub from_block: Option<String>,
    pub to_block: Option<String>,
    pub is_open_ended: bool,
}

/// Compact review shape for a fixed-order JSON-RPC-ready event-log query batch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogRpcQueryBatchSummary {
    pub len: usize,
    pub is_empty: bool,
    pub open_ended_queries: usize,
    pub total_address_filters: usize,
    pub total_topic0_filters: usize,
    pub queries: Vec<EventLogRpcQuerySummary>,
}

/// Errors that can occur while splitting provider log queries into block windows.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EventQueryError {
    #[error("event query chunk size must be greater than zero")]
    ZeroChunkSize,
    #[error("event query chunking requires both from_block and to_block")]
    OpenEndedRange,
    #[error("invalid event query block range: from_block {from_block} > to_block {to_block}")]
    InvalidRange { from_block: u64, to_block: u64 },
}

/// `AccountRegistered(uint256,address,uint64)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRegisteredEvent {
    pub account_id: u128,
    pub owner: Address,
    pub registered_at: u64,
}

/// `Deposited(uint256,address,uint256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositedEvent {
    pub account_id: u128,
    pub from: Address,
    pub amount: u128,
}

/// `Withdrawn(uint256,address,uint256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawnEvent {
    pub account_id: u128,
    pub to: Address,
    pub amount: u128,
}

/// `MarginLocked(uint256,uint256)` or `MarginReleased(uint256,uint256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarginAmountEvent {
    pub account_id: u128,
    pub amount: u128,
}

/// `PnLApplied(uint256,int256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PnlAppliedEvent {
    pub account_id: u128,
    pub pnl: i128,
}

/// `OrderSubmitted(bytes32,uint256,uint256,bool,uint256,uint256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderSubmittedEvent {
    pub order_hash: B256,
    pub account_id: u128,
    pub market_id: u128,
    pub is_buy: bool,
    pub limit_price: u128,
    pub size: u128,
}

/// `OrderCancelled(bytes32,uint256,string)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderCancelledEvent {
    pub order_hash: B256,
    pub account_id: u128,
    pub reason: String,
}

/// `Matched(bytes32,bytes32,uint256,uint256,uint256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedEvent {
    pub buy_order_hash: B256,
    pub sell_order_hash: B256,
    pub market_id: u128,
    pub size: u128,
    pub price: u128,
}

/// `Settled(bytes32,bytes32,uint256,uint256,uint256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettledEvent {
    pub buy_order_hash: B256,
    pub sell_order_hash: B256,
    pub market_id: u128,
    pub size: u128,
    pub price: u128,
}

/// `MarketRegistered(uint256,string,address)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketRegisteredEvent {
    pub market_id: u128,
    pub symbol: String,
    pub price_feed: Address,
}

/// `MarketParamsUpdated(uint256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketParamsUpdatedEvent {
    pub market_id: u128,
}

/// `MarketPaused(uint256,bool)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketPausedEvent {
    pub market_id: u128,
    pub paused: bool,
}

/// `Liquidated(uint256,uint256,address,uint256,int256)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidatedEvent {
    pub account_id: u128,
    pub market_id: u128,
    pub liquidator: Address,
    pub mark_price: u128,
    pub pnl: i128,
}

impl TangentEvent {
    /// Return this decoded event's identity without payload data.
    #[must_use]
    pub const fn kind(&self) -> TangentEventKind {
        match self {
            Self::AccountRegistered(_) => TangentEventKind::AccountRegistered,
            Self::Deposited(_) => TangentEventKind::Deposited,
            Self::Withdrawn(_) => TangentEventKind::Withdrawn,
            Self::MarginLocked(_) => TangentEventKind::MarginLocked,
            Self::MarginReleased(_) => TangentEventKind::MarginReleased,
            Self::PnlApplied(_) => TangentEventKind::PnlApplied,
            Self::OrderSubmitted(_) => TangentEventKind::OrderSubmitted,
            Self::OrderCancelled(_) => TangentEventKind::OrderCancelled,
            Self::Matched(_) => TangentEventKind::Matched,
            Self::Settled(_) => TangentEventKind::Settled,
            Self::MarketRegistered(_) => TangentEventKind::MarketRegistered,
            Self::MarketParamsUpdated(_) => TangentEventKind::MarketParamsUpdated,
            Self::MarketPaused(_) => TangentEventKind::MarketPaused,
            Self::Liquidated(_) => TangentEventKind::Liquidated,
        }
    }

    /// Decode a raw log when its `topic0` matches a known Tangent event.
    ///
    /// Unknown logs return `Ok(None)` so callers can pass mixed transaction
    /// receipts through this helper without treating unrelated events as errors.
    /// Malformed known logs return an [`EventDecodeError`].
    pub fn decode_known(log: &RawLog) -> Result<Option<Self>, EventDecodeError> {
        let Some(topic0) = log.topic0() else {
            return Ok(None);
        };

        if topic0 == AccountRegisteredEvent::topic0() {
            return AccountRegisteredEvent::decode(log)
                .map(Self::AccountRegistered)
                .map(Some);
        }
        if topic0 == DepositedEvent::topic0() {
            return DepositedEvent::decode(log).map(Self::Deposited).map(Some);
        }
        if topic0 == WithdrawnEvent::topic0() {
            return WithdrawnEvent::decode(log).map(Self::Withdrawn).map(Some);
        }
        if topic0 == MarginAmountEvent::locked_topic0() {
            return MarginAmountEvent::decode_locked(log)
                .map(Self::MarginLocked)
                .map(Some);
        }
        if topic0 == MarginAmountEvent::released_topic0() {
            return MarginAmountEvent::decode_released(log)
                .map(Self::MarginReleased)
                .map(Some);
        }
        if topic0 == PnlAppliedEvent::topic0() {
            return PnlAppliedEvent::decode(log).map(Self::PnlApplied).map(Some);
        }
        if topic0 == OrderSubmittedEvent::topic0() {
            return OrderSubmittedEvent::decode(log)
                .map(Self::OrderSubmitted)
                .map(Some);
        }
        if topic0 == OrderCancelledEvent::topic0() {
            return OrderCancelledEvent::decode(log)
                .map(Self::OrderCancelled)
                .map(Some);
        }
        if topic0 == MatchedEvent::topic0() {
            return MatchedEvent::decode(log).map(Self::Matched).map(Some);
        }
        if topic0 == SettledEvent::topic0() {
            return SettledEvent::decode(log).map(Self::Settled).map(Some);
        }
        if topic0 == MarketRegisteredEvent::topic0() {
            return MarketRegisteredEvent::decode(log)
                .map(Self::MarketRegistered)
                .map(Some);
        }
        if topic0 == MarketParamsUpdatedEvent::topic0() {
            return MarketParamsUpdatedEvent::decode(log)
                .map(Self::MarketParamsUpdated)
                .map(Some);
        }
        if topic0 == MarketPausedEvent::topic0() {
            return MarketPausedEvent::decode(log)
                .map(Self::MarketPaused)
                .map(Some);
        }
        if topic0 == LiquidatedEvent::topic0() {
            return LiquidatedEvent::decode(log).map(Self::Liquidated).map(Some);
        }

        Ok(None)
    }

    /// Decode every known Tangent event in a mixed batch of raw logs.
    ///
    /// Unknown logs are counted and skipped. Malformed logs with known Tangent
    /// topics still return an error, which lets receipt processors distinguish
    /// unrelated third-party logs from broken Tangent ABI data.
    pub fn decode_logs<'a>(
        logs: impl IntoIterator<Item = &'a RawLog>,
    ) -> Result<DecodedTangentLogs, EventDecodeError> {
        Self::decode_log_records(logs).map(DecodedTangentLogRecords::into_decoded_logs)
    }

    /// Decode every known Tangent event in a mixed batch and retain log metadata.
    pub fn decode_log_records<'a>(
        logs: impl IntoIterator<Item = &'a RawLog>,
    ) -> Result<DecodedTangentLogRecords, EventDecodeError> {
        let mut records = Vec::new();
        let mut unknown_logs = 0;

        for log in logs {
            match Self::decode_known(log)? {
                Some(event) => records.push(DecodedTangentLogRecord::new(event, log.metadata)),
                None => unknown_logs += 1,
            }
        }

        Ok(DecodedTangentLogRecords {
            records,
            unknown_logs,
        })
    }
}

impl DecodedTangentLogRecord {
    #[must_use]
    pub const fn new(event: TangentEvent, metadata: Option<RawLogMetadata>) -> Self {
        Self { event, metadata }
    }

    #[must_use]
    pub const fn kind(&self) -> TangentEventKind {
        self.event.kind()
    }

    #[must_use]
    pub const fn cursor(&self) -> Option<RawLogCursor> {
        match self.metadata {
            Some(metadata) => metadata.cursor(),
            None => None,
        }
    }

    #[must_use]
    pub const fn block_number(&self) -> Option<u64> {
        match self.metadata {
            Some(metadata) => metadata.block_number,
            None => None,
        }
    }

    #[must_use]
    pub const fn transaction_hash(&self) -> Option<B256> {
        match self.metadata {
            Some(metadata) => metadata.transaction_hash,
            None => None,
        }
    }

    #[must_use]
    pub const fn log_index(&self) -> Option<u64> {
        match self.metadata {
            Some(metadata) => metadata.log_index,
            None => None,
        }
    }
}

impl DecodedTangentLogRecords {
    pub fn extend(&mut self, other: Self) {
        self.records.extend(other.records);
        self.unknown_logs = self.unknown_logs.saturating_add(other.unknown_logs);
    }

    #[must_use]
    pub fn from_batches(batches: impl IntoIterator<Item = Self>) -> Self {
        let mut combined = Self::default();
        for batch in batches {
            combined.extend(batch);
        }
        combined
    }

    #[must_use]
    pub fn into_decoded_logs(self) -> DecodedTangentLogs {
        DecodedTangentLogs {
            events: self
                .records
                .into_iter()
                .map(|record| record.event)
                .collect(),
            unknown_logs: self.unknown_logs,
        }
    }

    #[must_use]
    pub fn known_logs(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn total_logs(&self) -> usize {
        self.records.len() + self.unknown_logs
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.unknown_logs == 0
    }

    #[must_use]
    pub fn last_cursor(&self) -> Option<RawLogCursor> {
        self.records
            .iter()
            .filter_map(DecodedTangentLogRecord::cursor)
            .max()
    }

    #[must_use]
    pub fn count_kind(&self, kind: TangentEventKind) -> usize {
        self.records
            .iter()
            .filter(|record| record.kind() == kind)
            .count()
    }

    #[must_use]
    pub fn contains_kind(&self, kind: TangentEventKind) -> bool {
        self.records.iter().any(|record| record.kind() == kind)
    }

    /// Return counts for every known event kind in canonical order.
    #[must_use]
    pub fn kind_counts(&self) -> Vec<TangentEventKindCount> {
        TangentEventKind::ALL
            .into_iter()
            .map(|kind| TangentEventKindCount {
                kind,
                count: self.count_kind(kind),
            })
            .collect()
    }

    /// Return non-zero event kind counts in canonical order.
    #[must_use]
    pub fn nonzero_kind_counts(&self) -> Vec<TangentEventKindCount> {
        self.kind_counts()
            .into_iter()
            .filter(|entry| entry.count != 0)
            .collect()
    }

    /// Return a compact source-preserving batch summary for logs and checkpoints.
    #[must_use]
    pub fn summary(&self) -> DecodedTangentLogRecordsSummary {
        let known_logs = self.known_logs();
        let records_with_cursor = self
            .records
            .iter()
            .filter(|record| record.cursor().is_some())
            .count();
        let records_without_cursor = known_logs.saturating_sub(records_with_cursor);
        let last_cursor = self.last_cursor();
        DecodedTangentLogRecordsSummary {
            known_logs,
            unknown_logs: self.unknown_logs,
            total_logs: self.total_logs(),
            is_empty: self.is_empty(),
            has_known_logs: known_logs > 0,
            has_unknown_logs: self.unknown_logs > 0,
            records_with_cursor,
            records_without_cursor,
            has_cursor: last_cursor.is_some(),
            all_known_logs_have_cursor: known_logs > 0 && records_without_cursor == 0,
            last_cursor,
            kind_counts: self.kind_counts(),
            nonzero_kind_counts: self.nonzero_kind_counts(),
        }
    }
}

impl DecodedTangentLogs {
    pub fn extend(&mut self, other: Self) {
        self.events.extend(other.events);
        self.unknown_logs = self.unknown_logs.saturating_add(other.unknown_logs);
    }

    #[must_use]
    pub fn from_batches(batches: impl IntoIterator<Item = Self>) -> Self {
        let mut combined = Self::default();
        for batch in batches {
            combined.extend(batch);
        }
        combined
    }

    #[must_use]
    pub fn known_logs(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn total_logs(&self) -> usize {
        self.events.len() + self.unknown_logs
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.unknown_logs == 0
    }

    #[must_use]
    pub fn count_kind(&self, kind: TangentEventKind) -> usize {
        self.events
            .iter()
            .filter(|event| event.kind() == kind)
            .count()
    }

    #[must_use]
    pub fn contains_kind(&self, kind: TangentEventKind) -> bool {
        self.events.iter().any(|event| event.kind() == kind)
    }

    /// Return counts for every known event kind in canonical order.
    #[must_use]
    pub fn kind_counts(&self) -> Vec<TangentEventKindCount> {
        TangentEventKind::ALL
            .into_iter()
            .map(|kind| TangentEventKindCount {
                kind,
                count: self.count_kind(kind),
            })
            .collect()
    }

    /// Return non-zero event kind counts in canonical order.
    #[must_use]
    pub fn nonzero_kind_counts(&self) -> Vec<TangentEventKindCount> {
        self.kind_counts()
            .into_iter()
            .filter(|entry| entry.count != 0)
            .collect()
    }

    /// Return a compact decoded-log batch summary for logs and operator UIs.
    #[must_use]
    pub fn summary(&self) -> DecodedTangentLogsSummary {
        let known_logs = self.known_logs();
        DecodedTangentLogsSummary {
            known_logs,
            unknown_logs: self.unknown_logs,
            total_logs: self.total_logs(),
            is_empty: self.is_empty(),
            has_known_logs: known_logs > 0,
            has_unknown_logs: self.unknown_logs > 0,
            kind_counts: self.kind_counts(),
            nonzero_kind_counts: self.nonzero_kind_counts(),
        }
    }
}

impl TangentEventKind {
    pub const ALL: [Self; 14] = [
        Self::AccountRegistered,
        Self::Deposited,
        Self::Withdrawn,
        Self::MarginLocked,
        Self::MarginReleased,
        Self::PnlApplied,
        Self::OrderSubmitted,
        Self::OrderCancelled,
        Self::Matched,
        Self::Settled,
        Self::MarketRegistered,
        Self::MarketParamsUpdated,
        Self::MarketPaused,
        Self::Liquidated,
    ];

    #[must_use]
    pub const fn signature(self) -> &'static str {
        match self {
            Self::AccountRegistered => AccountRegisteredEvent::SIGNATURE,
            Self::Deposited => DepositedEvent::SIGNATURE,
            Self::Withdrawn => WithdrawnEvent::SIGNATURE,
            Self::MarginLocked => MarginAmountEvent::LOCKED_SIGNATURE,
            Self::MarginReleased => MarginAmountEvent::RELEASED_SIGNATURE,
            Self::PnlApplied => PnlAppliedEvent::SIGNATURE,
            Self::OrderSubmitted => OrderSubmittedEvent::SIGNATURE,
            Self::OrderCancelled => OrderCancelledEvent::SIGNATURE,
            Self::Matched => MatchedEvent::SIGNATURE,
            Self::Settled => SettledEvent::SIGNATURE,
            Self::MarketRegistered => MarketRegisteredEvent::SIGNATURE,
            Self::MarketParamsUpdated => MarketParamsUpdatedEvent::SIGNATURE,
            Self::MarketPaused => MarketPausedEvent::SIGNATURE,
            Self::Liquidated => LiquidatedEvent::SIGNATURE,
        }
    }

    #[must_use]
    pub fn topic0(self) -> B256 {
        event_topic(self.signature())
    }

    /// Build canonical event filters from a deployment manifest.
    ///
    /// Optional full-stack perp contracts are included only when the manifest
    /// publishes their addresses, so the current v0.1 Arc manifest still
    /// produces useful primitive filters without pretending the full DEX stack
    /// is deployed.
    #[must_use]
    pub fn filters_for_manifest(manifest: &DeploymentManifest) -> Vec<EventFilter> {
        let mut filters = Vec::new();

        filters.push(Self::AccountRegistered.filter(manifest.contracts.account_manager));

        filters.extend([
            Self::Deposited.filter(manifest.contracts.usdc_vault),
            Self::Withdrawn.filter(manifest.contracts.usdc_vault),
            Self::MarginLocked.filter(manifest.contracts.usdc_vault),
            Self::MarginReleased.filter(manifest.contracts.usdc_vault),
            Self::PnlApplied.filter(manifest.contracts.usdc_vault),
        ]);

        filters.extend([
            Self::MarketRegistered.filter(manifest.contracts.market_registry),
            Self::MarketParamsUpdated.filter(manifest.contracts.market_registry),
            Self::MarketPaused.filter(manifest.contracts.market_registry),
        ]);

        if let Some(order_book) = manifest.contracts.order_book {
            filters.extend([
                Self::OrderSubmitted.filter(order_book),
                Self::OrderCancelled.filter(order_book),
                Self::Matched.filter(order_book),
            ]);
        }

        if let Some(settlement_engine) = manifest.contracts.settlement_engine {
            filters.push(Self::Settled.filter(settlement_engine));
        }

        if let Some(liquidation_keeper) = manifest.contracts.liquidation_keeper {
            filters.push(Self::Liquidated.filter(liquidation_keeper));
        }

        filters
    }

    #[must_use]
    pub fn filter(self, address: Address) -> EventFilter {
        EventFilter {
            kind: self,
            address,
            topic0: self.topic0(),
            signature: self.signature(),
        }
    }
}

impl EventFilter {
    /// True when a raw log came from this filter's contract and event topic.
    #[must_use]
    pub fn matches_log(&self, log: &RawLog) -> bool {
        log.address == self.address && log.topic0() == Some(self.topic0)
    }
}

impl EventFilterSet {
    #[must_use]
    pub fn new(filters: Vec<EventFilter>) -> Self {
        Self { filters }
    }

    #[must_use]
    pub fn from_manifest(manifest: &DeploymentManifest) -> Self {
        Self::new(TangentEventKind::filters_for_manifest(manifest))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    /// Build a broad provider request from this exact filter set.
    #[must_use]
    pub fn to_request(&self) -> EventFilterRequest {
        let mut addresses = Vec::new();
        let mut topic0 = Vec::new();

        for filter in &self.filters {
            if !addresses.contains(&filter.address) {
                addresses.push(filter.address);
            }
            if !topic0.contains(&filter.topic0) {
                topic0.push(filter.topic0);
            }
        }

        EventFilterRequest { addresses, topic0 }
    }

    /// Build a provider-neutral log query with an optional inclusive block range.
    #[must_use]
    pub fn to_query(&self, from_block: Option<u64>, to_block: Option<u64>) -> EventLogQuery {
        EventLogQuery {
            filter: self.to_request(),
            from_block,
            to_block,
        }
    }

    /// Build a provider-neutral resume query from a persisted log cursor.
    ///
    /// The query starts at the cursor's block so callers can re-fetch that
    /// block, then post-filter logs at or before the cursor.
    #[must_use]
    pub fn resume_query(&self, cursor: RawLogCursor, to_block: Option<u64>) -> EventLogQuery {
        self.to_query(Some(cursor.resume_from_block()), to_block)
    }

    /// Build a JSON-RPC-friendly resume query from a persisted log cursor.
    #[must_use]
    pub fn resume_rpc_query(
        &self,
        cursor: RawLogCursor,
        to_block: Option<u64>,
    ) -> EventLogRpcQuery {
        self.resume_query(cursor, to_block).to_rpc_query()
    }

    /// Build provider-neutral log queries split into inclusive block windows.
    pub fn chunked_queries(
        &self,
        from_block: u64,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<Vec<EventLogQuery>, EventQueryError> {
        self.to_query(Some(from_block), Some(to_block))
            .chunked(max_blocks)
    }

    /// Build JSON-RPC-friendly log queries split into inclusive block windows.
    pub fn chunked_rpc_queries(
        &self,
        from_block: u64,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<Vec<EventLogRpcQuery>, EventQueryError> {
        self.chunked_queries(from_block, to_block, max_blocks)
            .map(|queries| {
                queries
                    .into_iter()
                    .map(|query| query.to_rpc_query())
                    .collect()
            })
    }

    /// Return the exact filter matched by this log, if it is a Tangent log.
    #[must_use]
    pub fn matching_filter(&self, log: &RawLog) -> Option<EventFilter> {
        self.filters
            .iter()
            .copied()
            .find(|filter| filter.matches_log(log))
    }

    /// True when this log matches one exact event/contract pair in the set.
    #[must_use]
    pub fn matches_log(&self, log: &RawLog) -> bool {
        self.matching_filter(log).is_some()
    }

    /// Decode logs that match this exact manifest-derived filter set.
    ///
    /// Logs outside the exact address/topic pairs are counted as unknown. Logs
    /// that match an exact filter but carry malformed ABI data return an error.
    pub fn decode_logs<'a>(
        &self,
        logs: impl IntoIterator<Item = &'a RawLog>,
    ) -> Result<DecodedTangentLogs, EventDecodeError> {
        self.decode_log_records(logs)
            .map(DecodedTangentLogRecords::into_decoded_logs)
    }

    /// Decode logs that match this exact filter set and retain log metadata.
    ///
    /// Logs outside the exact address/topic pairs are counted as unknown. Logs
    /// that match an exact filter but carry malformed ABI data return an error.
    pub fn decode_log_records<'a>(
        &self,
        logs: impl IntoIterator<Item = &'a RawLog>,
    ) -> Result<DecodedTangentLogRecords, EventDecodeError> {
        let mut records = Vec::new();
        let mut unknown_logs = 0;

        for log in logs {
            if !self.matches_log(log) {
                unknown_logs += 1;
                continue;
            }

            match TangentEvent::decode_known(log)? {
                Some(event) => records.push(DecodedTangentLogRecord::new(event, log.metadata)),
                None => unknown_logs += 1,
            }
        }

        Ok(DecodedTangentLogRecords {
            records,
            unknown_logs,
        })
    }

    /// Decode only logs after a persisted cursor.
    ///
    /// Logs with no cursor are retained because this SDK cannot prove whether
    /// they are older than the checkpoint. Logs with cursors at or before the
    /// checkpoint are skipped before exact event matching and ABI decoding.
    pub fn decode_logs_after_cursor<'a>(
        &self,
        logs: impl IntoIterator<Item = &'a RawLog>,
        cursor: RawLogCursor,
    ) -> Result<DecodedTangentLogs, EventDecodeError> {
        self.decode_log_records_after_cursor(logs, cursor)
            .map(DecodedTangentLogRecords::into_decoded_logs)
    }

    /// Decode only logs after a persisted cursor and retain log metadata.
    pub fn decode_log_records_after_cursor<'a>(
        &self,
        logs: impl IntoIterator<Item = &'a RawLog>,
        cursor: RawLogCursor,
    ) -> Result<DecodedTangentLogRecords, EventDecodeError> {
        self.decode_log_records(logs.into_iter().filter(|log| match log.cursor() {
            Some(log_cursor) => log_cursor > cursor,
            None => true,
        }))
    }
}

impl EventFilterRequest {
    /// Build a JSON-RPC-friendly log filter without block bounds.
    #[must_use]
    pub fn to_rpc_query(&self) -> EventLogRpcQuery {
        EventLogRpcQuery {
            addresses: self.addresses.clone(),
            topics: vec![self.topic0.clone()],
            from_block: None,
            to_block: None,
        }
    }
}

impl EventLogQuery {
    #[must_use]
    pub const fn is_open_ended(&self) -> bool {
        self.from_block.is_none() || self.to_block.is_none()
    }

    #[must_use]
    pub fn summary(&self) -> EventLogQuerySummary {
        let (block_span, has_invalid_range) = event_block_span(self.from_block, self.to_block);
        EventLogQuerySummary {
            address_count: self.filter.addresses.len(),
            topic0_count: self.filter.topic0.len(),
            from_block: self.from_block,
            to_block: self.to_block,
            is_open_ended: self.is_open_ended(),
            block_span,
            has_invalid_range,
        }
    }

    #[must_use]
    pub fn summarize_batch(queries: &[Self]) -> EventLogQueryBatchSummary {
        let queries = queries
            .iter()
            .map(Self::summary)
            .collect::<Vec<EventLogQuerySummary>>();
        let open_ended_queries = queries
            .iter()
            .filter(|summary| summary.is_open_ended)
            .count();
        let invalid_range_queries = queries
            .iter()
            .filter(|summary| summary.has_invalid_range)
            .count();
        let total_block_span = queries.iter().try_fold(0_u64, |total, summary| {
            summary.block_span.and_then(|span| total.checked_add(span))
        });
        let total_address_filters = queries.iter().map(|summary| summary.address_count).sum();
        let total_topic0_filters = queries.iter().map(|summary| summary.topic0_count).sum();

        EventLogQueryBatchSummary {
            len: queries.len(),
            is_empty: queries.is_empty(),
            open_ended_queries,
            invalid_range_queries,
            total_block_span,
            total_address_filters,
            total_topic0_filters,
            queries,
        }
    }

    /// Build a JSON-RPC-friendly `eth_getLogs` filter.
    #[must_use]
    pub fn to_rpc_query(&self) -> EventLogRpcQuery {
        EventLogRpcQuery {
            addresses: self.filter.addresses.clone(),
            topics: vec![self.filter.topic0.clone()],
            from_block: self.from_block.map(block_quantity_hex),
            to_block: self.to_block.map(block_quantity_hex),
        }
    }

    /// Split this query into inclusive block windows while preserving its filter.
    ///
    /// `max_blocks` is an inclusive block count, so `100..=105` with
    /// `max_blocks = 2` yields `100..=101`, `102..=103`, and `104..=105`.
    pub fn chunked(&self, max_blocks: u64) -> Result<Vec<Self>, EventQueryError> {
        if max_blocks == 0 {
            return Err(EventQueryError::ZeroChunkSize);
        }

        let (from_block, to_block) = match (self.from_block, self.to_block) {
            (Some(from_block), Some(to_block)) => (from_block, to_block),
            _ => return Err(EventQueryError::OpenEndedRange),
        };

        if from_block > to_block {
            return Err(EventQueryError::InvalidRange {
                from_block,
                to_block,
            });
        }

        let mut chunks = Vec::new();
        let mut start = from_block;

        while start <= to_block {
            let end = start.saturating_add(max_blocks - 1).min(to_block);
            chunks.push(Self {
                filter: self.filter.clone(),
                from_block: Some(start),
                to_block: Some(end),
            });

            if end == u64::MAX {
                break;
            }
            start = end + 1;
        }

        Ok(chunks)
    }

    /// Split this query and return JSON-RPC-friendly `eth_getLogs` filters.
    pub fn chunked_rpc(&self, max_blocks: u64) -> Result<Vec<EventLogRpcQuery>, EventQueryError> {
        self.chunked(max_blocks).map(|queries| {
            queries
                .into_iter()
                .map(|query| query.to_rpc_query())
                .collect()
        })
    }
}

impl EventLogRpcQuery {
    #[must_use]
    pub fn summary(&self) -> EventLogRpcQuerySummary {
        EventLogRpcQuerySummary {
            address_count: self.addresses.len(),
            topic0_count: self.topics.first().map_or(0, Vec::len),
            from_block: self.from_block.clone(),
            to_block: self.to_block.clone(),
            is_open_ended: self.from_block.is_none() || self.to_block.is_none(),
        }
    }

    #[must_use]
    pub fn summarize_batch(queries: &[Self]) -> EventLogRpcQueryBatchSummary {
        let queries = queries
            .iter()
            .map(Self::summary)
            .collect::<Vec<EventLogRpcQuerySummary>>();
        let open_ended_queries = queries
            .iter()
            .filter(|summary| summary.is_open_ended)
            .count();
        let total_address_filters = queries.iter().map(|summary| summary.address_count).sum();
        let total_topic0_filters = queries.iter().map(|summary| summary.topic0_count).sum();

        EventLogRpcQueryBatchSummary {
            len: queries.len(),
            is_empty: queries.is_empty(),
            open_ended_queries,
            total_address_filters,
            total_topic0_filters,
            queries,
        }
    }
}

impl AccountRegisteredEvent {
    pub const SIGNATURE: &'static str = "AccountRegistered(uint256,address,uint64)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 3)?;
        let registered_at = decode_u64_word(data_word(&log.data, 0, 1)?)?;
        Ok(Self {
            account_id: topic_u128(&log.topics[1])?,
            owner: topic_address(&log.topics[2])?,
            registered_at,
        })
    }
}

impl DepositedEvent {
    pub const SIGNATURE: &'static str = "Deposited(uint256,address,uint256)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 3)?;
        Ok(Self {
            account_id: topic_u128(&log.topics[1])?,
            from: topic_address(&log.topics[2])?,
            amount: crate::abi::decode_u128(data_word(&log.data, 0, 1)?)?,
        })
    }
}

impl WithdrawnEvent {
    pub const SIGNATURE: &'static str = "Withdrawn(uint256,address,uint256)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 3)?;
        Ok(Self {
            account_id: topic_u128(&log.topics[1])?,
            to: topic_address(&log.topics[2])?,
            amount: crate::abi::decode_u128(data_word(&log.data, 0, 1)?)?,
        })
    }
}

impl MarginAmountEvent {
    pub const LOCKED_SIGNATURE: &'static str = "MarginLocked(uint256,uint256)";
    pub const RELEASED_SIGNATURE: &'static str = "MarginReleased(uint256,uint256)";

    #[must_use]
    pub fn locked_topic0() -> B256 {
        event_topic(Self::LOCKED_SIGNATURE)
    }

    #[must_use]
    pub fn released_topic0() -> B256 {
        event_topic(Self::RELEASED_SIGNATURE)
    }

    pub fn decode_locked(log: &RawLog) -> Result<Self, EventDecodeError> {
        Self::decode_with_topic(log, Self::locked_topic0())
    }

    pub fn decode_released(log: &RawLog) -> Result<Self, EventDecodeError> {
        Self::decode_with_topic(log, Self::released_topic0())
    }

    fn decode_with_topic(log: &RawLog, topic0: B256) -> Result<Self, EventDecodeError> {
        check_topics(log, topic0, 2)?;
        Ok(Self {
            account_id: topic_u128(&log.topics[1])?,
            amount: crate::abi::decode_u128(data_word(&log.data, 0, 1)?)?,
        })
    }
}

impl PnlAppliedEvent {
    pub const SIGNATURE: &'static str = "PnLApplied(uint256,int256)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 2)?;
        Ok(Self {
            account_id: topic_u128(&log.topics[1])?,
            pnl: crate::abi::decode_i128(data_word(&log.data, 0, 1)?)?,
        })
    }
}

impl OrderSubmittedEvent {
    pub const SIGNATURE: &'static str =
        "OrderSubmitted(bytes32,uint256,uint256,bool,uint256,uint256)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 4)?;
        Ok(Self {
            order_hash: log.topics[1],
            account_id: topic_u128(&log.topics[2])?,
            market_id: topic_u128(&log.topics[3])?,
            is_buy: crate::abi::decode_bool(data_word(&log.data, 0, 3)?)?,
            limit_price: crate::abi::decode_u128(data_word(&log.data, 1, 3)?)?,
            size: crate::abi::decode_u128(data_word(&log.data, 2, 3)?)?,
        })
    }
}

impl OrderCancelledEvent {
    pub const SIGNATURE: &'static str = "OrderCancelled(bytes32,uint256,string)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 3)?;
        Ok(Self {
            order_hash: log.topics[1],
            account_id: topic_u128(&log.topics[2])?,
            reason: crate::abi::decode_dynamic_string(&log.data, 0, 1)?,
        })
    }
}

impl MatchedEvent {
    pub const SIGNATURE: &'static str = "Matched(bytes32,bytes32,uint256,uint256,uint256)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 4)?;
        Ok(Self {
            buy_order_hash: log.topics[1],
            sell_order_hash: log.topics[2],
            market_id: topic_u128(&log.topics[3])?,
            size: crate::abi::decode_u128(data_word(&log.data, 0, 2)?)?,
            price: crate::abi::decode_u128(data_word(&log.data, 1, 2)?)?,
        })
    }
}

impl SettledEvent {
    pub const SIGNATURE: &'static str = "Settled(bytes32,bytes32,uint256,uint256,uint256)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 4)?;
        Ok(Self {
            buy_order_hash: log.topics[1],
            sell_order_hash: log.topics[2],
            market_id: topic_u128(&log.topics[3])?,
            size: crate::abi::decode_u128(data_word(&log.data, 0, 2)?)?,
            price: crate::abi::decode_u128(data_word(&log.data, 1, 2)?)?,
        })
    }
}

impl MarketRegisteredEvent {
    pub const SIGNATURE: &'static str = "MarketRegistered(uint256,string,address)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 2)?;
        Ok(Self {
            market_id: topic_u128(&log.topics[1])?,
            symbol: crate::abi::decode_dynamic_string(&log.data, 0, 2)?,
            price_feed: crate::abi::decode_address(head_word(&log.data, 1, 2)?)?,
        })
    }
}

impl MarketParamsUpdatedEvent {
    pub const SIGNATURE: &'static str = "MarketParamsUpdated(uint256)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 2)?;
        crate::abi::decode_empty(&log.data)?;
        Ok(Self {
            market_id: topic_u128(&log.topics[1])?,
        })
    }
}

impl MarketPausedEvent {
    pub const SIGNATURE: &'static str = "MarketPaused(uint256,bool)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 2)?;
        Ok(Self {
            market_id: topic_u128(&log.topics[1])?,
            paused: crate::abi::decode_bool(data_word(&log.data, 0, 1)?)?,
        })
    }
}

impl LiquidatedEvent {
    pub const SIGNATURE: &'static str = "Liquidated(uint256,uint256,address,uint256,int256)";

    #[must_use]
    pub fn topic0() -> B256 {
        event_topic(Self::SIGNATURE)
    }

    pub fn decode(log: &RawLog) -> Result<Self, EventDecodeError> {
        check_topics(log, Self::topic0(), 4)?;
        Ok(Self {
            account_id: topic_u128(&log.topics[1])?,
            market_id: topic_u128(&log.topics[2])?,
            liquidator: topic_address(&log.topics[3])?,
            mark_price: crate::abi::decode_u128(data_word(&log.data, 0, 2)?)?,
            pnl: crate::abi::decode_i128(data_word(&log.data, 1, 2)?)?,
        })
    }
}

#[must_use]
pub fn event_topic(signature: &str) -> B256 {
    keccak256(signature.as_bytes())
}

fn check_topics(
    log: &RawLog,
    expected_topic0: B256,
    expected_count: usize,
) -> Result<(), EventDecodeError> {
    if log.topics.len() != expected_count {
        return Err(EventDecodeError::InvalidTopicCount {
            expected: expected_count,
            actual: log.topics.len(),
        });
    }

    let actual = log.topics[0];
    if actual != expected_topic0 {
        return Err(EventDecodeError::UnexpectedTopic {
            expected: expected_topic0,
            actual,
        });
    }

    Ok(())
}

fn data_word(data: &[u8], index: usize, expected_words: usize) -> Result<&[u8], EventDecodeError> {
    let expected = expected_words * 32;
    if data.len() != expected {
        return Err(EventDecodeError::Abi(AbiDecodeError::InvalidLength {
            expected,
            actual: data.len(),
        }));
    }

    let start = index * 32;
    Ok(&data[start..start + 32])
}

fn head_word(data: &[u8], index: usize, head_words: usize) -> Result<&[u8], EventDecodeError> {
    let expected = head_words * 32;
    if data.len() < expected {
        return Err(EventDecodeError::Abi(AbiDecodeError::InvalidLength {
            expected,
            actual: data.len(),
        }));
    }

    let start = index * 32;
    Ok(&data[start..start + 32])
}

fn topic_u128(topic: &B256) -> Result<u128, EventDecodeError> {
    Ok(crate::abi::decode_u128(topic.as_slice())?)
}

fn topic_address(topic: &B256) -> Result<Address, EventDecodeError> {
    Ok(crate::abi::decode_address(topic.as_slice())?)
}

fn decode_u64_word(word: &[u8]) -> Result<u64, EventDecodeError> {
    let value = crate::abi::decode_u128(word)?;
    u64::try_from(value).map_err(|_| EventDecodeError::Abi(AbiDecodeError::UintOverflow))
}

fn strip_hex_prefix(value: &str) -> &str {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
}

fn block_quantity_hex(block: u64) -> String {
    format!("0x{block:x}")
}

fn event_block_span(from_block: Option<u64>, to_block: Option<u64>) -> (Option<u64>, bool) {
    match (from_block, to_block) {
        (Some(from_block), Some(to_block)) if from_block <= to_block => (
            to_block
                .checked_sub(from_block)
                .and_then(|delta| delta.checked_add(1)),
            false,
        ),
        (Some(_), Some(_)) => (None, true),
        _ => (None, false),
    }
}

mod raw_log_data {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::{strip_hex_prefix, RawLogError};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(bytes)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        hex::decode(strip_hex_prefix(&encoded))
            .map_err(RawLogError::InvalidDataHex)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(value: u128) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[16..].copy_from_slice(&value.to_be_bytes());
        out
    }

    fn signed_word(value: i128) -> [u8; 32] {
        let mut out = if value < 0 { [0xffu8; 32] } else { [0u8; 32] };
        out[16..].copy_from_slice(&value.to_be_bytes());
        out
    }

    fn encoded_string(value: &str, head_words: u128) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&word(head_words * 32));
        out.extend_from_slice(&word(value.len() as u128));
        let padded_len = value.len().div_ceil(32) * 32;
        let mut padded = vec![0u8; padded_len];
        padded[..value.len()].copy_from_slice(value.as_bytes());
        out.extend_from_slice(&padded);
        out
    }

    fn encoded_string_and_address(value: &str, address: Address) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&word(64));
        let mut address_word = [0u8; 32];
        address_word[12..].copy_from_slice(address.as_slice());
        out.extend_from_slice(&address_word);
        out.extend_from_slice(&word(value.len() as u128));
        let padded_len = value.len().div_ceil(32) * 32;
        let mut padded = vec![0u8; padded_len];
        padded[..value.len()].copy_from_slice(value.as_bytes());
        out.extend_from_slice(&padded);
        out
    }

    fn topic_u128(value: u128) -> B256 {
        B256::from(word(value))
    }

    fn topic_address(value: Address) -> B256 {
        let mut out = [0u8; 32];
        out[12..].copy_from_slice(value.as_slice());
        B256::from(out)
    }

    fn full_stack_manifest() -> DeploymentManifest {
        DeploymentManifest {
            project: "Tangent".to_owned(),
            version: "0.1.0".to_owned(),
            chain_id: 11111,
            network: "arc-testnet".to_owned(),
            deployed_at: "2026-05-25T18:42:40.104Z".to_owned(),
            deployer: Address::repeat_byte(0x10),
            contracts: crate::ContractAddresses {
                account_manager: Address::repeat_byte(0x11),
                usdc_vault: Address::repeat_byte(0x12),
                market_registry: Address::repeat_byte(0x13),
                order_book: Some(Address::repeat_byte(0x14)),
                settlement_engine: Some(Address::repeat_byte(0x15)),
                liquidation_keeper: Some(Address::repeat_byte(0x16)),
            },
            verified_on_arcscan: true,
            constants: crate::NetworkConstants {
                usdc: Address::repeat_byte(0x17),
            },
        }
    }

    #[test]
    fn builds_event_filters_from_current_manifest() {
        let manifest = DeploymentManifest::from_json(include_str!(
            "../../../docs/deployments/arc-testnet.json"
        ))
        .expect("manifest parses");
        let filters = TangentEventKind::filters_for_manifest(&manifest);

        assert_eq!(filters.len(), 9);
        assert!(filters.contains(
            &TangentEventKind::AccountRegistered.filter(manifest.contracts.account_manager)
        ));
        assert!(
            filters.contains(&TangentEventKind::Deposited.filter(manifest.contracts.usdc_vault))
        );
        assert!(filters
            .contains(&TangentEventKind::MarketPaused.filter(manifest.contracts.market_registry)));
        assert!(!filters.iter().any(|filter| matches!(
            filter.kind,
            TangentEventKind::OrderSubmitted
                | TangentEventKind::Settled
                | TangentEventKind::Liquidated
        )));

        let registered_log = RawLog::new(
            manifest.contracts.account_manager,
            vec![AccountRegisteredEvent::topic0()],
            vec![],
        );
        let registered_filter =
            TangentEventKind::AccountRegistered.filter(manifest.contracts.account_manager);
        assert!(registered_filter.matches_log(&registered_log));
        assert_eq!(
            registered_filter.signature,
            AccountRegisteredEvent::SIGNATURE
        );
        assert_eq!(registered_filter.topic0, AccountRegisteredEvent::topic0());

        let filter_set = EventFilterSet::from_manifest(&manifest);
        let request = filter_set.to_request();
        let query = filter_set.to_query(Some(100), Some(200));
        assert_eq!(filter_set.len(), 9);
        assert!(!filter_set.is_empty());
        assert_eq!(request.addresses.len(), 3);
        assert_eq!(request.topic0.len(), 9);
        assert_eq!(query.filter, request);
        assert_eq!(query.from_block, Some(100));
        assert_eq!(query.to_block, Some(200));
        let rpc_request = request.to_rpc_query();
        let rpc_query = query.to_rpc_query();
        assert_eq!(rpc_request.addresses, request.addresses);
        assert_eq!(rpc_request.topics, vec![request.topic0.clone()]);
        assert_eq!(rpc_request.from_block, None);
        assert_eq!(rpc_request.to_block, None);
        assert_eq!(rpc_query.addresses, request.addresses);
        assert_eq!(rpc_query.topics, vec![request.topic0.clone()]);
        assert_eq!(rpc_query.from_block, Some("0x64".to_owned()));
        assert_eq!(rpc_query.to_block, Some("0xc8".to_owned()));
        let rpc_json = serde_json::to_string(&rpc_query).expect("serialize rpc query");
        assert!(rpc_json.contains("\"address\""));
        assert!(rpc_json.contains("\"topics\""));
        assert!(rpc_json.contains("\"fromBlock\":\"0x64\""));
        assert!(rpc_json.contains("\"toBlock\":\"0xc8\""));
        assert!(!rpc_json.contains("\"addresses\""));
        assert!(!query.is_open_ended());
        let query_summary = query.summary();
        assert_eq!(query_summary.address_count, 3);
        assert_eq!(query_summary.topic0_count, 9);
        assert_eq!(query_summary.from_block, Some(100));
        assert_eq!(query_summary.to_block, Some(200));
        assert_eq!(query_summary.block_span, Some(101));
        assert!(!query_summary.is_open_ended);
        assert!(!query_summary.has_invalid_range);
        let rpc_query_summary = rpc_query.summary();
        assert_eq!(rpc_query_summary.address_count, 3);
        assert_eq!(rpc_query_summary.topic0_count, 9);
        assert_eq!(rpc_query_summary.from_block.as_deref(), Some("0x64"));
        assert_eq!(rpc_query_summary.to_block.as_deref(), Some("0xc8"));
        assert!(!rpc_query_summary.is_open_ended);
        let chunks = query.chunked(50).expect("chunk query");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].filter, request);
        assert_eq!(chunks[0].from_block, Some(100));
        assert_eq!(chunks[0].to_block, Some(149));
        assert_eq!(chunks[1].from_block, Some(150));
        assert_eq!(chunks[1].to_block, Some(199));
        assert_eq!(chunks[2].from_block, Some(200));
        assert_eq!(chunks[2].to_block, Some(200));
        let chunk_summary = EventLogQuery::summarize_batch(&chunks);
        assert_eq!(chunk_summary.len, 3);
        assert!(!chunk_summary.is_empty);
        assert_eq!(chunk_summary.open_ended_queries, 0);
        assert_eq!(chunk_summary.invalid_range_queries, 0);
        assert_eq!(chunk_summary.total_block_span, Some(101));
        assert_eq!(chunk_summary.total_address_filters, 9);
        assert_eq!(chunk_summary.total_topic0_filters, 27);
        assert_eq!(chunk_summary.queries[0].block_span, Some(50));
        let chunk_summary_json =
            serde_json::to_string(&chunk_summary).expect("serialize chunk summary");
        let restored_chunk_summary: EventLogQueryBatchSummary =
            serde_json::from_str(&chunk_summary_json).expect("deserialize chunk summary");
        assert_eq!(restored_chunk_summary, chunk_summary);
        let rpc_chunks = query.chunked_rpc(50).expect("chunk rpc query");
        assert_eq!(rpc_chunks.len(), 3);
        assert_eq!(rpc_chunks[0].addresses, request.addresses);
        assert_eq!(rpc_chunks[0].topics, vec![request.topic0.clone()]);
        assert_eq!(rpc_chunks[0].from_block, Some("0x64".to_owned()));
        assert_eq!(rpc_chunks[0].to_block, Some("0x95".to_owned()));
        assert_eq!(rpc_chunks[1].from_block, Some("0x96".to_owned()));
        assert_eq!(rpc_chunks[1].to_block, Some("0xc7".to_owned()));
        assert_eq!(rpc_chunks[2].from_block, Some("0xc8".to_owned()));
        assert_eq!(rpc_chunks[2].to_block, Some("0xc8".to_owned()));
        let rpc_chunk_summary = EventLogRpcQuery::summarize_batch(&rpc_chunks);
        assert_eq!(rpc_chunk_summary.len, 3);
        assert_eq!(rpc_chunk_summary.open_ended_queries, 0);
        assert_eq!(rpc_chunk_summary.total_address_filters, 9);
        assert_eq!(rpc_chunk_summary.total_topic0_filters, 27);
        assert_eq!(
            rpc_chunk_summary.queries[0].from_block.as_deref(),
            Some("0x64")
        );
        let rpc_chunk_summary_json =
            serde_json::to_string(&rpc_chunk_summary).expect("serialize rpc chunk summary");
        let restored_rpc_chunk_summary: EventLogRpcQueryBatchSummary =
            serde_json::from_str(&rpc_chunk_summary_json).expect("deserialize rpc chunk summary");
        assert_eq!(restored_rpc_chunk_summary, rpc_chunk_summary);
        assert!(serde_json::to_string(&query)
            .expect("serialize")
            .contains("\"from_block\":100"));
        assert!(request
            .addresses
            .contains(&manifest.contracts.account_manager));
        assert!(request.topic0.contains(&AccountRegisteredEvent::topic0()));
        assert_eq!(
            filter_set.matching_filter(&registered_log),
            Some(registered_filter)
        );

        let broad_request_false_positive = RawLog::new(
            manifest.contracts.account_manager,
            vec![DepositedEvent::topic0()],
            vec![],
        );
        assert!(request
            .addresses
            .contains(&broad_request_false_positive.address));
        assert!(request
            .topic0
            .contains(&broad_request_false_positive.topic0().expect("topic0")));
        assert!(!filter_set.matches_log(&broad_request_false_positive));

        let exact_registered_log = RawLog::new(
            manifest.contracts.account_manager,
            vec![
                AccountRegisteredEvent::topic0(),
                topic_u128(7),
                topic_address(manifest.deployer),
            ],
            word(123).to_vec(),
        );
        let exact_registered_log_from_hex = RawLog::from_hex_data(
            manifest.contracts.account_manager,
            vec![
                AccountRegisteredEvent::topic0(),
                topic_u128(7),
                topic_address(manifest.deployer),
            ],
            exact_registered_log.data_hex(),
        )
        .expect("hex log data");
        assert_eq!(exact_registered_log_from_hex, exact_registered_log);
        let metadata = RawLogMetadata::new(Some(123), Some(B256::repeat_byte(0xab)), Some(9));
        let exact_registered_log_with_metadata =
            exact_registered_log.clone().with_metadata(metadata);
        assert_eq!(exact_registered_log_with_metadata.metadata, Some(metadata));
        assert_eq!(
            exact_registered_log_with_metadata.cursor(),
            Some(RawLogCursor::new(123, 9))
        );
        assert_eq!(metadata.cursor(), Some(RawLogCursor::new(123, 9)));
        assert_eq!(
            RawLogCursor::from_metadata(&metadata),
            Some(RawLogCursor::new(123, 9))
        );
        let cursor = RawLogCursor::from_log(&exact_registered_log_with_metadata)
            .expect("log cursor from metadata");
        assert_eq!(cursor.resume_from_block(), 123);
        assert_eq!(cursor.next_block(), 124);
        assert!(cursor > RawLogCursor::new(122, u64::MAX));
        assert!(cursor < RawLogCursor::new(123, 10));
        let resume_query = filter_set.resume_query(cursor, Some(200));
        assert_eq!(resume_query.from_block, Some(123));
        assert_eq!(resume_query.to_block, Some(200));
        let resume_rpc_query = filter_set.resume_rpc_query(cursor, Some(200));
        assert_eq!(resume_rpc_query.from_block, Some("0x7b".to_owned()));
        assert_eq!(resume_rpc_query.to_block, Some("0xc8".to_owned()));
        assert_eq!(
            exact_registered_log_with_metadata
                .clone()
                .without_metadata(),
            exact_registered_log
        );
        assert!(!metadata.is_empty());
        assert!(RawLogMetadata::new(None, None, None).is_empty());
        assert_eq!(
            RawLogMetadata::new(Some(123), Some(B256::repeat_byte(0xab)), None).cursor(),
            None
        );
        assert_eq!(RawLogCursor::new(u64::MAX, 0).next_block(), u64::MAX);
        assert_eq!(
            RawLog::from_hex_data_with_metadata(
                manifest.contracts.account_manager,
                vec![
                    AccountRegisteredEvent::topic0(),
                    topic_u128(7),
                    topic_address(manifest.deployer),
                ],
                exact_registered_log.data_hex(),
                metadata,
            )
            .expect("hex log data with metadata"),
            exact_registered_log_with_metadata
        );
        let old_registered_log = exact_registered_log
            .clone()
            .with_metadata(RawLogMetadata::new(
                Some(123),
                Some(B256::repeat_byte(0xab)),
                Some(8),
            ));
        let new_registered_log = exact_registered_log
            .clone()
            .with_metadata(RawLogMetadata::new(
                Some(123),
                Some(B256::repeat_byte(0xab)),
                Some(10),
            ));
        let decoded_after_cursor = filter_set
            .decode_logs_after_cursor(
                [
                    &old_registered_log,
                    &new_registered_log,
                    &broad_request_false_positive,
                ],
                cursor,
            )
            .expect("decode logs after cursor");
        assert_eq!(decoded_after_cursor.known_logs(), 1);
        assert_eq!(decoded_after_cursor.unknown_logs, 1);
        let decoded_records_after_cursor = filter_set
            .decode_log_records_after_cursor(
                [
                    &old_registered_log,
                    &new_registered_log,
                    &broad_request_false_positive,
                ],
                cursor,
            )
            .expect("decode log records after cursor");
        assert_eq!(decoded_records_after_cursor.known_logs(), 1);
        assert_eq!(decoded_records_after_cursor.unknown_logs, 1);
        assert_eq!(
            decoded_records_after_cursor.last_cursor(),
            Some(RawLogCursor::new(123, 10))
        );
        assert_eq!(
            decoded_records_after_cursor.records[0].cursor(),
            Some(RawLogCursor::new(123, 10))
        );
        assert_eq!(
            decoded_records_after_cursor.records[0].transaction_hash(),
            Some(B256::repeat_byte(0xab))
        );
        assert_eq!(
            decoded_records_after_cursor.records[0].kind(),
            TangentEventKind::AccountRegistered
        );
        assert_eq!(
            decoded_records_after_cursor.count_kind(TangentEventKind::AccountRegistered),
            1
        );
        assert!(decoded_records_after_cursor.contains_kind(TangentEventKind::AccountRegistered));
        assert_eq!(
            decoded_records_after_cursor.nonzero_kind_counts(),
            vec![TangentEventKindCount {
                kind: TangentEventKind::AccountRegistered,
                count: 1,
            }]
        );
        let records_summary = decoded_records_after_cursor.summary();
        assert_eq!(records_summary.known_logs, 1);
        assert_eq!(records_summary.unknown_logs, 1);
        assert_eq!(records_summary.total_logs, 2);
        assert!(!records_summary.is_empty);
        assert!(records_summary.has_known_logs);
        assert!(records_summary.has_unknown_logs);
        assert_eq!(records_summary.records_with_cursor, 1);
        assert_eq!(records_summary.records_without_cursor, 0);
        assert!(records_summary.has_cursor);
        assert!(records_summary.all_known_logs_have_cursor);
        assert_eq!(
            records_summary.last_cursor,
            Some(RawLogCursor::new(123, 10))
        );
        assert_eq!(
            records_summary.nonzero_kind_counts,
            decoded_records_after_cursor.nonzero_kind_counts()
        );
        let records_summary_json =
            serde_json::to_string(&records_summary).expect("record summary serializes");
        assert!(records_summary_json.contains("\"has_cursor\":true"));
        assert!(records_summary_json.contains("\"all_known_logs_have_cursor\":true"));
        let restored_records_summary: DecodedTangentLogRecordsSummary =
            serde_json::from_str(&records_summary_json).expect("record summary deserializes");
        assert_eq!(restored_records_summary, records_summary);
        let mut legacy_records_summary_json =
            serde_json::to_value(&records_summary).expect("record summary value");
        let legacy_records_summary_object = legacy_records_summary_json
            .as_object_mut()
            .expect("record summary object");
        legacy_records_summary_object.remove("has_known_logs");
        legacy_records_summary_object.remove("has_unknown_logs");
        legacy_records_summary_object.remove("has_cursor");
        legacy_records_summary_object.remove("all_known_logs_have_cursor");
        let legacy_records_summary: DecodedTangentLogRecordsSummary =
            serde_json::from_value(legacy_records_summary_json)
                .expect("legacy record summary deserializes");
        assert!(!legacy_records_summary.has_known_logs);
        assert!(!legacy_records_summary.has_unknown_logs);
        assert!(!legacy_records_summary.has_cursor);
        assert!(!legacy_records_summary.all_known_logs_have_cursor);
        assert_eq!(
            decoded_records_after_cursor
                .clone()
                .into_decoded_logs()
                .known_logs(),
            decoded_after_cursor.known_logs()
        );
        assert_eq!(
            RawLog::from_hex_data(Address::ZERO, vec![], "0X1234")
                .expect("upper prefix")
                .data_hex(),
            "0x1234"
        );
        assert!(matches!(
            RawLog::from_hex_data(Address::ZERO, vec![], "0x123").expect_err("bad hex"),
            RawLogError::InvalidDataHex(_)
        ));
        let serialized_log = serde_json::to_string(&exact_registered_log).expect("serialize log");
        assert!(serialized_log.contains("\"data\":\"0x"));
        assert!(!serialized_log.contains("\"metadata\""));
        assert_eq!(
            serde_json::from_str::<RawLog>(&serialized_log).expect("deserialize log"),
            exact_registered_log
        );
        let serialized_log_with_metadata =
            serde_json::to_string(&exact_registered_log_with_metadata)
                .expect("serialize log metadata");
        assert!(serialized_log_with_metadata.contains("\"metadata\""));
        assert_eq!(
            serde_json::from_str::<RawLog>(&serialized_log_with_metadata)
                .expect("deserialize log metadata"),
            exact_registered_log_with_metadata
        );
        let wrong_address_log = RawLog::new(
            Address::ZERO,
            exact_registered_log.topics.clone(),
            exact_registered_log.data.clone(),
        );
        assert_eq!(
            filter_set
                .decode_logs([
                    &exact_registered_log,
                    &broad_request_false_positive,
                    &wrong_address_log,
                ])
                .expect("exact filtered batch"),
            DecodedTangentLogs {
                events: vec![TangentEvent::AccountRegistered(AccountRegisteredEvent {
                    account_id: 7,
                    owner: manifest.deployer,
                    registered_at: 123,
                })],
                unknown_logs: 2,
            }
        );

        assert_eq!(
            filter_set
                .decode_logs([&registered_log])
                .expect_err("malformed exact log"),
            EventDecodeError::InvalidTopicCount {
                expected: 3,
                actual: 1,
            }
        );
    }

    #[test]
    fn full_stack_manifest_filters_include_perp_events() {
        let manifest = full_stack_manifest();
        let filters = TangentEventKind::filters_for_manifest(&manifest);
        let filter_set = EventFilterSet::from_manifest(&manifest);
        let request = filter_set.to_request();
        let open_ended_query = filter_set.to_query(Some(500), None);

        assert_eq!(filters.len(), 14);
        assert_eq!(filter_set.len(), 14);
        assert_eq!(request.addresses.len(), 6);
        assert_eq!(request.topic0.len(), 14);
        assert_eq!(open_ended_query.from_block, Some(500));
        assert_eq!(open_ended_query.to_block, None);
        assert!(open_ended_query.is_open_ended());
        let open_ended_summary = open_ended_query.summary();
        assert!(open_ended_summary.is_open_ended);
        assert_eq!(open_ended_summary.block_span, None);
        assert!(!open_ended_summary.has_invalid_range);
        assert_eq!(
            open_ended_query
                .chunked(100)
                .expect_err("open-ended chunks need an end block"),
            EventQueryError::OpenEndedRange
        );
        let invalid_range_query = filter_set.to_query(Some(200), Some(100));
        let invalid_range_summary = invalid_range_query.summary();
        assert!(!invalid_range_summary.is_open_ended);
        assert_eq!(invalid_range_summary.block_span, None);
        assert!(invalid_range_summary.has_invalid_range);
        assert_eq!(
            invalid_range_query.chunked(100).expect_err("invalid range"),
            EventQueryError::InvalidRange {
                from_block: 200,
                to_block: 100,
            }
        );
        let mixed_summary =
            EventLogQuery::summarize_batch(&[open_ended_query.clone(), invalid_range_query]);
        assert_eq!(mixed_summary.len, 2);
        assert_eq!(mixed_summary.open_ended_queries, 1);
        assert_eq!(mixed_summary.invalid_range_queries, 1);
        assert_eq!(mixed_summary.total_block_span, None);
        assert_eq!(
            filter_set
                .to_query(Some(100), Some(200))
                .chunked(0)
                .expect_err("zero chunk size"),
            EventQueryError::ZeroChunkSize
        );
        let chunked_queries = filter_set
            .chunked_queries(100, 105, 2)
            .expect("chunked filter queries");
        assert_eq!(chunked_queries.len(), 3);
        assert_eq!(chunked_queries[0].filter, request);
        assert_eq!(chunked_queries[0].from_block, Some(100));
        assert_eq!(chunked_queries[0].to_block, Some(101));
        assert_eq!(chunked_queries[1].from_block, Some(102));
        assert_eq!(chunked_queries[1].to_block, Some(103));
        assert_eq!(chunked_queries[2].from_block, Some(104));
        assert_eq!(chunked_queries[2].to_block, Some(105));
        let chunked_rpc_queries = filter_set
            .chunked_rpc_queries(100, 105, 2)
            .expect("chunked rpc filter queries");
        assert_eq!(chunked_rpc_queries.len(), 3);
        assert_eq!(chunked_rpc_queries[0].addresses, request.addresses);
        assert_eq!(chunked_rpc_queries[0].topics, vec![request.topic0.clone()]);
        assert_eq!(
            chunked_rpc_queries
                .iter()
                .map(|query| (query.from_block.as_deref(), query.to_block.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                (Some("0x64"), Some("0x65")),
                (Some("0x66"), Some("0x67")),
                (Some("0x68"), Some("0x69")),
            ]
        );
        assert!(filters.contains(
            &TangentEventKind::OrderSubmitted
                .filter(manifest.contracts.order_book.expect("order book"))
        ));
        assert!(filters.contains(
            &TangentEventKind::Settled
                .filter(manifest.contracts.settlement_engine.expect("settlement"))
        ));
        assert!(filters.contains(
            &TangentEventKind::Liquidated
                .filter(manifest.contracts.liquidation_keeper.expect("liquidation"))
        ));
        assert_eq!(
            TangentEventKind::OrderCancelled.signature(),
            OrderCancelledEvent::SIGNATURE
        );
        assert_eq!(
            TangentEventKind::OrderCancelled.topic0(),
            OrderCancelledEvent::topic0()
        );
    }

    #[test]
    fn decodes_account_registered_log() {
        let owner = Address::repeat_byte(0x11);
        let log = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![
                AccountRegisteredEvent::topic0(),
                topic_u128(7),
                topic_address(owner),
            ],
            word(123).to_vec(),
        );

        assert_eq!(
            AccountRegisteredEvent::decode(&log).expect("decode"),
            AccountRegisteredEvent {
                account_id: 7,
                owner,
                registered_at: 123,
            }
        );
        assert_eq!(log.topic0(), Some(AccountRegisteredEvent::topic0()));
        assert_eq!(
            TangentEvent::decode_known(&log).expect("known event"),
            Some(TangentEvent::AccountRegistered(AccountRegisteredEvent {
                account_id: 7,
                owner,
                registered_at: 123,
            }))
        );
    }

    #[test]
    fn decodes_collateral_logs() {
        let from = Address::repeat_byte(0x22);
        let deposit = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![DepositedEvent::topic0(), topic_u128(7), topic_address(from)],
            word(100).to_vec(),
        );
        assert_eq!(
            DepositedEvent::decode(&deposit).expect("deposit"),
            DepositedEvent {
                account_id: 7,
                from,
                amount: 100,
            }
        );

        let locked = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![MarginAmountEvent::locked_topic0(), topic_u128(7)],
            word(25).to_vec(),
        );
        assert_eq!(
            MarginAmountEvent::decode_locked(&locked).expect("locked"),
            MarginAmountEvent {
                account_id: 7,
                amount: 25,
            }
        );

        let pnl = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![PnlAppliedEvent::topic0(), topic_u128(7)],
            signed_word(-9).to_vec(),
        );
        assert_eq!(
            PnlAppliedEvent::decode(&pnl).expect("pnl"),
            PnlAppliedEvent {
                account_id: 7,
                pnl: -9,
            }
        );
    }

    #[test]
    fn decodes_order_and_settlement_logs() {
        let order_hash = B256::repeat_byte(0x33);
        let buy_hash = B256::repeat_byte(0x44);
        let sell_hash = B256::repeat_byte(0x55);

        let mut submitted_data = Vec::new();
        submitted_data.extend_from_slice(&word(1));
        submitted_data.extend_from_slice(&word(65_000));
        submitted_data.extend_from_slice(&word(10));
        let submitted = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![
                OrderSubmittedEvent::topic0(),
                order_hash,
                topic_u128(7),
                topic_u128(1),
            ],
            submitted_data,
        );
        assert_eq!(
            OrderSubmittedEvent::decode(&submitted).expect("submitted"),
            OrderSubmittedEvent {
                order_hash,
                account_id: 7,
                market_id: 1,
                is_buy: true,
                limit_price: 65_000,
                size: 10,
            }
        );

        let cancelled = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![OrderCancelledEvent::topic0(), order_hash, topic_u128(7)],
            encoded_string("owner", 1),
        );
        assert_eq!(
            OrderCancelledEvent::decode(&cancelled).expect("cancelled"),
            OrderCancelledEvent {
                order_hash,
                account_id: 7,
                reason: "owner".to_owned(),
            }
        );

        let mut fill_data = Vec::new();
        fill_data.extend_from_slice(&word(10));
        fill_data.extend_from_slice(&word(65_000));
        let matched = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![MatchedEvent::topic0(), buy_hash, sell_hash, topic_u128(1)],
            fill_data.clone(),
        );
        assert_eq!(
            MatchedEvent::decode(&matched).expect("matched"),
            MatchedEvent {
                buy_order_hash: buy_hash,
                sell_order_hash: sell_hash,
                market_id: 1,
                size: 10,
                price: 65_000,
            }
        );

        let settled = RawLog::new(
            Address::repeat_byte(0xbb),
            vec![SettledEvent::topic0(), buy_hash, sell_hash, topic_u128(1)],
            fill_data,
        );
        assert_eq!(
            SettledEvent::decode(&settled).expect("settled"),
            SettledEvent {
                buy_order_hash: buy_hash,
                sell_order_hash: sell_hash,
                market_id: 1,
                size: 10,
                price: 65_000,
            }
        );
    }

    #[test]
    fn decodes_market_registry_logs() {
        let price_feed = Address::repeat_byte(0x77);
        let registered = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![MarketRegisteredEvent::topic0(), topic_u128(1)],
            encoded_string_and_address("BTC", price_feed),
        );
        assert_eq!(
            MarketRegisteredEvent::decode(&registered).expect("registered"),
            MarketRegisteredEvent {
                market_id: 1,
                symbol: "BTC".to_owned(),
                price_feed,
            }
        );

        let updated = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![MarketParamsUpdatedEvent::topic0(), topic_u128(1)],
            vec![],
        );
        assert_eq!(
            MarketParamsUpdatedEvent::decode(&updated).expect("updated"),
            MarketParamsUpdatedEvent { market_id: 1 }
        );

        let paused = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![MarketPausedEvent::topic0(), topic_u128(1)],
            word(1).to_vec(),
        );
        assert_eq!(
            MarketPausedEvent::decode(&paused).expect("paused"),
            MarketPausedEvent {
                market_id: 1,
                paused: true,
            }
        );
        assert_eq!(
            TangentEvent::decode_known(&paused).expect("known event"),
            Some(TangentEvent::MarketPaused(MarketPausedEvent {
                market_id: 1,
                paused: true,
            }))
        );
    }

    #[test]
    fn known_event_dispatcher_skips_unknown_and_rejects_malformed_known_logs() {
        let empty = RawLog::new(Address::ZERO, vec![], vec![]);
        assert_eq!(empty.topic0(), None);
        assert_eq!(TangentEvent::decode_known(&empty).expect("empty"), None);

        let unknown = RawLog::new(Address::ZERO, vec![B256::repeat_byte(0xee)], vec![]);
        assert_eq!(TangentEvent::decode_known(&unknown).expect("unknown"), None);

        let malformed = RawLog::new(
            Address::ZERO,
            vec![DepositedEvent::topic0(), topic_u128(7)],
            word(100).to_vec(),
        );
        assert_eq!(
            TangentEvent::decode_known(&malformed).expect_err("malformed known event"),
            EventDecodeError::InvalidTopicCount {
                expected: 3,
                actual: 2,
            }
        );
    }

    #[test]
    fn decodes_mixed_log_batches_and_counts_unknown_logs() {
        let deposit = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![
                DepositedEvent::topic0(),
                topic_u128(7),
                topic_address(Address::repeat_byte(0x22)),
            ],
            word(100).to_vec(),
        );
        let unknown = RawLog::new(Address::ZERO, vec![B256::repeat_byte(0xee)], vec![]);
        let paused = RawLog::new(
            Address::repeat_byte(0xbb),
            vec![MarketPausedEvent::topic0(), topic_u128(1)],
            word(1).to_vec(),
        );

        let decoded = TangentEvent::decode_logs([&deposit, &unknown, &paused]).expect("batch");
        assert_eq!(
            decoded,
            DecodedTangentLogs {
                events: vec![
                    TangentEvent::Deposited(DepositedEvent {
                        account_id: 7,
                        from: Address::repeat_byte(0x22),
                        amount: 100,
                    }),
                    TangentEvent::MarketPaused(MarketPausedEvent {
                        market_id: 1,
                        paused: true,
                    }),
                ],
                unknown_logs: 1,
            }
        );
        assert_eq!(TangentEventKind::ALL.len(), 14);
        assert_eq!(decoded.known_logs(), 2);
        assert_eq!(decoded.total_logs(), 3);
        assert!(!decoded.is_empty());
        assert_eq!(decoded.events[0].kind(), TangentEventKind::Deposited);
        assert_eq!(decoded.count_kind(TangentEventKind::Deposited), 1);
        assert_eq!(decoded.count_kind(TangentEventKind::MarketPaused), 1);
        assert_eq!(decoded.count_kind(TangentEventKind::Liquidated), 0);
        assert!(decoded.contains_kind(TangentEventKind::Deposited));
        assert!(!decoded.contains_kind(TangentEventKind::Liquidated));
        let kind_counts = decoded.kind_counts();
        assert_eq!(kind_counts.len(), TangentEventKind::ALL.len());
        assert_eq!(
            kind_counts[0],
            TangentEventKindCount {
                kind: TangentEventKind::AccountRegistered,
                count: 0,
            }
        );
        assert_eq!(
            kind_counts[1],
            TangentEventKindCount {
                kind: TangentEventKind::Deposited,
                count: 1,
            }
        );
        assert_eq!(
            decoded.nonzero_kind_counts(),
            vec![
                TangentEventKindCount {
                    kind: TangentEventKind::Deposited,
                    count: 1,
                },
                TangentEventKindCount {
                    kind: TangentEventKind::MarketPaused,
                    count: 1,
                },
            ]
        );
        assert!(serde_json::to_string(&decoded.nonzero_kind_counts())
            .expect("serialize counts")
            .contains("\"count\":1"));
        let summary = decoded.summary();
        assert_eq!(summary.known_logs, 2);
        assert_eq!(summary.unknown_logs, 1);
        assert_eq!(summary.total_logs, 3);
        assert!(!summary.is_empty);
        assert!(summary.has_known_logs);
        assert!(summary.has_unknown_logs);
        assert_eq!(summary.kind_counts.len(), TangentEventKind::ALL.len());
        assert_eq!(summary.nonzero_kind_counts, decoded.nonzero_kind_counts());
        let summary_json = serde_json::to_string(&summary).expect("summary serializes");
        assert!(summary_json.contains("\"has_known_logs\":true"));
        assert!(summary_json.contains("\"has_unknown_logs\":true"));
        let restored_summary: DecodedTangentLogsSummary =
            serde_json::from_str(&summary_json).expect("summary deserializes");
        assert_eq!(restored_summary, summary);
        let mut legacy_summary_json = serde_json::to_value(&summary).expect("summary value");
        let legacy_summary_object = legacy_summary_json.as_object_mut().expect("summary object");
        legacy_summary_object.remove("has_known_logs");
        legacy_summary_object.remove("has_unknown_logs");
        let legacy_summary: DecodedTangentLogsSummary =
            serde_json::from_value(legacy_summary_json).expect("legacy summary deserializes");
        assert!(!legacy_summary.has_known_logs);
        assert!(!legacy_summary.has_unknown_logs);
        assert!(DecodedTangentLogs {
            events: vec![],
            unknown_logs: 0,
        }
        .is_empty());

        let malformed = RawLog::new(
            Address::ZERO,
            vec![DepositedEvent::topic0(), topic_u128(7)],
            word(100).to_vec(),
        );
        assert_eq!(
            TangentEvent::decode_logs([&unknown, &malformed]).expect_err("bad known log"),
            EventDecodeError::InvalidTopicCount {
                expected: 3,
                actual: 2,
            }
        );
    }

    #[test]
    fn decodes_liquidation_log_and_rejects_bad_topics() {
        let liquidator = Address::repeat_byte(0x66);
        let mut data = Vec::new();
        data.extend_from_slice(&word(64_000));
        data.extend_from_slice(&signed_word(-10));
        let log = RawLog::new(
            Address::repeat_byte(0xaa),
            vec![
                LiquidatedEvent::topic0(),
                topic_u128(7),
                topic_u128(1),
                topic_address(liquidator),
            ],
            data,
        );

        assert_eq!(
            LiquidatedEvent::decode(&log).expect("liquidated"),
            LiquidatedEvent {
                account_id: 7,
                market_id: 1,
                liquidator,
                mark_price: 64_000,
                pnl: -10,
            }
        );

        let wrong_topic = RawLog::new(Address::ZERO, vec![DepositedEvent::topic0()], vec![]);
        assert_eq!(
            LiquidatedEvent::decode(&wrong_topic).expect_err("wrong topic count"),
            EventDecodeError::InvalidTopicCount {
                expected: 4,
                actual: 1,
            }
        );

        let wrong_event = RawLog::new(
            Address::ZERO,
            vec![
                DepositedEvent::topic0(),
                topic_u128(7),
                topic_u128(1),
                topic_address(liquidator),
            ],
            vec![0u8; 64],
        );
        assert!(matches!(
            LiquidatedEvent::decode(&wrong_event).expect_err("wrong topic"),
            EventDecodeError::UnexpectedTopic { .. }
        ));
    }
}
