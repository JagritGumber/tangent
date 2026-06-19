//! Transport-neutral client configuration.
//!
//! This module does not open sockets or pick an HTTP/WebSocket implementation.
//! It packages the manifest-bound SDK context with endpoint and policy values
//! that a future `TangentClient` or keeper daemon can hand to its own transport.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    AbiDecodeError, AccountMarketProjectionKey, AccountStatus, AccountStatusPlan, CallReturn,
    CallReturnBatch, CollateralStatus, CollateralStatusPlan, DecodedTangentLogRecords,
    DecodedTangentLogs, DeploymentManifest, EventDecodeError, EventLogRpcQuery,
    EventLogRpcQueryBatchSummary, EventLogRpcQuerySummary, EventQueryError, ExternalSignerAdapter,
    JsonRpcBackoffPolicy, JsonRpcExecutor, JsonRpcExecutorError, JsonRpcRetryPolicy,
    JsonRpcTransport, KeeperCapability, KeeperPollingOutcome, KeeperPollingPlan,
    KeeperPollingPlanSummary, KeeperPollingPolicy, KeeperPollingSnapshot, KeeperRuntimePlan,
    LiquidationDecodeError, LiquidationReadPlan, LiquidationStatus, MarketReadPlan,
    MarketReadSummary, OrderError, OrderLifecyclePlan, OrderLifecycleStatus, OrderParams,
    OrderPlacement, OrderPlacementPlan, OrderPlacementSignError, OrderSigner,
    PerpStackAvailability, PreparedOrder, RawLog, RawLogCursor, RawTransactionSigner,
    SettlementReadPlan, SettlementStatus, SignedOrder, SignerBackendConfig,
    SignerBackendConfigError, SignerBackendKind, TangentContext, TangentContextError,
    TangentEventProjection, TangentEventProjectionError, TangentEventProjectionSummary,
    TxConfirmationBatchReport, TxConfirmationBatchSnapshot, TxConfirmationPolicy,
    TxConfirmationSnapshot, TxConfirmationSnapshotReport, TxFeePolicy, TxHash, TxSubmissionPlan,
    TxSubmissionPlanBatchSummary, TxSubmissionPlanSummary, TxWorkflowBatchResumePlan,
    TxWorkflowBatchResumePlanSummary, TxWorkflowBatchSubmission, TxWorkflowBatchSubmissionReport,
    TxWorkflowError, TxWorkflowExecutor, TxWorkflowSubmission, TxWorkflowSubmissionReport,
    UnsignedCall, UnsignedCallBatchSummary, UnsignedTx,
};

/// RPC endpoint plus optional static headers for a caller-owned transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcEndpointConfig {
    pub url: String,
    #[serde(default)]
    pub headers: Vec<RpcHeader>,
}

/// One static RPC header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcHeader {
    pub name: String,
    pub value: String,
}

/// Policy bundle shared by client and keeper workflows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentClientPolicies {
    pub retry: JsonRpcRetryPolicy,
    pub backoff: JsonRpcBackoffPolicy,
    pub fee: TxFeePolicy,
    pub confirmation: TxConfirmationPolicy,
    pub keeper_polling: KeeperPollingPolicy,
}

/// Manifest-bound client configuration plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentClientConfig {
    pub endpoint: RpcEndpointConfig,
    pub chain_id: u64,
    pub policies: TangentClientPolicies,
    #[serde(default)]
    pub signer_backends: Vec<SignerBackendConfig>,
}

/// Secret-free report for client configuration logs and support bundles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentClientConfigReport {
    pub chain_id: u64,
    pub endpoint: RpcEndpointConfigReport,
    pub policies: TangentClientPolicies,
    pub signer_backend_count: usize,
    #[serde(default)]
    pub has_signer_backends: bool,
    pub signer_backend_kinds: Vec<SignerBackendKind>,
    #[serde(default)]
    pub has_multiple_signer_backend_kinds: bool,
    pub signer_backends: Vec<SignerBackendConfigReport>,
}

/// Secret-free RPC endpoint configuration summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcEndpointConfigReport {
    pub scheme: Option<String>,
    pub is_secure: bool,
    pub static_rpc_headers: usize,
    pub static_rpc_header_names: Vec<String>,
    #[serde(default)]
    pub static_rpc_auth_headers: usize,
    #[serde(default)]
    pub has_static_rpc_auth_header: bool,
}

/// Secret-free signer backend configuration summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignerBackendConfigReport {
    pub kind: SignerBackendKind,
    pub key_id: String,
    pub address: Option<alloy_primitives::Address>,
    pub metadata_keys: Vec<String>,
    pub secret_metadata_keys: Vec<String>,
    pub metadata_count: usize,
    pub secret_metadata_count: usize,
    #[serde(default)]
    pub has_address: bool,
    #[serde(default)]
    pub has_metadata: bool,
    #[serde(default)]
    pub has_secret_metadata: bool,
}

/// Parsed manifest plus transport-neutral client configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentClientPlan {
    pub context: TangentContext,
    pub config: TangentClientConfig,
}

/// Secret-free support bundle for manifest/config/startup diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentClientSupportReport {
    pub startup: TangentClientStartupReport,
    pub config: TangentClientConfigReport,
}

/// Serializable startup summary for a manifest-bound client or keeper runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentClientStartupReport {
    pub project: String,
    pub version: String,
    pub network: String,
    pub manifest_chain_id: u64,
    pub configured_chain_id: u64,
    pub chain_id_matches_manifest: bool,
    pub endpoint_scheme: Option<String>,
    pub endpoint_is_secure: bool,
    pub static_rpc_headers: usize,
    #[serde(default)]
    pub static_rpc_auth_headers: usize,
    #[serde(default)]
    pub has_static_rpc_auth_header: bool,
    pub signer_backend_count: usize,
    #[serde(default)]
    pub has_signer_backends: bool,
    pub signer_backend_kinds: Vec<SignerBackendKind>,
    #[serde(default)]
    pub has_multiple_signer_backend_kinds: bool,
    pub perp_stack: PerpStackAvailability,
    pub missing_perp_contracts: Vec<String>,
    pub keeper_capabilities: Vec<KeeperCapability>,
    pub readiness: TangentClientStartupReadiness,
    pub policies: TangentClientPolicies,
}

/// Explicit startup readiness gates for client, keeper, and fork reference UIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentClientStartupReadiness {
    pub primitive_reads: bool,
    pub orderbook_workflows: bool,
    pub settlement_reads: bool,
    pub liquidation_reads: bool,
    pub full_perp_stack: bool,
    pub keeper_polling: bool,
    pub blocking_reasons: Vec<String>,
}

/// Transport-neutral client facade over a caller-provided JSON-RPC transport.
#[derive(Debug, Clone)]
pub struct TangentClient<T> {
    plan: TangentClientPlan,
    rpc: JsonRpcExecutor<T>,
}

/// Client workflow facade over caller-provided JSON-RPC transport and signer.
#[derive(Debug, Clone)]
pub struct TangentClientWorkflow<T, S> {
    plan: TangentClientPlan,
    workflow: TxWorkflowExecutor<T, S>,
}

/// Market-validated order preparation returned by the client read path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentOrderPlacementPreparation {
    pub plan: OrderPlacementPlan,
    pub market: MarketReadSummary,
    pub prepared_order: PreparedOrder,
}

/// Signed order placement plus the raw transaction submission result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TangentOrderPlacementSubmission {
    pub placement: OrderPlacement,
    pub submission: TxWorkflowSubmission,
}

/// Decoded liquidation status plus the submitted permissionless liquidation transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TangentLiquidationSubmission {
    pub status: LiquidationStatus,
    pub submission: TxWorkflowSubmission,
}

/// Readiness plus optional transaction dry-run summary for one liquidation candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentLiquidationDryRun {
    pub candidate: TangentKeeperLiquidationCandidate,
    pub status: LiquidationStatus,
    pub readiness: crate::LiquidationReadiness,
    pub transaction_summary: Option<TxSubmissionPlanBatchSummary>,
}

/// Compact review shape for one liquidation dry-run candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentLiquidationDryRunSummary {
    pub candidate: TangentKeeperLiquidationCandidate,
    pub readiness: crate::LiquidationReadiness,
    pub is_liquidatable: bool,
    pub below_maintenance: bool,
    pub equity: i128,
    pub maintenance_margin: u128,
    #[serde(default)]
    pub transaction_planned: bool,
    #[serde(default)]
    pub has_transaction_summary: bool,
    pub transaction_summary: Option<TxSubmissionPlanBatchSummary>,
}

/// Ordered dry-run report for a liquidation candidate batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentLiquidationDryRunBatch {
    pub candidates: usize,
    pub ready: usize,
    pub blocked: usize,
    pub ready_transaction_summary: TxSubmissionPlanBatchSummary,
    pub reports: Vec<TangentLiquidationDryRun>,
}

/// Compact review shape for a liquidation dry-run batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentLiquidationDryRunBatchSummary {
    pub candidates: usize,
    pub ready: usize,
    pub blocked: usize,
    #[serde(default)]
    pub has_ready: bool,
    #[serde(default)]
    pub has_blocked: bool,
    #[serde(default)]
    pub all_ready: bool,
    pub below_maintenance: usize,
    pub transaction_plans: usize,
    #[serde(default)]
    pub has_transaction_plans: bool,
    pub ready_transaction_summary: TxSubmissionPlanBatchSummary,
    pub reports: Vec<TangentLiquidationDryRunSummary>,
}

/// Account/market pair to evaluate during a keeper liquidation scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TangentKeeperLiquidationCandidate {
    pub account_id: u128,
    pub market_id: u128,
}

/// Decoded scan result for one liquidation candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TangentKeeperLiquidationScanResult {
    pub candidate: TangentKeeperLiquidationCandidate,
    pub status: LiquidationStatus,
    pub submission: Option<TxWorkflowSubmission>,
}

/// Compact serializable result for one liquidation candidate scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentKeeperLiquidationScanReport {
    pub candidate: TangentKeeperLiquidationCandidate,
    pub readiness: crate::LiquidationReadiness,
    #[serde(default)]
    pub has_submission: bool,
    #[serde(default)]
    pub has_submitted_transaction_hash: bool,
    pub submitted_transaction_hash: Option<TxHash>,
    #[serde(default)]
    pub has_submitted_transaction_report: bool,
    pub submitted_transaction_report: Option<TxWorkflowSubmissionReport>,
}

/// Result of executing one caller-managed keeper polling pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TangentKeeperPollingExecution {
    pub plan: KeeperPollingPlan,
    pub event_records: DecodedTangentLogRecords,
    pub events: DecodedTangentLogs,
    pub projection: TangentEventProjection,
    pub derived_liquidation_candidates: Vec<TangentKeeperLiquidationCandidate>,
    pub maintenance_submission: Option<TxWorkflowBatchSubmission>,
    pub liquidation_results: Vec<TangentKeeperLiquidationScanResult>,
    pub outcome: KeeperPollingOutcome,
}

/// Compact serializable report for one executed keeper polling pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentKeeperPollingExecutionReport {
    pub checkpoint: TangentKeeperPollingCheckpoint,
    pub plan_summary: KeeperPollingPlanSummary,
    pub event_records: usize,
    pub decoded_events: usize,
    pub unknown_logs: usize,
    pub projection: TangentEventProjectionSummary,
    pub derived_liquidation_candidates: Vec<TangentKeeperLiquidationCandidate>,
    pub maintenance_submissions: usize,
    pub maintenance_transaction_hashes: Vec<TxHash>,
    pub maintenance_submission_report: Option<TxWorkflowBatchSubmissionReport>,
    #[serde(default)]
    pub has_maintenance_submission_report: bool,
    pub liquidation_scans: usize,
    #[serde(default)]
    pub has_liquidation_reports: bool,
    pub ready_liquidations: usize,
    #[serde(default)]
    pub has_ready_liquidations: bool,
    pub submitted_liquidations: usize,
    #[serde(default)]
    pub has_submitted_liquidations: bool,
    pub liquidation_reports: Vec<TangentKeeperLiquidationScanReport>,
    pub outcome: KeeperPollingOutcome,
}

/// Small status summary derived from a keeper polling execution report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentKeeperPollingExecutionSummary {
    pub checkpoint: TangentKeeperPollingCheckpoint,
    pub planned_work: bool,
    pub event_query_count: usize,
    pub maintenance_transaction_count: usize,
    pub should_scan_liquidations: bool,
    pub event_records: usize,
    #[serde(default)]
    pub has_event_records: bool,
    pub decoded_events: usize,
    #[serde(default)]
    pub has_decoded_events: bool,
    pub unknown_logs: usize,
    #[serde(default)]
    pub has_unknown_logs: bool,
    pub derived_liquidation_candidates: usize,
    #[serde(default)]
    pub has_derived_liquidation_candidates: bool,
    pub maintenance_submissions: usize,
    #[serde(default)]
    pub maintenance_transaction_hashes: Vec<TxHash>,
    pub liquidation_scans: usize,
    #[serde(default)]
    pub has_liquidation_scans: bool,
    pub ready_liquidations: usize,
    #[serde(default)]
    pub has_ready_liquidations: bool,
    pub submitted_liquidations: usize,
    #[serde(default)]
    pub liquidation_transaction_hashes: Vec<TxHash>,
    pub submitted_transactions: usize,
    #[serde(default)]
    pub has_submissions: bool,
    pub advanced_event_cursor: bool,
    pub completed_maintenance: bool,
    pub completed_liquidation_scan: bool,
}

/// Persistable caller-owned state for one keeper polling loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentKeeperPollingState {
    pub snapshot: KeeperPollingSnapshot,
    pub projection: TangentEventProjection,
}

/// Compact persistable checkpoint for a keeper polling loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentKeeperPollingCheckpoint {
    pub snapshot: KeeperPollingSnapshot,
    pub projection: TangentEventProjectionSummary,
}

/// Serializable restart decision derived from a persisted keeper checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentKeeperPollingResumeReport {
    pub checkpoint: TangentKeeperPollingCheckpoint,
    pub current_block: u64,
    pub effective_event_cursor: Option<RawLogCursor>,
    pub projection_cursor_is_checkpointed: bool,
    pub resume_snapshot: KeeperPollingSnapshot,
}

/// Local preview of one keeper polling pass before executing RPC or transactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentKeeperPollingPreview {
    pub checkpoint: TangentKeeperPollingCheckpoint,
    pub plan_summary: KeeperPollingPlanSummary,
    pub explicit_liquidation_candidates: Vec<TangentKeeperLiquidationCandidate>,
    pub derived_liquidation_candidates: Vec<TangentKeeperLiquidationCandidate>,
    pub scan_candidates: Vec<TangentKeeperLiquidationCandidate>,
}

/// Compact review shape for a keeper polling preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TangentKeeperPollingPreviewSummary {
    pub checkpoint: TangentKeeperPollingCheckpoint,
    pub planned_work: bool,
    pub event_query_count: usize,
    pub maintenance_transaction_count: usize,
    pub should_scan_liquidations: bool,
    pub explicit_liquidation_candidates: usize,
    #[serde(default)]
    pub has_explicit_liquidation_candidates: bool,
    pub derived_liquidation_candidates: usize,
    #[serde(default)]
    pub has_derived_liquidation_candidates: bool,
    pub scan_candidates: usize,
    pub has_scan_candidates: bool,
}

/// Execution result plus the next state to persist before the next poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TangentKeeperPollingStateExecution {
    pub execution: TangentKeeperPollingExecution,
    pub next_state: TangentKeeperPollingState,
}

/// Common interface for fixed-order SDK read plans.
pub trait TangentReadPlan {
    type Output;
    type DecodeError;

    fn calls(&self) -> Vec<UnsignedCall>;
    fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<Self::Output, Self::DecodeError>;
}

/// Errors that can occur while accepting client configuration.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TangentClientConfigError {
    #[error("RPC endpoint URL must not be empty")]
    EmptyEndpoint,
    #[error("RPC endpoint URL scheme must be http, https, ws, or wss: {0}")]
    UnsupportedEndpointScheme(String),
    #[error("RPC header name must not be empty")]
    EmptyHeaderName,
    #[error("duplicate signer backend key id: {0}")]
    DuplicateSignerBackendKeyId(String),
    #[error("signer backend key id is not configured: {0}")]
    MissingSignerBackendKeyId(String),
    #[error(transparent)]
    SignerBackend(#[from] SignerBackendConfigError),
}

/// Errors surfaced while executing and decoding a typed read plan.
#[derive(Debug, thiserror::Error)]
pub enum TangentReadPlanExecutionError<RpcError, DecodeError> {
    #[error("read plan RPC step failed")]
    Rpc(#[from] JsonRpcExecutorError<RpcError>),
    #[error("read plan decode step failed")]
    Decode(DecodeError),
}

/// Errors surfaced by manifest-bound client read helpers.
#[derive(Debug, thiserror::Error)]
pub enum TangentClientReadError<RpcError, DecodeError> {
    #[error("read plan is unavailable because deployment manifest is missing {0}")]
    Unavailable(&'static str),
    #[error(transparent)]
    Execution(#[from] TangentReadPlanExecutionError<RpcError, DecodeError>),
}

/// Errors surfaced while fetching and decoding manifest-bound event logs.
#[derive(Debug, thiserror::Error)]
pub enum TangentClientEventLogError<RpcError> {
    #[error("event log query planning failed")]
    Query(#[from] EventQueryError),
    #[error("event log RPC step failed")]
    Rpc(#[from] JsonRpcExecutorError<RpcError>),
    #[error("event log decode step failed")]
    Decode(#[from] EventDecodeError),
}

/// Errors surfaced while fetching event logs and folding them into a projection.
#[derive(Debug, thiserror::Error)]
pub enum TangentClientEventProjectionError<RpcError> {
    #[error(transparent)]
    EventLogs(#[from] TangentClientEventLogError<RpcError>),
    #[error("event projection failed")]
    Projection(#[from] TangentEventProjectionError),
}

/// Errors surfaced while preflighting and summarizing manifest-bound transactions.
#[derive(Debug, thiserror::Error)]
pub enum TangentClientPreflightSummaryError<RpcError> {
    #[error(transparent)]
    Context(#[from] TangentContextError),
    #[error("transaction preflight RPC step failed")]
    Rpc(#[from] JsonRpcExecutorError<RpcError>),
}

/// Errors surfaced while reading and preflighting a liquidation candidate dry run.
#[derive(Debug, thiserror::Error)]
pub enum TangentLiquidationDryRunError<RpcError> {
    #[error(transparent)]
    Context(#[from] TangentContextError),
    #[error("liquidation dry-run read failed")]
    Read(#[from] TangentReadPlanExecutionError<RpcError, LiquidationDecodeError>),
    #[error("liquidation dry-run preflight failed")]
    Preflight(#[from] JsonRpcExecutorError<RpcError>),
}

/// Errors surfaced while reading market state and preparing an order placement.
#[derive(Debug, thiserror::Error)]
pub enum TangentOrderPlacementPrepareError<RpcError> {
    #[error(transparent)]
    Context(#[from] TangentContextError),
    #[error("order placement market read failed")]
    MarketRead(#[from] TangentReadPlanExecutionError<RpcError, AbiDecodeError>),
    #[error(transparent)]
    Order(#[from] OrderError),
}

/// Errors surfaced while signing and submitting an order placement.
#[derive(Debug, thiserror::Error)]
pub enum TangentOrderPlacementSubmitError<RpcError, SignerError> {
    #[error(transparent)]
    Context(#[from] TangentContextError),
    #[error("order placement market read failed")]
    MarketRead(#[from] TangentReadPlanExecutionError<RpcError, AbiDecodeError>),
    #[error(transparent)]
    Sign(#[from] OrderPlacementSignError<SignerError>),
    #[error("order placement submit transaction failed")]
    Submit(#[from] TxWorkflowError<RpcError, SignerError>),
}

/// Errors surfaced while submitting a manifest-bound signed-order lifecycle transaction.
#[derive(Debug, thiserror::Error)]
pub enum TangentOrderLifecycleSubmitError<RpcError, SignerError> {
    #[error(transparent)]
    Context(#[from] TangentContextError),
    #[error("order lifecycle transaction failed")]
    Submit(#[from] TxWorkflowError<RpcError, SignerError>),
}

/// Errors surfaced while executing manifest-bound keeper workflows.
#[derive(Debug, thiserror::Error)]
pub enum TangentKeeperWorkflowError<RpcError, SignerError> {
    #[error(transparent)]
    Context(#[from] TangentContextError),
    #[error("keeper liquidation read failed")]
    LiquidationRead(#[from] TangentReadPlanExecutionError<RpcError, LiquidationDecodeError>),
    #[error("liquidation candidate is not ready: {0:?}")]
    NotLiquidatable(LiquidationStatus),
    #[error("keeper transaction failed")]
    Submit(#[from] TxWorkflowError<RpcError, SignerError>),
}

/// Errors surfaced while executing one keeper polling pass.
#[derive(Debug, thiserror::Error)]
pub enum TangentKeeperPollingExecutionError<RpcError, SignerError> {
    #[error(transparent)]
    Context(#[from] TangentContextError),
    #[error("keeper polling plan failed")]
    Plan(#[from] EventQueryError),
    #[error("keeper polling event log RPC step failed")]
    EventRpc(#[from] JsonRpcExecutorError<RpcError>),
    #[error("keeper polling event decode step failed")]
    EventDecode(#[from] EventDecodeError),
    #[error("keeper polling event projection failed")]
    Projection(#[from] TangentEventProjectionError),
    #[error("keeper polling liquidation read failed")]
    LiquidationRead(#[from] TangentReadPlanExecutionError<RpcError, LiquidationDecodeError>),
    #[error("keeper polling transaction workflow failed")]
    Submit(#[from] TxWorkflowError<RpcError, SignerError>),
}

fn latest_log_cursor_after(logs: &[RawLog], after: Option<RawLogCursor>) -> Option<RawLogCursor> {
    logs.iter()
        .filter_map(RawLog::cursor)
        .filter(|cursor| match after {
            Some(after) => *cursor > after,
            None => true,
        })
        .max()
}

impl TangentKeeperLiquidationCandidate {
    #[must_use]
    pub const fn new(account_id: u128, market_id: u128) -> Self {
        Self {
            account_id,
            market_id,
        }
    }
}

impl TangentLiquidationDryRun {
    #[must_use]
    pub fn summary(&self) -> TangentLiquidationDryRunSummary {
        TangentLiquidationDryRunSummary {
            candidate: self.candidate,
            readiness: self.readiness,
            is_liquidatable: self.status.is_liquidatable,
            below_maintenance: self.status.is_below_maintenance(),
            equity: self.status.equity,
            maintenance_margin: self.status.maintenance_margin,
            transaction_planned: self.transaction_summary.is_some(),
            has_transaction_summary: self.transaction_summary.is_some(),
            transaction_summary: self.transaction_summary.clone(),
        }
    }
}

impl TangentLiquidationDryRunBatch {
    #[must_use]
    pub fn summary(&self) -> TangentLiquidationDryRunBatchSummary {
        let reports = self
            .reports
            .iter()
            .map(TangentLiquidationDryRun::summary)
            .collect::<Vec<_>>();
        let below_maintenance = reports
            .iter()
            .filter(|report| report.below_maintenance)
            .count();
        let transaction_plans = reports
            .iter()
            .filter(|report| report.transaction_planned)
            .count();

        TangentLiquidationDryRunBatchSummary {
            candidates: self.candidates,
            ready: self.ready,
            blocked: self.blocked,
            has_ready: self.ready > 0,
            has_blocked: self.blocked > 0,
            all_ready: self.candidates > 0 && self.blocked == 0,
            below_maintenance,
            transaction_plans,
            has_transaction_plans: transaction_plans > 0,
            ready_transaction_summary: self.ready_transaction_summary.clone(),
            reports,
        }
    }
}

impl TangentClientStartupReadiness {
    #[must_use]
    pub fn from_parts(
        chain_id_matches_manifest: bool,
        perp_stack: PerpStackAvailability,
        keeper_capabilities: &[KeeperCapability],
    ) -> Self {
        let primitive_reads = chain_id_matches_manifest;
        let orderbook_workflows = chain_id_matches_manifest && perp_stack.order_book;
        let settlement_reads = chain_id_matches_manifest && perp_stack.settlement_engine;
        let liquidation_reads = chain_id_matches_manifest && perp_stack.liquidation_keeper;
        let full_perp_stack = chain_id_matches_manifest && perp_stack.is_complete();
        let keeper_polling = chain_id_matches_manifest
            && keeper_capabilities.contains(&KeeperCapability::EventIndexing);
        let mut blocking_reasons = Vec::new();

        if !chain_id_matches_manifest {
            blocking_reasons.push("configured chain id does not match manifest".to_owned());
        }
        for missing in perp_stack.missing_contracts() {
            blocking_reasons.push(format!("missing {missing}"));
        }

        Self {
            primitive_reads,
            orderbook_workflows,
            settlement_reads,
            liquidation_reads,
            full_perp_stack,
            keeper_polling,
            blocking_reasons,
        }
    }
}

fn single_plan_batch_summary(plan: TxSubmissionPlanSummary) -> TxSubmissionPlanBatchSummary {
    let ready_for_submission_request = tx_submission_plan_summary_is_ready(&plan);
    TxSubmissionPlanBatchSummary {
        len: 1,
        is_empty: false,
        total_calldata_bytes: plan.calldata_bytes,
        total_gas: plan.gas.as_deref().and_then(parse_rpc_quantity_u128),
        first_nonce: plan.nonce.clone(),
        last_nonce: plan.nonce.clone(),
        chain_id: plan.chain_id.clone(),
        all_same_chain_id: true,
        ready_plans: usize::from(ready_for_submission_request),
        not_ready_plans: usize::from(!ready_for_submission_request),
        has_ready_plans: ready_for_submission_request,
        all_ready: ready_for_submission_request,
        eip1559_transactions: usize::from(plan.uses_eip1559_fees),
        legacy_gas_price_transactions: usize::from(plan.uses_legacy_gas_price),
        plans: vec![plan],
    }
}

fn tx_submission_plan_summary_is_ready(summary: &TxSubmissionPlanSummary) -> bool {
    let has_complete_eip1559_fees =
        summary.max_fee_per_gas.is_some() && summary.max_priority_fee_per_gas.is_some();
    summary.chain_id.is_some()
        && summary.nonce.is_some()
        && summary.gas.is_some()
        && (summary.gas_price.is_some() || has_complete_eip1559_fees)
}

fn parse_rpc_quantity_u128(value: &str) -> Option<u128> {
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))?;
    if hex.is_empty() {
        return None;
    }
    u128::from_str_radix(hex, 16).ok()
}

impl TangentKeeperPollingState {
    #[must_use]
    pub fn new(snapshot: KeeperPollingSnapshot, projection: TangentEventProjection) -> Self {
        Self {
            snapshot,
            projection,
        }
    }

    #[must_use]
    pub fn at_block(current_block: u64) -> Self {
        Self::new(
            KeeperPollingSnapshot::at_block(current_block),
            TangentEventProjection::default(),
        )
    }

    #[must_use]
    pub fn with_snapshot(mut self, snapshot: KeeperPollingSnapshot) -> Self {
        self.snapshot = snapshot;
        self
    }

    #[must_use]
    pub fn with_projection(mut self, projection: TangentEventProjection) -> Self {
        self.projection = projection;
        self
    }

    #[must_use]
    pub fn checkpoint(&self) -> TangentKeeperPollingCheckpoint {
        TangentKeeperPollingCheckpoint::new(self.snapshot, self.projection.summary())
    }

    #[must_use]
    pub fn resume_report_at(&self, current_block: u64) -> TangentKeeperPollingResumeReport {
        self.checkpoint().resume_report_at(current_block)
    }
}

impl TangentKeeperPollingExecution {
    #[must_use]
    pub fn next_state(
        &self,
        previous_snapshot: KeeperPollingSnapshot,
    ) -> TangentKeeperPollingState {
        TangentKeeperPollingState::new(
            self.outcome.next_snapshot(previous_snapshot),
            self.projection.clone(),
        )
    }

    #[must_use]
    pub fn checkpoint(
        &self,
        previous_snapshot: KeeperPollingSnapshot,
    ) -> TangentKeeperPollingCheckpoint {
        self.next_state(previous_snapshot).checkpoint()
    }

    #[must_use]
    pub fn report(
        &self,
        previous_snapshot: KeeperPollingSnapshot,
    ) -> TangentKeeperPollingExecutionReport {
        self.report_at_checkpoint(self.checkpoint(previous_snapshot))
    }

    #[must_use]
    pub fn report_at_checkpoint(
        &self,
        checkpoint: TangentKeeperPollingCheckpoint,
    ) -> TangentKeeperPollingExecutionReport {
        let liquidation_reports = self
            .liquidation_results
            .iter()
            .map(TangentKeeperLiquidationScanReport::from_scan_result)
            .collect::<Vec<_>>();
        let ready_liquidations = liquidation_reports
            .iter()
            .filter(|report| report.readiness == crate::LiquidationReadiness::Ready)
            .count();
        let maintenance_transaction_hashes = self
            .maintenance_submission
            .as_ref()
            .map(|submission| {
                submission
                    .submissions
                    .iter()
                    .map(|submission| submission.transaction_hash)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let maintenance_submission_report = self
            .maintenance_submission
            .as_ref()
            .map(TxWorkflowBatchSubmission::report);
        let has_maintenance_submission_report = maintenance_submission_report.is_some();
        let liquidation_scans = liquidation_reports.len();
        let submitted_liquidations = liquidation_reports
            .iter()
            .filter(|report| report.submitted_transaction_hash.is_some())
            .count();

        TangentKeeperPollingExecutionReport {
            checkpoint,
            plan_summary: self.plan.summary(),
            event_records: self.event_records.known_logs(),
            decoded_events: self.events.known_logs(),
            unknown_logs: self.events.unknown_logs,
            projection: self.projection.summary(),
            derived_liquidation_candidates: self.derived_liquidation_candidates.clone(),
            maintenance_submissions: maintenance_transaction_hashes.len(),
            maintenance_transaction_hashes,
            maintenance_submission_report,
            has_maintenance_submission_report,
            liquidation_scans,
            has_liquidation_reports: liquidation_scans > 0,
            ready_liquidations,
            has_ready_liquidations: ready_liquidations > 0,
            submitted_liquidations,
            has_submitted_liquidations: submitted_liquidations > 0,
            liquidation_reports,
            outcome: self.outcome,
        }
    }
}

impl TangentKeeperPollingExecutionReport {
    #[must_use]
    pub fn summary(&self) -> TangentKeeperPollingExecutionSummary {
        let submitted_transactions = self
            .maintenance_submissions
            .saturating_add(self.submitted_liquidations);
        let liquidation_transaction_hashes = self
            .liquidation_reports
            .iter()
            .filter_map(|report| report.submitted_transaction_hash)
            .collect::<Vec<_>>();

        TangentKeeperPollingExecutionSummary {
            checkpoint: self.checkpoint,
            planned_work: self.plan_summary.has_work,
            event_query_count: self.plan_summary.event_query_count,
            maintenance_transaction_count: self.plan_summary.maintenance_transaction_count,
            should_scan_liquidations: self.plan_summary.should_scan_liquidations,
            event_records: self.event_records,
            has_event_records: self.event_records > 0,
            decoded_events: self.decoded_events,
            has_decoded_events: self.decoded_events > 0,
            unknown_logs: self.unknown_logs,
            has_unknown_logs: self.unknown_logs > 0,
            derived_liquidation_candidates: self.derived_liquidation_candidates.len(),
            has_derived_liquidation_candidates: !self.derived_liquidation_candidates.is_empty(),
            maintenance_submissions: self.maintenance_submissions,
            maintenance_transaction_hashes: self.maintenance_transaction_hashes.clone(),
            liquidation_scans: self.liquidation_scans,
            has_liquidation_scans: self.liquidation_scans > 0,
            ready_liquidations: self.ready_liquidations,
            has_ready_liquidations: self.ready_liquidations > 0,
            submitted_liquidations: self.submitted_liquidations,
            liquidation_transaction_hashes,
            submitted_transactions,
            has_submissions: submitted_transactions > 0,
            advanced_event_cursor: self.outcome.latest_event_cursor.is_some(),
            completed_maintenance: self.outcome.completed_maintenance,
            completed_liquidation_scan: self.outcome.completed_liquidation_scan,
        }
    }
}

impl TangentKeeperLiquidationScanReport {
    #[must_use]
    pub fn from_scan_result(result: &TangentKeeperLiquidationScanResult) -> Self {
        let has_submission = result.submission.is_some();
        let submitted_transaction_hash = result
            .submission
            .as_ref()
            .map(|submission| submission.transaction_hash);
        let submitted_transaction_report =
            result.submission.as_ref().map(TxWorkflowSubmission::report);
        Self {
            candidate: result.candidate,
            readiness: result.status.readiness(),
            has_submission,
            has_submitted_transaction_hash: submitted_transaction_hash.is_some(),
            submitted_transaction_hash,
            has_submitted_transaction_report: submitted_transaction_report.is_some(),
            submitted_transaction_report,
        }
    }
}

impl TangentKeeperPollingPreview {
    #[must_use]
    pub fn summary(&self) -> TangentKeeperPollingPreviewSummary {
        TangentKeeperPollingPreviewSummary {
            checkpoint: self.checkpoint,
            planned_work: self.plan_summary.has_work,
            event_query_count: self.plan_summary.event_query_count,
            maintenance_transaction_count: self.plan_summary.maintenance_transaction_count,
            should_scan_liquidations: self.plan_summary.should_scan_liquidations,
            explicit_liquidation_candidates: self.explicit_liquidation_candidates.len(),
            has_explicit_liquidation_candidates: !self.explicit_liquidation_candidates.is_empty(),
            derived_liquidation_candidates: self.derived_liquidation_candidates.len(),
            has_derived_liquidation_candidates: !self.derived_liquidation_candidates.is_empty(),
            scan_candidates: self.scan_candidates.len(),
            has_scan_candidates: !self.scan_candidates.is_empty(),
        }
    }
}

impl TangentKeeperPollingStateExecution {
    #[must_use]
    pub fn checkpoint(&self) -> TangentKeeperPollingCheckpoint {
        self.next_state.checkpoint()
    }

    #[must_use]
    pub fn report(&self) -> TangentKeeperPollingExecutionReport {
        self.execution.report_at_checkpoint(self.checkpoint())
    }
}

impl TangentKeeperPollingCheckpoint {
    #[must_use]
    pub const fn new(
        snapshot: KeeperPollingSnapshot,
        projection: TangentEventProjectionSummary,
    ) -> Self {
        Self {
            snapshot,
            projection,
        }
    }

    #[must_use]
    pub fn effective_event_cursor(&self) -> Option<RawLogCursor> {
        self.snapshot.event_cursor.max(self.projection.last_cursor)
    }

    #[must_use]
    pub fn projection_cursor_is_checkpointed(&self) -> bool {
        match (self.snapshot.event_cursor, self.projection.last_cursor) {
            (_, None) => true,
            (Some(snapshot), Some(projection)) => snapshot >= projection,
            (None, Some(_)) => false,
        }
    }

    #[must_use]
    pub fn reconciled_snapshot(&self) -> KeeperPollingSnapshot {
        let mut snapshot = self.snapshot;
        if let Some(cursor) = self.effective_event_cursor() {
            snapshot.event_cursor = Some(cursor);
            snapshot.event_from_block = None;
        }
        snapshot
    }

    #[must_use]
    pub fn resume_snapshot_at(&self, current_block: u64) -> KeeperPollingSnapshot {
        KeeperPollingSnapshot {
            current_block,
            ..self.reconciled_snapshot()
        }
    }

    #[must_use]
    pub fn resume_report_at(&self, current_block: u64) -> TangentKeeperPollingResumeReport {
        TangentKeeperPollingResumeReport {
            checkpoint: *self,
            current_block,
            effective_event_cursor: self.effective_event_cursor(),
            projection_cursor_is_checkpointed: self.projection_cursor_is_checkpointed(),
            resume_snapshot: self.resume_snapshot_at(current_block),
        }
    }

    #[must_use]
    pub fn resume_state_at(
        &self,
        current_block: u64,
        projection: TangentEventProjection,
    ) -> TangentKeeperPollingState {
        let checkpoint = TangentKeeperPollingCheckpoint::new(
            self.resume_snapshot_at(current_block),
            projection.summary(),
        );
        TangentKeeperPollingState::new(checkpoint.reconciled_snapshot(), projection)
    }
}

impl From<AccountMarketProjectionKey> for TangentKeeperLiquidationCandidate {
    fn from(value: AccountMarketProjectionKey) -> Self {
        Self::new(value.account_id, value.market_id)
    }
}

impl TangentReadPlan for AccountStatusPlan {
    type Output = AccountStatus;
    type DecodeError = AbiDecodeError;

    fn calls(&self) -> Vec<UnsignedCall> {
        AccountStatusPlan::calls(self).to_vec()
    }

    fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<Self::Output, Self::DecodeError> {
        AccountStatusPlan::decode_return_batch(self, returns)
    }
}

impl TangentReadPlan for CollateralStatusPlan {
    type Output = CollateralStatus;
    type DecodeError = AbiDecodeError;

    fn calls(&self) -> Vec<UnsignedCall> {
        CollateralStatusPlan::calls(self).to_vec()
    }

    fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<Self::Output, Self::DecodeError> {
        CollateralStatusPlan::decode_return_batch(self, returns)
    }
}

impl TangentReadPlan for MarketReadPlan {
    type Output = MarketReadSummary;
    type DecodeError = AbiDecodeError;

    fn calls(&self) -> Vec<UnsignedCall> {
        MarketReadPlan::calls(self).to_vec()
    }

    fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<Self::Output, Self::DecodeError> {
        MarketReadPlan::decode_return_batch(self, returns)
    }
}

impl TangentReadPlan for OrderLifecyclePlan {
    type Output = OrderLifecycleStatus;
    type DecodeError = AbiDecodeError;

    fn calls(&self) -> Vec<UnsignedCall> {
        OrderLifecyclePlan::calls(self).to_vec()
    }

    fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<Self::Output, Self::DecodeError> {
        OrderLifecyclePlan::decode_return_batch(self, returns)
    }
}

impl TangentReadPlan for SettlementReadPlan {
    type Output = SettlementStatus;
    type DecodeError = AbiDecodeError;

    fn calls(&self) -> Vec<UnsignedCall> {
        SettlementReadPlan::calls(self).to_vec()
    }

    fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<Self::Output, Self::DecodeError> {
        SettlementReadPlan::decode_return_batch(self, returns)
    }
}

impl TangentReadPlan for LiquidationReadPlan {
    type Output = LiquidationStatus;
    type DecodeError = LiquidationDecodeError;

    fn calls(&self) -> Vec<UnsignedCall> {
        LiquidationReadPlan::calls(self).to_vec()
    }

    fn decode_return_batch(
        &self,
        returns: &CallReturnBatch,
    ) -> Result<Self::Output, Self::DecodeError> {
        LiquidationReadPlan::decode_return_batch(self, returns)
    }
}

impl RpcEndpointConfig {
    pub fn new(url: impl Into<String>) -> Result<Self, TangentClientConfigError> {
        let endpoint = Self {
            url: url.into(),
            headers: Vec::new(),
        };
        endpoint.validate()?;
        Ok(endpoint)
    }

    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, TangentClientConfigError> {
        let header = RpcHeader {
            name: name.into(),
            value: value.into(),
        };
        header.validate()?;
        self.headers.push(header);
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), TangentClientConfigError> {
        if self.url.trim().is_empty() {
            return Err(TangentClientConfigError::EmptyEndpoint);
        }

        match self.scheme() {
            Some("http" | "https" | "ws" | "wss") => {}
            Some(scheme) => {
                return Err(TangentClientConfigError::UnsupportedEndpointScheme(
                    scheme.to_owned(),
                ));
            }
            None => {
                return Err(TangentClientConfigError::UnsupportedEndpointScheme(
                    String::new(),
                ));
            }
        }

        for header in &self.headers {
            header.validate()?;
        }

        Ok(())
    }

    #[must_use]
    pub fn scheme(&self) -> Option<&str> {
        self.url.split_once(':').map(|(scheme, _)| scheme)
    }

    #[must_use]
    pub fn is_secure(&self) -> bool {
        matches!(self.scheme(), Some("https" | "wss"))
    }

    #[must_use]
    pub fn redacted(&self) -> Self {
        Self {
            url: self.url.clone(),
            headers: self
                .headers
                .iter()
                .map(|header| RpcHeader {
                    name: header.name.clone(),
                    value: "<redacted>".to_owned(),
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn report(&self) -> RpcEndpointConfigReport {
        let static_rpc_auth_headers = self
            .headers
            .iter()
            .filter(|header| is_auth_header_name(&header.name))
            .count();

        RpcEndpointConfigReport {
            scheme: self.scheme().map(ToOwned::to_owned),
            is_secure: self.is_secure(),
            static_rpc_headers: self.headers.len(),
            static_rpc_header_names: self
                .headers
                .iter()
                .map(|header| header.name.clone())
                .collect(),
            static_rpc_auth_headers,
            has_static_rpc_auth_header: static_rpc_auth_headers > 0,
        }
    }
}

impl RpcHeader {
    pub fn validate(&self) -> Result<(), TangentClientConfigError> {
        if self.name.trim().is_empty() {
            return Err(TangentClientConfigError::EmptyHeaderName);
        }
        Ok(())
    }
}

impl Default for TangentClientPolicies {
    fn default() -> Self {
        Self {
            retry: JsonRpcRetryPolicy::default(),
            backoff: JsonRpcBackoffPolicy::new(250, 2_000),
            fee: TxFeePolicy::Eip1559FromGasPrice {
                max_fee_multiplier: 2,
                min_priority_fee_per_gas: None,
            },
            confirmation: TxConfirmationPolicy::new(2).with_timeout_blocks(20),
            keeper_polling: KeeperPollingPolicy::default(),
        }
    }
}

impl TangentClientConfig {
    pub fn new(
        endpoint: RpcEndpointConfig,
        chain_id: u64,
    ) -> Result<Self, TangentClientConfigError> {
        Self::with_policies(endpoint, chain_id, TangentClientPolicies::default())
    }

    pub fn with_policies(
        endpoint: RpcEndpointConfig,
        chain_id: u64,
        policies: TangentClientPolicies,
    ) -> Result<Self, TangentClientConfigError> {
        let config = Self {
            endpoint,
            chain_id,
            policies,
            signer_backends: Vec::new(),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn with_signer_backend(
        mut self,
        backend: SignerBackendConfig,
    ) -> Result<Self, TangentClientConfigError> {
        backend.validate()?;
        self.ensure_signer_backend_key_id_available(&backend.key_id)?;
        self.signer_backends.push(backend);
        Ok(self)
    }

    pub fn with_signer_backends(
        mut self,
        backends: impl IntoIterator<Item = SignerBackendConfig>,
    ) -> Result<Self, TangentClientConfigError> {
        for backend in backends {
            self = self.with_signer_backend(backend)?;
        }
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), TangentClientConfigError> {
        self.endpoint.validate()?;
        for (index, backend) in self.signer_backends.iter().enumerate() {
            backend.validate()?;
            if self.signer_backends[..index]
                .iter()
                .any(|existing| existing.key_id == backend.key_id)
            {
                return Err(TangentClientConfigError::DuplicateSignerBackendKeyId(
                    backend.key_id.clone(),
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn redacted(&self) -> Self {
        Self {
            endpoint: self.endpoint.redacted(),
            chain_id: self.chain_id,
            policies: self.policies,
            signer_backends: self
                .signer_backends
                .iter()
                .map(SignerBackendConfig::redacted)
                .collect(),
        }
    }

    #[must_use]
    pub fn report(&self) -> TangentClientConfigReport {
        let mut signer_backend_kinds = Vec::new();
        for backend in &self.signer_backends {
            if !signer_backend_kinds.contains(&backend.kind) {
                signer_backend_kinds.push(backend.kind);
            }
        }

        TangentClientConfigReport {
            chain_id: self.chain_id,
            endpoint: self.endpoint.report(),
            policies: self.policies,
            signer_backend_count: self.signer_backends.len(),
            has_signer_backends: !self.signer_backends.is_empty(),
            has_multiple_signer_backend_kinds: signer_backend_kinds.len() > 1,
            signer_backend_kinds,
            signer_backends: self
                .signer_backends
                .iter()
                .map(SignerBackendConfigReport::from_backend)
                .collect(),
        }
    }

    #[must_use]
    pub fn signer_backend(&self, key_id: &str) -> Option<&SignerBackendConfig> {
        self.signer_backends
            .iter()
            .find(|backend| backend.key_id == key_id)
    }

    pub fn require_signer_backend(
        &self,
        key_id: &str,
    ) -> Result<&SignerBackendConfig, TangentClientConfigError> {
        self.signer_backend(key_id)
            .ok_or_else(|| TangentClientConfigError::MissingSignerBackendKeyId(key_id.to_owned()))
    }

    #[must_use]
    pub fn signer_backend_for_kind(&self, kind: SignerBackendKind) -> Option<&SignerBackendConfig> {
        self.signer_backends
            .iter()
            .find(|backend| backend.kind == kind)
    }

    #[must_use]
    pub fn signer_backends_for_kind(&self, kind: SignerBackendKind) -> Vec<&SignerBackendConfig> {
        self.signer_backends
            .iter()
            .filter(|backend| backend.kind == kind)
            .collect()
    }

    #[must_use]
    pub fn signer_backend_for_address(
        &self,
        address: alloy_primitives::Address,
    ) -> Option<&SignerBackendConfig> {
        self.signer_backends
            .iter()
            .find(|backend| backend.address == Some(address))
    }

    pub fn external_signer_adapter<C>(
        &self,
        key_id: &str,
        client: C,
    ) -> Result<ExternalSignerAdapter<C>, TangentClientConfigError> {
        ExternalSignerAdapter::new(self.require_signer_backend(key_id)?.clone(), client)
            .map_err(Into::into)
    }

    pub fn external_signer_adapter_for_kind<C>(
        &self,
        kind: SignerBackendKind,
        client: C,
    ) -> Option<ExternalSignerAdapter<C>> {
        self.signer_backend_for_kind(kind)
            .and_then(|backend| ExternalSignerAdapter::new(backend.clone(), client).ok())
    }

    pub fn external_signer_adapter_for_address<C>(
        &self,
        address: alloy_primitives::Address,
        client: C,
    ) -> Option<ExternalSignerAdapter<C>> {
        self.signer_backend_for_address(address)
            .and_then(|backend| ExternalSignerAdapter::new(backend.clone(), client).ok())
    }

    fn ensure_signer_backend_key_id_available(
        &self,
        key_id: &str,
    ) -> Result<(), TangentClientConfigError> {
        if self.signer_backend(key_id).is_some() {
            Err(TangentClientConfigError::DuplicateSignerBackendKeyId(
                key_id.to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

impl SignerBackendConfigReport {
    #[must_use]
    pub fn from_backend(backend: &SignerBackendConfig) -> Self {
        let metadata_keys = backend
            .metadata
            .iter()
            .map(|metadata| metadata.key.clone())
            .collect::<Vec<_>>();
        let secret_metadata_keys = backend
            .metadata
            .iter()
            .filter(|metadata| metadata.secret)
            .map(|metadata| metadata.key.clone())
            .collect::<Vec<_>>();

        Self {
            kind: backend.kind,
            key_id: backend.key_id.clone(),
            address: backend.address,
            metadata_count: metadata_keys.len(),
            secret_metadata_count: secret_metadata_keys.len(),
            has_address: backend.address.is_some(),
            has_metadata: !metadata_keys.is_empty(),
            has_secret_metadata: !secret_metadata_keys.is_empty(),
            metadata_keys,
            secret_metadata_keys,
        }
    }
}

fn is_auth_header_name(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "authorization" | "x-api-key" | "api-key" | "apikey"
    ) || normalized.contains("auth")
        || normalized.contains("api-key")
}

impl TangentClientPlan {
    pub fn new(
        manifest: DeploymentManifest,
        endpoint: RpcEndpointConfig,
    ) -> Result<Self, TangentClientConfigError> {
        Self::with_policies(manifest, endpoint, TangentClientPolicies::default())
    }

    pub fn with_policies(
        manifest: DeploymentManifest,
        endpoint: RpcEndpointConfig,
        policies: TangentClientPolicies,
    ) -> Result<Self, TangentClientConfigError> {
        let chain_id = manifest.chain_id;
        Ok(Self {
            context: TangentContext::new(manifest),
            config: TangentClientConfig::with_policies(endpoint, chain_id, policies)?,
        })
    }

    #[must_use]
    pub const fn context(&self) -> &TangentContext {
        &self.context
    }

    #[must_use]
    pub const fn config(&self) -> &TangentClientConfig {
        &self.config
    }

    #[must_use]
    pub const fn chain_id_matches_manifest(&self) -> bool {
        self.context.chain_id() == self.config.chain_id
    }

    #[must_use]
    pub fn keeper_runtime(&self) -> KeeperRuntimePlan {
        self.context.keeper_runtime()
    }

    #[must_use]
    pub fn startup_report(&self) -> TangentClientStartupReport {
        let manifest = self.context.manifest();
        let perp_stack = manifest.perp_stack_availability();
        let keeper_runtime = self.keeper_runtime();
        let keeper_capabilities = keeper_runtime.capabilities();
        let chain_id_matches_manifest = self.chain_id_matches_manifest();
        let mut signer_backend_kinds = Vec::new();
        for backend in &self.config.signer_backends {
            if !signer_backend_kinds.contains(&backend.kind) {
                signer_backend_kinds.push(backend.kind);
            }
        }
        let endpoint_report = self.config.endpoint.report();

        TangentClientStartupReport {
            project: manifest.project.clone(),
            version: manifest.version.clone(),
            network: manifest.network.clone(),
            manifest_chain_id: manifest.chain_id,
            configured_chain_id: self.config.chain_id,
            chain_id_matches_manifest,
            endpoint_scheme: endpoint_report.scheme,
            endpoint_is_secure: endpoint_report.is_secure,
            static_rpc_headers: endpoint_report.static_rpc_headers,
            static_rpc_auth_headers: endpoint_report.static_rpc_auth_headers,
            has_static_rpc_auth_header: endpoint_report.has_static_rpc_auth_header,
            signer_backend_count: self.config.signer_backends.len(),
            has_signer_backends: !self.config.signer_backends.is_empty(),
            has_multiple_signer_backend_kinds: signer_backend_kinds.len() > 1,
            signer_backend_kinds,
            perp_stack,
            missing_perp_contracts: perp_stack
                .missing_contracts()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            keeper_capabilities: keeper_capabilities.clone(),
            readiness: TangentClientStartupReadiness::from_parts(
                chain_id_matches_manifest,
                perp_stack,
                &keeper_capabilities,
            ),
            policies: self.config.policies,
        }
    }

    #[must_use]
    pub fn support_report(&self) -> TangentClientSupportReport {
        TangentClientSupportReport {
            startup: self.startup_report(),
            config: self.config.report(),
        }
    }

    #[must_use]
    pub fn event_log_query_summary(
        &self,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> EventLogRpcQuerySummary {
        self.context
            .event_filters()
            .to_query(from_block, to_block)
            .to_rpc_query()
            .summary()
    }

    #[must_use]
    pub fn resume_event_log_query_summary(
        &self,
        cursor: RawLogCursor,
        to_block: Option<u64>,
    ) -> EventLogRpcQuerySummary {
        self.context
            .event_filters()
            .resume_rpc_query(cursor, to_block)
            .summary()
    }

    pub fn chunked_event_log_query_summary(
        &self,
        from_block: u64,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<EventLogRpcQueryBatchSummary, crate::EventQueryError> {
        let queries = self
            .context
            .event_filters()
            .chunked_rpc_queries(from_block, to_block, max_blocks)?;
        Ok(EventLogRpcQuery::summarize_batch(&queries))
    }

    pub fn chunked_resume_event_log_query_summary(
        &self,
        cursor: RawLogCursor,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<EventLogRpcQueryBatchSummary, crate::EventQueryError> {
        let queries = self.context.event_filters().chunked_rpc_queries(
            cursor.resume_from_block(),
            to_block,
            max_blocks,
        )?;
        Ok(EventLogRpcQuery::summarize_batch(&queries))
    }

    pub fn keeper_polling_plan(
        &self,
        snapshot: KeeperPollingSnapshot,
    ) -> Result<KeeperPollingPlan, crate::EventQueryError> {
        self.keeper_runtime()
            .polling_plan(snapshot, self.config.policies.keeper_polling)
    }

    pub fn keeper_polling_plan_summary(
        &self,
        snapshot: KeeperPollingSnapshot,
    ) -> Result<KeeperPollingPlanSummary, crate::EventQueryError> {
        Ok(self.keeper_polling_plan(snapshot)?.summary())
    }

    pub fn keeper_polling_preview(
        &self,
        state: &TangentKeeperPollingState,
        liquidation_candidates: &[TangentKeeperLiquidationCandidate],
    ) -> Result<TangentKeeperPollingPreview, crate::EventQueryError> {
        let plan_summary = self.keeper_polling_plan_summary(state.snapshot)?;
        let derived_liquidation_candidates = state
            .projection
            .account_market_keys()
            .into_iter()
            .map(TangentKeeperLiquidationCandidate::from)
            .collect::<Vec<_>>();
        let scan_candidates = if plan_summary.should_scan_liquidations {
            liquidation_candidates
                .iter()
                .copied()
                .chain(derived_liquidation_candidates.iter().copied())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };

        Ok(TangentKeeperPollingPreview {
            checkpoint: state.checkpoint(),
            plan_summary,
            explicit_liquidation_candidates: liquidation_candidates.to_vec(),
            derived_liquidation_candidates,
            scan_candidates,
        })
    }
}

impl<T> TangentClient<T> {
    #[must_use]
    pub fn new(plan: TangentClientPlan, transport: T) -> Self {
        Self {
            plan,
            rpc: JsonRpcExecutor::new(transport),
        }
    }

    #[must_use]
    pub const fn with_executor(plan: TangentClientPlan, rpc: JsonRpcExecutor<T>) -> Self {
        Self { plan, rpc }
    }

    #[must_use]
    pub const fn plan(&self) -> &TangentClientPlan {
        &self.plan
    }

    #[must_use]
    pub const fn config(&self) -> &TangentClientConfig {
        self.plan.config()
    }

    #[must_use]
    pub const fn context(&self) -> &TangentContext {
        self.plan.context()
    }

    pub fn keeper_polling_plan_summary(
        &self,
        snapshot: KeeperPollingSnapshot,
    ) -> Result<KeeperPollingPlanSummary, crate::EventQueryError> {
        self.plan.keeper_polling_plan_summary(snapshot)
    }

    pub fn keeper_polling_preview(
        &self,
        state: &TangentKeeperPollingState,
        liquidation_candidates: &[TangentKeeperLiquidationCandidate],
    ) -> Result<TangentKeeperPollingPreview, crate::EventQueryError> {
        self.plan
            .keeper_polling_preview(state, liquidation_candidates)
    }

    #[must_use]
    pub const fn rpc(&self) -> &JsonRpcExecutor<T> {
        &self.rpc
    }

    #[must_use]
    pub fn rpc_mut(&mut self) -> &mut JsonRpcExecutor<T> {
        &mut self.rpc
    }

    #[must_use]
    pub fn into_parts(self) -> (TangentClientPlan, JsonRpcExecutor<T>) {
        (self.plan, self.rpc)
    }

    #[must_use]
    pub fn into_workflow<S>(self, signer: S) -> TangentClientWorkflow<T, S> {
        TangentClientWorkflow {
            plan: self.plan,
            workflow: TxWorkflowExecutor::new(self.rpc, signer),
        }
    }
}

impl<T: JsonRpcTransport> TangentClient<T> {
    pub fn call(
        &mut self,
        call: &UnsignedCall,
        block: crate::RpcBlockTag,
    ) -> Result<CallReturn, JsonRpcExecutorError<T::Error>> {
        self.rpc.call(call, block)
    }

    pub fn call_batch(
        &mut self,
        calls: &[UnsignedCall],
        block: crate::RpcBlockTag,
    ) -> Result<CallReturnBatch, JsonRpcExecutorError<T::Error>> {
        self.rpc.call_batch(calls, block)
    }

    pub fn read_plan<P: TangentReadPlan>(
        &mut self,
        plan: &P,
        block: crate::RpcBlockTag,
    ) -> Result<P::Output, TangentReadPlanExecutionError<T::Error, P::DecodeError>> {
        let calls = plan.calls();
        let returns = self
            .rpc
            .call_batch(&calls, block)
            .map_err(TangentReadPlanExecutionError::Rpc)?;
        plan.decode_return_batch(&returns)
            .map_err(TangentReadPlanExecutionError::Decode)
    }

    #[must_use]
    pub fn read_plan_summary<P: TangentReadPlan>(&self, plan: &P) -> UnsignedCallBatchSummary {
        let calls = plan.calls();
        UnsignedCall::summarize_batch(&calls)
    }

    #[must_use]
    pub fn event_log_query_summary(
        &self,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> EventLogRpcQuerySummary {
        self.plan.event_log_query_summary(from_block, to_block)
    }

    #[must_use]
    pub fn resume_event_log_query_summary(
        &self,
        cursor: RawLogCursor,
        to_block: Option<u64>,
    ) -> EventLogRpcQuerySummary {
        self.plan.resume_event_log_query_summary(cursor, to_block)
    }

    pub fn chunked_event_log_query_summary(
        &self,
        from_block: u64,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<EventLogRpcQueryBatchSummary, crate::EventQueryError> {
        self.plan
            .chunked_event_log_query_summary(from_block, to_block, max_blocks)
    }

    pub fn chunked_resume_event_log_query_summary(
        &self,
        cursor: RawLogCursor,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<EventLogRpcQueryBatchSummary, crate::EventQueryError> {
        self.plan
            .chunked_resume_event_log_query_summary(cursor, to_block, max_blocks)
    }

    pub fn account_status(
        &mut self,
        owner: alloy_primitives::Address,
        account_id: u128,
        block: crate::RpcBlockTag,
    ) -> Result<AccountStatus, TangentReadPlanExecutionError<T::Error, AbiDecodeError>> {
        let plan = self.context().account_status(owner, account_id);
        self.read_plan(&plan, block)
    }

    pub fn collateral_status(
        &mut self,
        owner: alloy_primitives::Address,
        account_id: u128,
        block: crate::RpcBlockTag,
    ) -> Result<CollateralStatus, TangentReadPlanExecutionError<T::Error, AbiDecodeError>> {
        let plan = self.context().collateral_status(owner, account_id);
        self.read_plan(&plan, block)
    }

    pub fn market_summary(
        &mut self,
        market_id: u128,
        block: crate::RpcBlockTag,
    ) -> Result<MarketReadSummary, TangentReadPlanExecutionError<T::Error, AbiDecodeError>> {
        let plan = self.context().market(market_id);
        self.read_plan(&plan, block)
    }

    pub fn prepare_order_placement(
        &mut self,
        params: OrderParams,
        current_timestamp: u64,
        market_block: crate::RpcBlockTag,
    ) -> Result<TangentOrderPlacementPreparation, TangentOrderPlacementPrepareError<T::Error>> {
        let plan = self.context().order_placement(params, current_timestamp)?;
        let market = self.read_plan(&plan.market_plan, market_block)?;
        let prepared_order = plan.prepare(&market)?;
        Ok(TangentOrderPlacementPreparation {
            plan,
            market,
            prepared_order,
        })
    }

    pub fn order_lifecycle_status(
        &mut self,
        signed_order: SignedOrder,
        block: crate::RpcBlockTag,
    ) -> Result<OrderLifecycleStatus, TangentClientReadError<T::Error, AbiDecodeError>> {
        let plan = self
            .context()
            .order_lifecycle(signed_order)
            .ok_or(TangentClientReadError::Unavailable("OrderBook"))?;
        self.read_plan(&plan, block).map_err(Into::into)
    }

    pub fn settlement_status(
        &mut self,
        account_id: u128,
        market_id: u128,
        block: crate::RpcBlockTag,
    ) -> Result<SettlementStatus, TangentClientReadError<T::Error, AbiDecodeError>> {
        let plan = self
            .context()
            .settlement(account_id, market_id)
            .ok_or(TangentClientReadError::Unavailable("SettlementEngine"))?;
        self.read_plan(&plan, block).map_err(Into::into)
    }

    pub fn liquidation_status(
        &mut self,
        account_id: u128,
        market_id: u128,
        block: crate::RpcBlockTag,
    ) -> Result<LiquidationStatus, TangentClientReadError<T::Error, LiquidationDecodeError>> {
        let plan = self
            .context()
            .liquidation(account_id, market_id)
            .ok_or(TangentClientReadError::Unavailable("LiquidationKeeper"))?;
        self.read_plan(&plan, block).map_err(Into::into)
    }

    pub fn logs(
        &mut self,
        query: &EventLogRpcQuery,
    ) -> Result<Vec<RawLog>, JsonRpcExecutorError<T::Error>> {
        self.rpc.logs(query)
    }

    pub fn decoded_event_logs(
        &mut self,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> Result<DecodedTangentLogs, TangentClientEventLogError<T::Error>> {
        let filters = self.context().event_filters();
        let logs = self.logs(&filters.to_query(from_block, to_block).to_rpc_query())?;
        filters.decode_logs(&logs).map_err(Into::into)
    }

    pub fn decoded_event_logs_for_query(
        &mut self,
        query: &EventLogRpcQuery,
    ) -> Result<DecodedTangentLogs, TangentClientEventLogError<T::Error>> {
        let filters = self.context().event_filters();
        let logs = self.logs(query)?;
        filters.decode_logs(&logs).map_err(Into::into)
    }

    pub fn decoded_event_log_records_for_query(
        &mut self,
        query: &EventLogRpcQuery,
    ) -> Result<DecodedTangentLogRecords, TangentClientEventLogError<T::Error>> {
        let filters = self.context().event_filters();
        let logs = self.logs(query)?;
        filters.decode_log_records(&logs).map_err(Into::into)
    }

    pub fn resume_decoded_event_logs(
        &mut self,
        cursor: RawLogCursor,
        to_block: Option<u64>,
    ) -> Result<DecodedTangentLogs, TangentClientEventLogError<T::Error>> {
        let filters = self.context().event_filters();
        let logs = self.logs(&filters.resume_rpc_query(cursor, to_block))?;
        filters
            .decode_logs_after_cursor(&logs, cursor)
            .map_err(Into::into)
    }

    pub fn chunked_decoded_event_logs(
        &mut self,
        from_block: u64,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<DecodedTangentLogs, TangentClientEventLogError<T::Error>> {
        let filters = self.context().event_filters();
        let queries = filters.chunked_rpc_queries(from_block, to_block, max_blocks)?;
        let mut decoded = DecodedTangentLogs::default();

        for query in queries {
            let logs = self.logs(&query)?;
            decoded.extend(filters.decode_logs(&logs)?);
        }

        Ok(decoded)
    }

    pub fn chunked_decoded_event_log_records(
        &mut self,
        from_block: u64,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<DecodedTangentLogRecords, TangentClientEventLogError<T::Error>> {
        let filters = self.context().event_filters();
        let queries = filters.chunked_rpc_queries(from_block, to_block, max_blocks)?;
        let mut decoded = DecodedTangentLogRecords::default();

        for query in queries {
            let logs = self.logs(&query)?;
            decoded.extend(filters.decode_log_records(&logs)?);
        }

        Ok(decoded)
    }

    pub fn chunked_event_projection(
        &mut self,
        from_block: u64,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<TangentEventProjection, TangentClientEventProjectionError<T::Error>> {
        let records = self.chunked_decoded_event_log_records(from_block, to_block, max_blocks)?;
        let mut projection = TangentEventProjection::default();
        projection.apply_records(&records)?;
        Ok(projection)
    }

    pub fn chunked_resume_decoded_event_logs(
        &mut self,
        cursor: RawLogCursor,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<DecodedTangentLogs, TangentClientEventLogError<T::Error>> {
        let filters = self.context().event_filters();
        let queries =
            filters.chunked_rpc_queries(cursor.resume_from_block(), to_block, max_blocks)?;
        let mut decoded = DecodedTangentLogs::default();

        for query in queries {
            let logs = self.logs(&query)?;
            decoded.extend(filters.decode_logs_after_cursor(&logs, cursor)?);
        }

        Ok(decoded)
    }

    pub fn chunked_resume_decoded_event_log_records(
        &mut self,
        cursor: RawLogCursor,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<DecodedTangentLogRecords, TangentClientEventLogError<T::Error>> {
        let filters = self.context().event_filters();
        let queries =
            filters.chunked_rpc_queries(cursor.resume_from_block(), to_block, max_blocks)?;
        let mut decoded = DecodedTangentLogRecords::default();

        for query in queries {
            let logs = self.logs(&query)?;
            decoded.extend(filters.decode_log_records_after_cursor(&logs, cursor)?);
        }

        Ok(decoded)
    }

    pub fn chunked_resume_event_projection(
        &mut self,
        cursor: RawLogCursor,
        to_block: u64,
        max_blocks: u64,
    ) -> Result<TangentEventProjection, TangentClientEventProjectionError<T::Error>> {
        let records =
            self.chunked_resume_decoded_event_log_records(cursor, to_block, max_blocks)?;
        let mut projection = TangentEventProjection::default();
        projection.apply_records(&records)?;
        Ok(projection)
    }

    pub fn preflight_transaction_plans(
        &mut self,
        txs: &[UnsignedTx],
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<Vec<TxSubmissionPlan>, JsonRpcExecutorError<T::Error>> {
        self.rpc.preflight_transaction_plans(
            txs,
            from,
            nonce_block,
            self.plan.config.policies.fee,
            self.plan.config.policies.confirmation,
        )
    }

    pub fn preflight_transaction_summary(
        &mut self,
        txs: &[UnsignedTx],
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxSubmissionPlanBatchSummary, JsonRpcExecutorError<T::Error>> {
        let plans = self.preflight_transaction_plans(txs, from, nonce_block)?;
        Ok(TxSubmissionPlan::summarize_batch(&plans))
    }

    pub fn account_registration_plans(
        &mut self,
        owner: alloy_primitives::Address,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<Vec<TxSubmissionPlan>, JsonRpcExecutorError<T::Error>> {
        let transactions = self.context().account_onboarding(owner).transactions();
        self.preflight_transaction_plans(&transactions, from, nonce_block)
    }

    pub fn account_registration_summary(
        &mut self,
        owner: alloy_primitives::Address,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxSubmissionPlanBatchSummary, JsonRpcExecutorError<T::Error>> {
        let transactions = self.context().account_onboarding(owner).transactions();
        self.preflight_transaction_summary(&transactions, from, nonce_block)
    }

    pub fn collateral_deposit_plans(
        &mut self,
        account_id: u128,
        amount: u128,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<Vec<TxSubmissionPlan>, JsonRpcExecutorError<T::Error>> {
        let transactions = self
            .context()
            .collateral_deposit(account_id, amount)
            .transactions();
        self.preflight_transaction_plans(&transactions, from, nonce_block)
    }

    pub fn collateral_deposit_summary(
        &mut self,
        account_id: u128,
        amount: u128,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxSubmissionPlanBatchSummary, JsonRpcExecutorError<T::Error>> {
        let transactions = self
            .context()
            .collateral_deposit(account_id, amount)
            .transactions();
        self.preflight_transaction_summary(&transactions, from, nonce_block)
    }

    pub fn collateral_withdrawal_plans(
        &mut self,
        account_id: u128,
        amount: u128,
        recipient: alloy_primitives::Address,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<Vec<TxSubmissionPlan>, JsonRpcExecutorError<T::Error>> {
        let transactions = self
            .context()
            .collateral_withdraw(account_id, amount, recipient)
            .transactions();
        self.preflight_transaction_plans(&transactions, from, nonce_block)
    }

    pub fn collateral_withdrawal_summary(
        &mut self,
        account_id: u128,
        amount: u128,
        recipient: alloy_primitives::Address,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxSubmissionPlanBatchSummary, JsonRpcExecutorError<T::Error>> {
        let transactions = self
            .context()
            .collateral_withdraw(account_id, amount, recipient)
            .transactions();
        self.preflight_transaction_summary(&transactions, from, nonce_block)
    }

    pub fn submit_order_summary(
        &mut self,
        signed_order: SignedOrder,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxSubmissionPlanBatchSummary, TangentClientPreflightSummaryError<T::Error>> {
        let plan = self
            .context()
            .order_lifecycle(signed_order)
            .ok_or(TangentContextError::MissingOrderBook)?;
        Ok(self.preflight_transaction_summary(&[plan.submit_tx()], from, nonce_block)?)
    }

    pub fn cancel_order_summary(
        &mut self,
        signed_order: SignedOrder,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxSubmissionPlanBatchSummary, TangentClientPreflightSummaryError<T::Error>> {
        let plan = self
            .context()
            .order_lifecycle(signed_order)
            .ok_or(TangentContextError::MissingOrderBook)?;
        Ok(self.preflight_transaction_summary(&[plan.cancel_tx()], from, nonce_block)?)
    }

    pub fn tick_orderbook_summary(
        &mut self,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxSubmissionPlanBatchSummary, TangentClientPreflightSummaryError<T::Error>> {
        let plan = self
            .context()
            .orderbook_maintenance()
            .ok_or(TangentContextError::MissingOrderBook)?;
        Ok(self.preflight_transaction_summary(&[plan.tick_tx()], from, nonce_block)?)
    }

    pub fn liquidation_dry_run(
        &mut self,
        account_id: u128,
        market_id: u128,
        read_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TangentLiquidationDryRun, TangentLiquidationDryRunError<T::Error>> {
        let plan = self
            .context()
            .liquidation(account_id, market_id)
            .ok_or(TangentContextError::MissingLiquidationKeeper)?;
        let status = self
            .read_plan(&plan, read_block)
            .map_err(TangentLiquidationDryRunError::Read)?;
        let readiness = status.readiness();
        let transaction_summary = if readiness == crate::LiquidationReadiness::Ready {
            Some(self.preflight_transaction_summary(&[plan.liquidate_tx()], from, nonce_block)?)
        } else {
            None
        };

        Ok(TangentLiquidationDryRun {
            candidate: TangentKeeperLiquidationCandidate::new(account_id, market_id),
            status,
            readiness,
            transaction_summary,
        })
    }

    pub fn liquidation_dry_run_batch(
        &mut self,
        candidates: &[TangentKeeperLiquidationCandidate],
        read_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TangentLiquidationDryRunBatch, TangentLiquidationDryRunError<T::Error>> {
        let mut reports = Vec::with_capacity(candidates.len());
        let mut ready_transactions = Vec::new();
        let mut ready_report_indexes = Vec::new();

        for candidate in candidates {
            let plan = self
                .context()
                .liquidation(candidate.account_id, candidate.market_id)
                .ok_or(TangentContextError::MissingLiquidationKeeper)?;
            let status = self
                .read_plan(&plan, read_block)
                .map_err(TangentLiquidationDryRunError::Read)?;
            let readiness = status.readiness();

            if readiness == crate::LiquidationReadiness::Ready {
                ready_report_indexes.push(reports.len());
                ready_transactions.push(plan.liquidate_tx());
            }

            reports.push(TangentLiquidationDryRun {
                candidate: *candidate,
                status,
                readiness,
                transaction_summary: None,
            });
        }

        let ready_transaction_summary =
            self.preflight_transaction_summary(&ready_transactions, from, nonce_block)?;

        for (offset, report_index) in ready_report_indexes.into_iter().enumerate() {
            reports[report_index].transaction_summary = Some(single_plan_batch_summary(
                ready_transaction_summary.plans[offset].clone(),
            ));
        }

        let ready = reports
            .iter()
            .filter(|report| report.readiness == crate::LiquidationReadiness::Ready)
            .count();
        let blocked = reports.len().saturating_sub(ready);

        Ok(TangentLiquidationDryRunBatch {
            candidates: candidates.len(),
            ready,
            blocked,
            ready_transaction_summary,
            reports,
        })
    }

    pub fn projection_liquidation_dry_run_batch(
        &mut self,
        projection: &TangentEventProjection,
        read_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TangentLiquidationDryRunBatch, TangentLiquidationDryRunError<T::Error>> {
        let candidates = projection
            .account_market_keys()
            .into_iter()
            .map(TangentKeeperLiquidationCandidate::from)
            .collect::<Vec<_>>();
        self.liquidation_dry_run_batch(&candidates, read_block, from, nonce_block)
    }

    pub fn active_projection_liquidation_dry_run_batch(
        &mut self,
        projection: &TangentEventProjection,
        read_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TangentLiquidationDryRunBatch, TangentLiquidationDryRunError<T::Error>> {
        let candidates = projection
            .active_account_market_keys()
            .into_iter()
            .map(TangentKeeperLiquidationCandidate::from)
            .collect::<Vec<_>>();
        self.liquidation_dry_run_batch(&candidates, read_block, from, nonce_block)
    }
}

impl<T, S> TangentClientWorkflow<T, S> {
    #[must_use]
    pub const fn plan(&self) -> &TangentClientPlan {
        &self.plan
    }

    #[must_use]
    pub const fn workflow(&self) -> &TxWorkflowExecutor<T, S> {
        &self.workflow
    }

    #[must_use]
    pub fn workflow_mut(&mut self) -> &mut TxWorkflowExecutor<T, S> {
        &mut self.workflow
    }

    pub fn keeper_polling_preview(
        &self,
        state: &TangentKeeperPollingState,
        liquidation_candidates: &[TangentKeeperLiquidationCandidate],
    ) -> Result<TangentKeeperPollingPreview, crate::EventQueryError> {
        self.plan
            .keeper_polling_preview(state, liquidation_candidates)
    }

    #[must_use]
    pub fn into_parts(self) -> (TangentClientPlan, TxWorkflowExecutor<T, S>) {
        (self.plan, self.workflow)
    }
}

impl<T, S> TangentClientWorkflow<T, S>
where
    T: JsonRpcTransport,
    S: RawTransactionSigner,
{
    pub fn preflight_sign_and_submit(
        &mut self,
        tx: &UnsignedTx,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxWorkflowSubmission, TxWorkflowError<T::Error, S::Error>> {
        self.workflow.preflight_sign_and_submit_with_fee_policy(
            tx,
            from,
            nonce_block,
            self.plan.config.policies.fee,
            self.plan.config.policies.confirmation,
        )
    }

    pub fn preflight_sign_and_submit_batch(
        &mut self,
        txs: &[UnsignedTx],
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxWorkflowBatchSubmission, TxWorkflowError<T::Error, S::Error>> {
        self.workflow
            .preflight_sign_and_submit_batch_with_fee_policy(
                txs,
                from,
                nonce_block,
                self.plan.config.policies.fee,
                self.plan.config.policies.confirmation,
            )
    }

    pub fn register_account(
        &mut self,
        owner: alloy_primitives::Address,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxWorkflowBatchSubmission, TxWorkflowError<T::Error, S::Error>> {
        let transactions = self.plan.context.account_onboarding(owner).transactions();
        self.preflight_sign_and_submit_batch(&transactions, from, nonce_block)
    }

    pub fn deposit_collateral(
        &mut self,
        account_id: u128,
        amount: u128,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxWorkflowBatchSubmission, TxWorkflowError<T::Error, S::Error>> {
        let transactions = self
            .plan
            .context
            .collateral_deposit(account_id, amount)
            .transactions();
        self.preflight_sign_and_submit_batch(&transactions, from, nonce_block)
    }

    pub fn withdraw_collateral(
        &mut self,
        account_id: u128,
        amount: u128,
        recipient: alloy_primitives::Address,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxWorkflowBatchSubmission, TxWorkflowError<T::Error, S::Error>> {
        let transactions = self
            .plan
            .context
            .collateral_withdraw(account_id, amount, recipient)
            .transactions();
        self.preflight_sign_and_submit_batch(&transactions, from, nonce_block)
    }

    pub fn submit_order(
        &mut self,
        signed_order: SignedOrder,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxWorkflowSubmission, TangentOrderLifecycleSubmitError<T::Error, S::Error>> {
        let plan = self
            .plan
            .context
            .order_lifecycle(signed_order)
            .ok_or(TangentContextError::MissingOrderBook)?;
        self.preflight_sign_and_submit(&plan.submit_tx(), from, nonce_block)
            .map_err(Into::into)
    }

    pub fn cancel_order(
        &mut self,
        signed_order: SignedOrder,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxWorkflowSubmission, TangentOrderLifecycleSubmitError<T::Error, S::Error>> {
        let plan = self
            .plan
            .context
            .order_lifecycle(signed_order)
            .ok_or(TangentContextError::MissingOrderBook)?;
        self.preflight_sign_and_submit(&plan.cancel_tx(), from, nonce_block)
            .map_err(Into::into)
    }

    pub fn tick_orderbook(
        &mut self,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TxWorkflowSubmission, TangentKeeperWorkflowError<T::Error, S::Error>> {
        let plan = self
            .plan
            .context
            .orderbook_maintenance()
            .ok_or(TangentContextError::MissingOrderBook)?;
        self.preflight_sign_and_submit(&plan.tick_tx(), from, nonce_block)
            .map_err(Into::into)
    }

    pub fn liquidate_if_ready(
        &mut self,
        account_id: u128,
        market_id: u128,
        read_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TangentLiquidationSubmission, TangentKeeperWorkflowError<T::Error, S::Error>> {
        let plan = self
            .plan
            .context
            .liquidation(account_id, market_id)
            .ok_or(TangentContextError::MissingLiquidationKeeper)?;
        let returns = self
            .workflow
            .rpc_mut()
            .call_batch(&plan.calls(), read_block)
            .map_err(TangentReadPlanExecutionError::Rpc)?;
        let status = plan
            .decode_return_batch(&returns)
            .map_err(TangentReadPlanExecutionError::Decode)?;

        if status.readiness() != crate::LiquidationReadiness::Ready {
            return Err(TangentKeeperWorkflowError::NotLiquidatable(status));
        }

        let submission = self.preflight_sign_and_submit(&plan.liquidate_tx(), from, nonce_block)?;
        Ok(TangentLiquidationSubmission { status, submission })
    }

    pub fn execute_keeper_polling_pass(
        &mut self,
        snapshot: KeeperPollingSnapshot,
        liquidation_candidates: &[TangentKeeperLiquidationCandidate],
        read_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TangentKeeperPollingExecution, TangentKeeperPollingExecutionError<T::Error, S::Error>>
    {
        self.execute_keeper_polling_pass_with_projection(
            snapshot,
            TangentEventProjection::default(),
            liquidation_candidates,
            read_block,
            from,
            nonce_block,
        )
    }

    pub fn execute_keeper_polling_pass_with_projection(
        &mut self,
        snapshot: KeeperPollingSnapshot,
        mut projection: TangentEventProjection,
        liquidation_candidates: &[TangentKeeperLiquidationCandidate],
        read_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<TangentKeeperPollingExecution, TangentKeeperPollingExecutionError<T::Error, S::Error>>
    {
        let plan = self.plan.keeper_polling_plan(snapshot)?;
        let filters = self.plan.context.event_filters();
        let mut event_records = DecodedTangentLogRecords::default();
        let mut latest_event_cursor = None;

        for query in &plan.event_queries {
            let logs = self.workflow.rpc_mut().logs(query)?;
            if let Some(cursor) = latest_log_cursor_after(&logs, snapshot.event_cursor) {
                latest_event_cursor = Some(
                    latest_event_cursor.map_or(cursor, |latest| std::cmp::max(latest, cursor)),
                );
            }

            let decoded_records = match snapshot.event_cursor {
                Some(cursor) => filters.decode_log_records_after_cursor(&logs, cursor)?,
                None => filters.decode_log_records(&logs)?,
            };
            event_records.extend(decoded_records);
        }

        let events = event_records.clone().into_decoded_logs();
        projection.apply_records(&event_records)?;
        let derived_liquidation_candidates = projection
            .account_market_keys()
            .into_iter()
            .map(TangentKeeperLiquidationCandidate::from)
            .collect::<Vec<_>>();

        let mut liquidation_results = Vec::new();
        let mut ready_liquidation_result_indexes = Vec::new();
        let mut transactions = plan.maintenance_transactions.clone();
        let maintenance_transaction_count = transactions.len();

        if plan.should_scan_liquidations {
            let scan_candidates = liquidation_candidates
                .iter()
                .copied()
                .chain(derived_liquidation_candidates.iter().copied())
                .collect::<BTreeSet<_>>();

            for candidate in scan_candidates {
                let liquidation_plan = self
                    .plan
                    .context
                    .liquidation(candidate.account_id, candidate.market_id)
                    .ok_or(TangentContextError::MissingLiquidationKeeper)?;
                let returns = self
                    .workflow
                    .rpc_mut()
                    .call_batch(&liquidation_plan.calls(), read_block)
                    .map_err(TangentReadPlanExecutionError::Rpc)?;
                let status = liquidation_plan
                    .decode_return_batch(&returns)
                    .map_err(TangentReadPlanExecutionError::Decode)?;

                let result_index = liquidation_results.len();
                if status.readiness() == crate::LiquidationReadiness::Ready {
                    transactions.push(liquidation_plan.liquidate_tx());
                    ready_liquidation_result_indexes.push(result_index);
                }

                liquidation_results.push(TangentKeeperLiquidationScanResult {
                    candidate,
                    status,
                    submission: None,
                });
            }
        }

        let maintenance_submission = if transactions.is_empty() {
            None
        } else {
            let batch = self.preflight_sign_and_submit_batch(&transactions, from, nonce_block)?;
            let maintenance_submissions =
                batch.submissions[..maintenance_transaction_count].to_vec();

            for (offset, result_index) in ready_liquidation_result_indexes.into_iter().enumerate() {
                let submission_index = maintenance_transaction_count + offset;
                liquidation_results[result_index].submission =
                    Some(batch.submissions[submission_index].clone());
            }

            if maintenance_submissions.is_empty() {
                None
            } else {
                Some(TxWorkflowBatchSubmission {
                    submissions: maintenance_submissions,
                })
            }
        };

        let mut outcome = KeeperPollingOutcome::at_block(snapshot.current_block);
        if let Some(cursor) = latest_event_cursor {
            outcome = outcome.with_latest_event_cursor(cursor);
        }
        if !plan.maintenance_transactions.is_empty() {
            outcome = outcome.with_completed_maintenance();
        }
        if plan.should_scan_liquidations {
            outcome = outcome.with_completed_liquidation_scan();
        }

        Ok(TangentKeeperPollingExecution {
            plan,
            event_records,
            events,
            projection,
            derived_liquidation_candidates,
            maintenance_submission,
            liquidation_results,
            outcome,
        })
    }

    pub fn execute_keeper_polling_state(
        &mut self,
        state: TangentKeeperPollingState,
        liquidation_candidates: &[TangentKeeperLiquidationCandidate],
        read_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<
        TangentKeeperPollingStateExecution,
        TangentKeeperPollingExecutionError<T::Error, S::Error>,
    > {
        let snapshot = state.snapshot;
        let execution = self.execute_keeper_polling_pass_with_projection(
            state.snapshot,
            state.projection,
            liquidation_candidates,
            read_block,
            from,
            nonce_block,
        )?;
        let next_state = execution.next_state(snapshot);

        Ok(TangentKeeperPollingStateExecution {
            execution,
            next_state,
        })
    }

    pub fn submit_raw_plan(
        &mut self,
        plan: &TxSubmissionPlan,
    ) -> Result<TxWorkflowSubmission, TxWorkflowError<T::Error, S::Error>> {
        self.workflow.submit_raw_plan(plan)
    }

    pub fn submit_raw_plans(
        &mut self,
        plans: &[TxSubmissionPlan],
    ) -> Result<TxWorkflowBatchSubmission, TxWorkflowError<T::Error, S::Error>> {
        self.workflow.submit_raw_plans(plans)
    }

    #[must_use]
    pub fn resume_raw_plan_batch(
        &self,
        original_plans: &[TxSubmissionPlan],
        submitted: &TxWorkflowBatchSubmission,
    ) -> TxWorkflowBatchResumePlan {
        submitted.resume_plan(original_plans)
    }

    #[must_use]
    pub fn resume_raw_plan_batch_summary(
        &self,
        original_plans: &[TxSubmissionPlan],
        submitted: &TxWorkflowBatchSubmission,
    ) -> TxWorkflowBatchResumePlanSummary {
        self.resume_raw_plan_batch(original_plans, submitted)
            .summary()
    }

    pub fn confirmation_snapshot(
        &mut self,
        submission: &TxWorkflowSubmission,
    ) -> Result<TxConfirmationSnapshot, TxWorkflowError<T::Error, S::Error>> {
        self.workflow.confirmation_snapshot(submission)
    }

    pub fn confirmation_report(
        &mut self,
        submission: &TxWorkflowSubmission,
    ) -> Result<TxConfirmationSnapshotReport, TxWorkflowError<T::Error, S::Error>> {
        self.workflow.confirmation_report(submission)
    }

    pub fn confirmation_snapshots(
        &mut self,
        batch: &TxWorkflowBatchSubmission,
    ) -> Result<Vec<TxConfirmationSnapshot>, TxWorkflowError<T::Error, S::Error>> {
        self.workflow.confirmation_snapshots(batch)
    }

    pub fn confirmation_batch_snapshot(
        &mut self,
        batch: &TxWorkflowBatchSubmission,
    ) -> Result<TxConfirmationBatchSnapshot, TxWorkflowError<T::Error, S::Error>> {
        self.workflow.confirmation_batch_snapshot(batch)
    }

    pub fn confirmation_batch_report(
        &mut self,
        batch: &TxWorkflowBatchSubmission,
    ) -> Result<TxConfirmationBatchReport, TxWorkflowError<T::Error, S::Error>> {
        self.workflow.confirmation_batch_report(batch)
    }
}

impl<T, S> TangentClientWorkflow<T, S>
where
    T: JsonRpcTransport,
    S: OrderSigner + RawTransactionSigner<Error = <S as OrderSigner>::Error>,
{
    pub fn place_order(
        &mut self,
        params: OrderParams,
        current_timestamp: u64,
        market_block: crate::RpcBlockTag,
        from: alloy_primitives::Address,
        nonce_block: crate::RpcBlockTag,
    ) -> Result<
        TangentOrderPlacementSubmission,
        TangentOrderPlacementSubmitError<T::Error, <S as OrderSigner>::Error>,
    > {
        let placement_plan = self
            .plan
            .context
            .order_placement(params, current_timestamp)?;
        let market_calls = placement_plan.market_calls();
        let market_returns = self
            .workflow
            .rpc_mut()
            .call_batch(&market_calls, market_block)
            .map_err(TangentReadPlanExecutionError::Rpc)?;
        let market = placement_plan
            .market_plan
            .decode_return_batch(&market_returns)
            .map_err(TangentReadPlanExecutionError::Decode)?;
        let placement = placement_plan.sign_with(&market, self.workflow.signer_mut())?;
        let submission =
            self.preflight_sign_and_submit(&placement.submit_tx(), from, nonce_block)?;
        Ok(TangentOrderPlacementSubmission {
            placement,
            submission,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContractAddresses, JsonRpcRequest, JsonRpcResponse, NetworkConstants, Order,
        OrderSignature, RawTransactionSigningRequest, RpcBlockTag, SignedRawTransaction,
        SignerBackendKind, TxHash, TxSubmissionPlan, UnsignedTxRequest, BASE_SCALE, PRICE_SCALE,
    };
    use alloy_primitives::{Address, B256};
    use serde::de::DeserializeOwned;
    use serde_json::json;
    use std::collections::VecDeque;

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

    fn signed_order() -> SignedOrder {
        SignedOrder {
            order: Order::new(
                7,
                1,
                true,
                65_000 * PRICE_SCALE,
                BASE_SCALE,
                1,
                1_717_000_000,
                false,
            ),
            signature: OrderSignature::from_bytes([1u8; OrderSignature::LEN]).expect("signature"),
        }
    }

    fn topic_u128(value: u128) -> Vec<u8> {
        let mut out = vec![0u8; 32];
        out[16..32].copy_from_slice(&value.to_be_bytes());
        out
    }

    fn topic_address(address: Address) -> Vec<u8> {
        let mut out = vec![0u8; 32];
        out[12..32].copy_from_slice(address.as_slice());
        out
    }

    #[derive(Debug, Clone, Default)]
    struct MockTransport {
        responses: VecDeque<serde_json::Value>,
        seen: Vec<JsonRpcRequest>,
    }

    impl MockTransport {
        fn new(responses: impl IntoIterator<Item = serde_json::Value>) -> Self {
            Self {
                responses: responses.into_iter().collect(),
                seen: Vec::new(),
            }
        }
    }

    impl JsonRpcTransport for MockTransport {
        type Error = String;

        fn send<T: DeserializeOwned + Default>(
            &mut self,
            request: &JsonRpcRequest,
        ) -> Result<JsonRpcResponse<T>, Self::Error> {
            self.seen.push(request.clone());
            let response = self
                .responses
                .pop_front()
                .ok_or_else(|| format!("missing response for {}", request.method))?;
            serde_json::from_value(response).map_err(|error| error.to_string())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockRawSigner {
        seen: Vec<RawTransactionSigningRequest>,
    }

    impl RawTransactionSigner for MockRawSigner {
        type Error = String;

        fn sign_transaction(
            &mut self,
            request: &RawTransactionSigningRequest,
        ) -> Result<SignedRawTransaction, Self::Error> {
            self.seen.push(request.clone());
            SignedRawTransaction::from_hex("0x02abcd").map_err(|error| error.to_string())
        }
    }

    impl OrderSigner for MockRawSigner {
        type Error = String;

        fn sign_order(
            &mut self,
            _request: &crate::OrderSigningRequest,
        ) -> Result<OrderSignature, Self::Error> {
            OrderSignature::from_bytes([3u8; OrderSignature::LEN])
                .map_err(|error| error.to_string())
        }
    }

    fn market_return(paused: bool) -> Vec<u8> {
        fn word_u128(value: u128) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[16..].copy_from_slice(&value.to_be_bytes());
            out
        }

        let mut data = Vec::new();
        data.extend_from_slice(&word_u128(288));
        let mut price_feed = [0u8; 32];
        price_feed[12..].copy_from_slice(Address::repeat_byte(0x11).as_slice());
        data.extend_from_slice(&price_feed);
        data.extend_from_slice(&word_u128(1_000));
        data.extend_from_slice(&word_u128(500));
        data.extend_from_slice(&word_u128(10));
        data.extend_from_slice(&word_u128(100));
        data.extend_from_slice(&word_u128(1_000_000_000_000_000));
        data.extend_from_slice(&word_u128(60));
        data.extend_from_slice(&word_u128(u128::from(paused)));
        data.extend_from_slice(&word_u128(3));
        let mut symbol = [0u8; 32];
        symbol[..3].copy_from_slice(b"BTC");
        data.extend_from_slice(&symbol);
        data
    }

    fn bool_return(value: bool) -> Vec<u8> {
        let mut out = vec![0u8; 32];
        out[31] = u8::from(value);
        out
    }

    fn liquidation_state_return(
        is_liquidatable: bool,
        equity: i128,
        maintenance_margin: u128,
    ) -> Vec<u8> {
        let mut predicate = [0u8; 32];
        predicate[31] = u8::from(is_liquidatable);

        let mut equity_word = if equity < 0 { [0xffu8; 32] } else { [0u8; 32] };
        equity_word[16..].copy_from_slice(&equity.to_be_bytes());

        let mut maintenance = [0u8; 32];
        maintenance[16..].copy_from_slice(&maintenance_margin.to_be_bytes());

        let mut data = Vec::new();
        data.extend_from_slice(&predicate);
        data.extend_from_slice(&equity_word);
        data.extend_from_slice(&maintenance);
        data
    }

    fn order_params() -> OrderParams {
        OrderParams {
            account_id: 7,
            market_id: 1,
            side: crate::Side::Buy,
            limit_price: 65_000 * PRICE_SCALE,
            size: BASE_SCALE,
            nonce: 1,
            expiry: 1_717_000_000,
            reduce_only: false,
        }
    }

    fn account_registered_log_at(
        manifest: &DeploymentManifest,
        block_number: u64,
        log_index: u64,
        registered_at: u64,
    ) -> RawLog {
        RawLog::new(
            manifest.contracts.account_manager,
            vec![
                crate::AccountRegisteredEvent::topic0(),
                B256::from_slice(&topic_u128(7)),
                B256::from_slice(&topic_address(manifest.deployer)),
            ],
            topic_u128(u128::from(registered_at)),
        )
        .with_metadata(crate::RawLogMetadata::new(
            Some(block_number),
            Some(B256::repeat_byte(0xab)),
            Some(log_index),
        ))
    }

    fn account_registered_log(manifest: &DeploymentManifest, log_index: u64) -> RawLog {
        account_registered_log_at(manifest, 123, log_index, 123)
    }

    fn order_submitted_log_at(
        manifest: &DeploymentManifest,
        block_number: u64,
        log_index: u64,
        order_hash: B256,
        account_id: u128,
        market_id: u128,
    ) -> RawLog {
        let mut data = Vec::new();
        data.extend_from_slice(&topic_u128(1));
        data.extend_from_slice(&topic_u128(65_000 * PRICE_SCALE));
        data.extend_from_slice(&topic_u128(BASE_SCALE));

        RawLog::new(
            manifest.contracts.order_book.expect("orderbook"),
            vec![
                crate::OrderSubmittedEvent::topic0(),
                order_hash,
                B256::from_slice(&topic_u128(account_id)),
                B256::from_slice(&topic_u128(market_id)),
            ],
            data,
        )
        .with_metadata(crate::RawLogMetadata::new(
            Some(block_number),
            Some(B256::repeat_byte(0xac)),
            Some(log_index),
        ))
    }

    #[test]
    fn endpoint_config_validates_scheme_and_headers() {
        assert_eq!(
            RpcEndpointConfig::new(""),
            Err(TangentClientConfigError::EmptyEndpoint)
        );
        assert_eq!(
            RpcEndpointConfig::new("ftp://example.invalid"),
            Err(TangentClientConfigError::UnsupportedEndpointScheme(
                "ftp".to_owned()
            ))
        );
        assert_eq!(
            RpcEndpointConfig::new("localhost:8545"),
            Err(TangentClientConfigError::UnsupportedEndpointScheme(
                "localhost".to_owned()
            ))
        );

        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid")
            .expect("endpoint")
            .with_header("Authorization", "Bearer secret")
            .expect("header");

        assert_eq!(endpoint.scheme(), Some("https"));
        assert!(endpoint.is_secure());
        assert_eq!(endpoint.headers.len(), 1);
        assert_eq!(endpoint.redacted().headers[0].value, "<redacted>");
        let endpoint_report = endpoint.report();
        assert_eq!(endpoint_report.static_rpc_headers, 1);
        assert_eq!(endpoint_report.static_rpc_auth_headers, 1);
        assert!(endpoint_report.has_static_rpc_auth_header);
        assert_eq!(
            endpoint.clone().with_header("", "value"),
            Err(TangentClientConfigError::EmptyHeaderName)
        );

        let api_key_endpoint = RpcEndpointConfig::new("wss://rpc.example.invalid")
            .expect("endpoint")
            .with_header("X-Api-Key", "secret")
            .expect("api key header")
            .with_header("X-Trace-Id", "trace")
            .expect("trace header");
        let api_key_report = api_key_endpoint.report();
        assert_eq!(api_key_report.static_rpc_headers, 2);
        assert_eq!(api_key_report.static_rpc_auth_headers, 1);
        assert!(api_key_report.has_static_rpc_auth_header);
    }

    #[test]
    fn client_plan_packages_manifest_endpoint_and_default_policies() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");

        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");

        assert_eq!(plan.context().manifest(), &manifest);
        assert_eq!(plan.config().chain_id, manifest.chain_id);
        assert!(plan.chain_id_matches_manifest());
        assert_eq!(plan.config().policies.retry.max_attempts, 3);
        assert!(plan.config().signer_backends.is_empty());
        assert_eq!(plan.config().policies.backoff.delay_millis(0), 250);
        assert_eq!(
            plan.config().policies.fee,
            TxFeePolicy::Eip1559FromGasPrice {
                max_fee_multiplier: 2,
                min_priority_fee_per_gas: None,
            }
        );
        assert_eq!(
            plan.config().policies.confirmation,
            TxConfirmationPolicy::new(2).with_timeout_blocks(20)
        );
        assert_eq!(plan.keeper_runtime().capabilities().len(), 1);
    }

    #[test]
    fn client_plan_builds_startup_report() {
        let current_plan = TangentClientPlan::new(
            current_manifest(),
            RpcEndpointConfig::new("http://rpc.example.invalid").expect("endpoint"),
        )
        .expect("current plan");
        let current_report = current_plan.startup_report();
        let current_support_report = current_plan.support_report();
        assert_eq!(current_support_report.startup, current_report);
        assert_eq!(current_support_report.config, current_plan.config.report());
        assert_eq!(current_report.endpoint_scheme.as_deref(), Some("http"));
        assert!(!current_report.endpoint_is_secure);
        assert_eq!(current_report.static_rpc_headers, 0);
        assert_eq!(current_report.static_rpc_auth_headers, 0);
        assert!(!current_report.has_static_rpc_auth_header);
        assert_eq!(current_report.signer_backend_count, 0);
        assert!(!current_report.has_signer_backends);
        assert!(current_report.signer_backend_kinds.is_empty());
        assert!(!current_report.has_multiple_signer_backend_kinds);
        assert!(!current_report.perp_stack.is_complete());
        assert_eq!(
            current_report.missing_perp_contracts,
            vec!["OrderBook", "SettlementEngine", "LiquidationKeeper"]
        );
        assert_eq!(
            current_report.keeper_capabilities,
            vec![KeeperCapability::EventIndexing]
        );
        assert!(current_report.readiness.primitive_reads);
        assert!(current_report.readiness.keeper_polling);
        assert!(!current_report.readiness.orderbook_workflows);
        assert!(!current_report.readiness.settlement_reads);
        assert!(!current_report.readiness.liquidation_reads);
        assert!(!current_report.readiness.full_perp_stack);
        assert_eq!(
            current_report.readiness.blocking_reasons,
            vec![
                "missing OrderBook",
                "missing SettlementEngine",
                "missing LiquidationKeeper"
            ]
        );

        let mismatch_report = TangentClientPlan {
            context: TangentContext::new(current_manifest()),
            config: TangentClientConfig::new(
                RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint"),
                1,
            )
            .expect("mismatch config"),
        }
        .startup_report();
        assert!(!mismatch_report.readiness.primitive_reads);
        assert!(!mismatch_report.readiness.keeper_polling);
        assert!(mismatch_report
            .readiness
            .blocking_reasons
            .contains(&"configured chain id does not match manifest".to_owned()));

        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("wss://rpc.example.invalid")
            .expect("endpoint")
            .with_header("Authorization", "Bearer secret")
            .expect("header");
        let config = TangentClientConfig::new(endpoint, manifest.chain_id)
            .expect("config")
            .with_signer_backend(SignerBackendConfig {
                kind: SignerBackendKind::CircleDevWallet,
                key_id: "circle-1".to_owned(),
                address: Some(Address::repeat_byte(0x33)),
                metadata: Vec::new(),
            })
            .expect("circle backend")
            .with_signer_backend(SignerBackendConfig {
                kind: SignerBackendKind::Kms,
                key_id: "kms-1".to_owned(),
                address: Some(Address::repeat_byte(0x44)),
                metadata: Vec::new(),
            })
            .expect("kms backend")
            .with_signer_backend(SignerBackendConfig {
                kind: SignerBackendKind::CircleDevWallet,
                key_id: "circle-2".to_owned(),
                address: Some(Address::repeat_byte(0x55)),
                metadata: Vec::new(),
            })
            .expect("second circle backend");
        let plan = TangentClientPlan {
            context: TangentContext::new(manifest.clone()),
            config,
        };

        let report = plan.startup_report();
        let support_report = plan.support_report();

        assert_eq!(report.project, "Tangent");
        assert_eq!(report.network, "arc-testnet");
        assert_eq!(report.manifest_chain_id, manifest.chain_id);
        assert_eq!(report.configured_chain_id, manifest.chain_id);
        assert!(report.chain_id_matches_manifest);
        assert_eq!(report.endpoint_scheme.as_deref(), Some("wss"));
        assert!(report.endpoint_is_secure);
        assert_eq!(report.static_rpc_headers, 1);
        assert_eq!(report.static_rpc_auth_headers, 1);
        assert!(report.has_static_rpc_auth_header);
        assert_eq!(report.signer_backend_count, 3);
        assert!(report.has_signer_backends);
        assert_eq!(
            report.signer_backend_kinds,
            vec![SignerBackendKind::CircleDevWallet, SignerBackendKind::Kms]
        );
        assert!(report.has_multiple_signer_backend_kinds);
        assert!(report.perp_stack.is_complete());
        assert!(report.missing_perp_contracts.is_empty());
        assert_eq!(
            report.keeper_capabilities,
            vec![
                KeeperCapability::EventIndexing,
                KeeperCapability::OrderBookMaintenance,
                KeeperCapability::SettlementReads,
                KeeperCapability::LiquidationReads,
                KeeperCapability::FullPerpStack,
            ]
        );
        assert_eq!(report.policies, TangentClientPolicies::default());
        assert!(report.readiness.primitive_reads);
        assert!(report.readiness.orderbook_workflows);
        assert!(report.readiness.settlement_reads);
        assert!(report.readiness.liquidation_reads);
        assert!(report.readiness.full_perp_stack);
        assert!(report.readiness.keeper_polling);
        assert!(report.readiness.blocking_reasons.is_empty());

        let json = serde_json::to_string(&report).expect("report serializes");
        let restored: TangentClientStartupReport =
            serde_json::from_str(&json).expect("report deserializes");
        assert_eq!(restored, report);
        let mut legacy_json = serde_json::to_value(&report).expect("startup report value");
        let legacy_object = legacy_json.as_object_mut().expect("startup report object");
        legacy_object.remove("static_rpc_auth_headers");
        legacy_object.remove("has_static_rpc_auth_header");
        legacy_object.remove("has_signer_backends");
        legacy_object.remove("has_multiple_signer_backend_kinds");
        let legacy_report: TangentClientStartupReport =
            serde_json::from_value(legacy_json).expect("legacy startup report deserializes");
        assert_eq!(legacy_report.static_rpc_auth_headers, 0);
        assert!(!legacy_report.has_static_rpc_auth_header);
        assert!(!legacy_report.has_signer_backends);
        assert!(!legacy_report.has_multiple_signer_backend_kinds);

        assert_eq!(support_report.startup, report);
        assert_eq!(support_report.config, plan.config.report());
        assert_eq!(support_report.config.endpoint.static_rpc_headers, 1);
        assert_eq!(support_report.config.endpoint.static_rpc_auth_headers, 1);
        assert!(support_report.config.endpoint.has_static_rpc_auth_header);
        assert_eq!(
            support_report.config.endpoint.static_rpc_header_names,
            vec!["Authorization"]
        );
        assert_eq!(support_report.config.signer_backend_count, 3);
        assert!(support_report.config.has_signer_backends);
        assert!(support_report.config.has_multiple_signer_backend_kinds);
        let support_json =
            serde_json::to_string(&support_report).expect("support report serializes");
        assert!(support_json.contains("Authorization"));
        assert!(!support_json.contains("Bearer secret"));
        let restored_support: TangentClientSupportReport =
            serde_json::from_str(&support_json).expect("support report deserializes");
        assert_eq!(restored_support, support_report);
    }

    #[test]
    fn client_plan_uses_configured_keeper_polling_policy() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("wss://rpc.example.invalid").expect("endpoint");
        let policies = TangentClientPolicies {
            keeper_polling: KeeperPollingPolicy::new(10, 1, 1),
            ..TangentClientPolicies::default()
        };
        let plan =
            TangentClientPlan::with_policies(manifest, endpoint, policies).expect("client plan");

        let polling = plan
            .keeper_polling_plan(KeeperPollingSnapshot::at_block(25).with_event_from_block(1))
            .expect("polling plan");

        assert_eq!(polling.event_queries.len(), 3);
        assert!(polling.maintenance_transactions.is_empty());
        assert!(!polling.should_scan_liquidations);

        let summary = plan
            .keeper_polling_plan_summary(
                KeeperPollingSnapshot::at_block(25).with_event_from_block(1),
            )
            .expect("polling summary");
        assert_eq!(summary.event_query_count, 3);
        assert_eq!(summary.first_event_from_block.as_deref(), Some("0x1"));
        assert_eq!(summary.last_event_to_block.as_deref(), Some("0x19"));
        assert!(summary.maintenance_transactions.is_empty());
        assert!(summary.has_work);

        let client = TangentClient::new(plan, MockTransport::default());
        let facade_summary = client
            .keeper_polling_plan_summary(
                KeeperPollingSnapshot::at_block(25).with_event_from_block(1),
            )
            .expect("client polling summary");
        assert_eq!(facade_summary, summary);
    }

    #[test]
    fn client_plan_summarizes_manifest_event_log_queries() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");

        let summary = plan.event_log_query_summary(Some(100), Some(105));
        assert_eq!(summary.address_count, 3);
        assert_eq!(summary.topic0_count, 9);
        assert_eq!(summary.from_block.as_deref(), Some("0x64"));
        assert_eq!(summary.to_block.as_deref(), Some("0x69"));
        assert!(!summary.is_open_ended);

        let resume_summary =
            plan.resume_event_log_query_summary(RawLogCursor::new(123, 9), Some(130));
        assert_eq!(resume_summary.from_block.as_deref(), Some("0x7b"));
        assert_eq!(resume_summary.to_block.as_deref(), Some("0x82"));

        let chunked = plan
            .chunked_event_log_query_summary(100, 105, 2)
            .expect("chunked summary");
        assert_eq!(chunked.len, 3);
        assert!(!chunked.is_empty);
        assert_eq!(chunked.open_ended_queries, 0);
        assert_eq!(chunked.total_address_filters, 9);
        assert_eq!(chunked.total_topic0_filters, 27);
        assert_eq!(chunked.queries[0].from_block.as_deref(), Some("0x64"));
        assert_eq!(chunked.queries[2].to_block.as_deref(), Some("0x69"));

        let resume_chunked = plan
            .chunked_resume_event_log_query_summary(RawLogCursor::new(123, 9), 126, 2)
            .expect("chunked resume summary");
        assert_eq!(resume_chunked.len, 2);
        assert_eq!(
            resume_chunked
                .queries
                .iter()
                .map(|query| (query.from_block.as_deref(), query.to_block.as_deref()))
                .collect::<Vec<_>>(),
            vec![(Some("0x7b"), Some("0x7c")), (Some("0x7d"), Some("0x7e"))]
        );

        assert_eq!(
            plan.chunked_event_log_query_summary(100, 105, 0)
                .expect_err("zero chunk size"),
            EventQueryError::ZeroChunkSize
        );

        let client = TangentClient::new(plan, MockTransport::new([]));
        assert_eq!(
            client.event_log_query_summary(Some(100), Some(105)),
            summary
        );
        assert_eq!(
            client
                .chunked_resume_event_log_query_summary(RawLogCursor::new(123, 9), 126, 2)
                .expect("facade chunked resume summary"),
            resume_chunked
        );
        let (_, rpc) = client.into_parts();
        assert!(rpc.into_transport().seen.is_empty());

        let json = serde_json::to_string(&chunked).expect("query summary serializes");
        let restored: EventLogRpcQueryBatchSummary =
            serde_json::from_str(&json).expect("query summary deserializes");
        assert_eq!(restored, chunked);
    }

    #[test]
    fn keeper_polling_state_builds_resume_report_from_projection_cursor() {
        let projection = TangentEventProjection {
            last_cursor: Some(RawLogCursor::new(120, 4)),
            ..TangentEventProjection::default()
        };
        let snapshot = KeeperPollingSnapshot::at_block(100)
            .with_event_cursor(RawLogCursor::new(110, 2))
            .with_event_from_block(90)
            .with_last_tick_block(80)
            .with_last_liquidation_scan_block(75);
        let state = TangentKeeperPollingState::new(snapshot, projection);

        let report = state.resume_report_at(130);

        assert_eq!(report.current_block, 130);
        assert_eq!(
            report.effective_event_cursor,
            Some(RawLogCursor::new(120, 4))
        );
        assert!(!report.projection_cursor_is_checkpointed);
        assert_eq!(report.resume_snapshot.current_block, 130);
        assert_eq!(
            report.resume_snapshot.event_cursor,
            Some(RawLogCursor::new(120, 4))
        );
        assert_eq!(report.resume_snapshot.event_from_block, None);
        assert_eq!(report.resume_snapshot.last_tick_block, Some(80));
        assert_eq!(report.resume_snapshot.last_liquidation_scan_block, Some(75));
        assert_eq!(report.checkpoint, state.checkpoint());

        let json = serde_json::to_string(&report).expect("resume report serializes");
        let restored: TangentKeeperPollingResumeReport =
            serde_json::from_str(&json).expect("resume report deserializes");
        assert_eq!(restored, report);
    }

    #[test]
    fn client_config_roundtrips_through_json() {
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid")
            .expect("endpoint")
            .with_header("x-api-key", "secret")
            .expect("header");
        let signer = SignerBackendConfig::new(SignerBackendKind::CircleDevWallet, "wallet-1")
            .expect("signer");
        let config = TangentClientConfig::new(endpoint, 11111)
            .expect("config")
            .with_signer_backend(signer.clone())
            .expect("signer backend");

        let json = serde_json::to_string(&config).expect("serialize config");
        let decoded: TangentClientConfig = serde_json::from_str(&json).expect("decode config");

        assert_eq!(decoded, config);
        assert_eq!(decoded.signer_backends, vec![signer]);
    }

    #[test]
    fn client_config_indexes_and_redacts_signer_backends() {
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid")
            .expect("endpoint")
            .with_header("authorization", "Bearer secret")
            .expect("header");
        let circle = SignerBackendConfig::new(SignerBackendKind::CircleDevWallet, "wallet-1")
            .expect("circle")
            .with_address(Address::repeat_byte(0x44))
            .with_metadata("entity", "entity-id", false)
            .expect("entity")
            .with_metadata("api-key", "secret", true)
            .expect("api key");
        let relayer =
            SignerBackendConfig::new(SignerBackendKind::Relayer, "relayer-1").expect("relayer");
        let config = TangentClientConfig::new(endpoint, 11111)
            .expect("config")
            .with_signer_backends([circle.clone(), relayer.clone()])
            .expect("signer backends");

        assert_eq!(config.signer_backend("wallet-1"), Some(&circle));
        assert_eq!(
            config
                .require_signer_backend("relayer-1")
                .expect("relayer backend"),
            &relayer
        );
        assert_eq!(
            config.signer_backend_for_kind(SignerBackendKind::CircleDevWallet),
            Some(&circle)
        );
        assert_eq!(
            config.signer_backends_for_kind(SignerBackendKind::Relayer),
            vec![&relayer]
        );
        assert_eq!(
            config.signer_backend_for_address(Address::repeat_byte(0x44)),
            Some(&circle)
        );
        assert_eq!(
            config
                .require_signer_backend("missing")
                .expect_err("missing"),
            TangentClientConfigError::MissingSignerBackendKeyId("missing".to_owned())
        );

        let redacted = config.redacted();
        assert_eq!(redacted.endpoint.headers[0].value, "<redacted>");
        assert_eq!(redacted.signer_backends[0].metadata[0].value, "entity-id");
        assert_eq!(redacted.signer_backends[0].metadata[1].value, "<redacted>");
        let report = config.report();
        assert_eq!(report.chain_id, 11111);
        assert_eq!(report.endpoint.scheme.as_deref(), Some("https"));
        assert!(report.endpoint.is_secure);
        assert_eq!(report.endpoint.static_rpc_headers, 1);
        assert_eq!(report.endpoint.static_rpc_auth_headers, 1);
        assert!(report.endpoint.has_static_rpc_auth_header);
        assert_eq!(
            report.endpoint.static_rpc_header_names,
            vec!["authorization".to_owned()]
        );
        assert_eq!(report.signer_backend_count, 2);
        assert!(report.has_signer_backends);
        assert_eq!(
            report.signer_backend_kinds,
            vec![
                SignerBackendKind::CircleDevWallet,
                SignerBackendKind::Relayer
            ]
        );
        assert!(report.has_multiple_signer_backend_kinds);
        assert_eq!(report.signer_backends[0].key_id, "wallet-1");
        assert_eq!(
            report.signer_backends[0].address,
            Some(Address::repeat_byte(0x44))
        );
        assert_eq!(
            report.signer_backends[0].metadata_keys,
            vec!["entity".to_owned(), "api-key".to_owned()]
        );
        assert_eq!(
            report.signer_backends[0].secret_metadata_keys,
            vec!["api-key".to_owned()]
        );
        assert_eq!(report.signer_backends[0].metadata_count, 2);
        assert_eq!(report.signer_backends[0].secret_metadata_count, 1);
        assert!(report.signer_backends[0].has_address);
        assert!(report.signer_backends[0].has_metadata);
        assert!(report.signer_backends[0].has_secret_metadata);
        let report_json = serde_json::to_string(&report).expect("config report serializes");
        assert!(report_json.contains("api-key"));
        assert!(!report_json.contains("entity-id"));
        assert!(!report_json.contains("Bearer secret"));
        let restored_report: TangentClientConfigReport =
            serde_json::from_str(&report_json).expect("config report deserializes");
        assert_eq!(restored_report, report);
        let mut legacy_report_json =
            serde_json::to_value(&report).expect("config report value serializes");
        let endpoint_report = legacy_report_json
            .get_mut("endpoint")
            .and_then(serde_json::Value::as_object_mut)
            .expect("endpoint report object");
        endpoint_report.remove("static_rpc_auth_headers");
        endpoint_report.remove("has_static_rpc_auth_header");
        let legacy_report_object = legacy_report_json
            .as_object_mut()
            .expect("config report object");
        legacy_report_object.remove("has_signer_backends");
        legacy_report_object.remove("has_multiple_signer_backend_kinds");
        let signer_backend_report = legacy_report_object
            .get_mut("signer_backends")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|backends| backends.first_mut())
            .and_then(serde_json::Value::as_object_mut)
            .expect("signer backend report object");
        signer_backend_report.remove("has_address");
        signer_backend_report.remove("has_metadata");
        signer_backend_report.remove("has_secret_metadata");
        let legacy_report: TangentClientConfigReport =
            serde_json::from_value(legacy_report_json).expect("legacy config report deserializes");
        assert_eq!(legacy_report.endpoint.static_rpc_auth_headers, 0);
        assert!(!legacy_report.endpoint.has_static_rpc_auth_header);
        assert!(!legacy_report.has_signer_backends);
        assert!(!legacy_report.has_multiple_signer_backend_kinds);
        assert!(!legacy_report.signer_backends[0].has_address);
        assert!(!legacy_report.signer_backends[0].has_metadata);
        assert!(!legacy_report.signer_backends[0].has_secret_metadata);

        let (adapter_backend, adapter_client) = config
            .external_signer_adapter("wallet-1", "circle-client")
            .expect("adapter from key")
            .into_parts();
        assert_eq!(adapter_backend, circle);
        assert_eq!(adapter_client, "circle-client");

        let (adapter_backend, adapter_client) = config
            .external_signer_adapter_for_kind(SignerBackendKind::Relayer, "relayer-client")
            .expect("adapter from kind")
            .into_parts();
        assert_eq!(adapter_backend, relayer);
        assert_eq!(adapter_client, "relayer-client");

        let (adapter_backend, adapter_client) = config
            .external_signer_adapter_for_address(Address::repeat_byte(0x44), "address-client")
            .expect("adapter from address")
            .into_parts();
        assert_eq!(adapter_backend.key_id, "wallet-1");
        assert_eq!(adapter_client, "address-client");

        assert!(config
            .external_signer_adapter_for_kind(SignerBackendKind::Kms, "missing")
            .is_none());
        assert_eq!(
            config
                .external_signer_adapter("missing", "missing-client")
                .expect_err("missing adapter"),
            TangentClientConfigError::MissingSignerBackendKeyId("missing".to_owned())
        );
    }

    #[test]
    fn client_config_rejects_duplicate_signer_backend_key_ids() {
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let signer = SignerBackendConfig::new(SignerBackendKind::CircleDevWallet, "wallet-1")
            .expect("signer");

        let error = TangentClientConfig::new(endpoint, 11111)
            .expect("config")
            .with_signer_backends([signer.clone(), signer])
            .expect_err("duplicate signer key id");

        assert_eq!(
            error,
            TangentClientConfigError::DuplicateSignerBackendKeyId("wallet-1".to_owned())
        );

        let duplicate_config = TangentClientConfig {
            endpoint: RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint"),
            chain_id: 11111,
            policies: TangentClientPolicies::default(),
            signer_backends: vec![
                SignerBackendConfig::new(SignerBackendKind::Kms, "key-1").expect("signer"),
                SignerBackendConfig::new(SignerBackendKind::Relayer, "key-1").expect("signer"),
            ],
        };

        assert_eq!(
            duplicate_config.validate(),
            Err(TangentClientConfigError::DuplicateSignerBackendKeyId(
                "key-1".to_owned()
            ))
        );
    }

    #[test]
    fn tangent_client_delegates_reads_to_rpc_executor() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let transport = MockTransport::new([json!({"jsonrpc":"2.0","id":1,"result":"0x1234"})]);
        let mut client = TangentClient::new(plan, transport);
        let call = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0xaa],
        };

        let returned = client
            .call(&call, RpcBlockTag::Latest)
            .expect("call returns");

        assert_eq!(returned.data_hex(), "0x1234");
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 1);
        assert_eq!(transport.seen[0].method, "eth_call");
    }

    #[test]
    fn tangent_client_executes_and_decodes_typed_read_plan() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let status_plan = TangentContext::new(manifest).account_status(owner, 7);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(topic_address(owner)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(topic_u128(7)))}),
            json!({"jsonrpc":"2.0","id":3,"result": format!("0x{}", hex::encode(topic_u128(9)))}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let summary = client.read_plan_summary(&status_plan);
        assert_eq!(summary.len, 3);
        assert!(!summary.is_empty);
        assert_eq!(summary.unique_contracts, 1);
        assert_eq!(summary.contracts[0].calls, 3);
        assert_eq!(summary.total_calldata_bytes, 76);
        assert!(summary.calls.iter().all(|call| call.selector.is_some()));

        let status = client
            .read_plan(&status_plan, RpcBlockTag::Latest)
            .expect("read plan executes and decodes");

        assert_eq!(status.owner_of_account, owner);
        assert_eq!(status.account_id_of_owner, 7);
        assert_eq!(status.total_accounts, 9);
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec!["eth_call", "eth_call", "eth_call"]
        );
    }

    #[test]
    fn tangent_client_read_plan_surfaces_decode_errors() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let status_plan =
            TangentContext::new(manifest).account_status(Address::repeat_byte(0x33), 7);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x"}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let error = client
            .read_plan(&status_plan, RpcBlockTag::Latest)
            .expect_err("bad return data fails decode");

        assert!(matches!(
            error,
            TangentReadPlanExecutionError::Decode(AbiDecodeError::InvalidLength { .. })
        ));
    }

    #[test]
    fn tangent_client_account_status_helper_builds_manifest_plan() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(topic_address(owner)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(topic_u128(7)))}),
            json!({"jsonrpc":"2.0","id":3,"result": format!("0x{}", hex::encode(topic_u128(9)))}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let status = client
            .account_status(owner, 7, RpcBlockTag::Latest)
            .expect("account status reads");

        assert!(status.is_registered_binding(owner, 7));
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 3);
        assert!(transport
            .seen
            .iter()
            .all(|request| request.method == "eth_call"));
    }

    #[test]
    fn tangent_client_fetches_and_decodes_manifest_event_logs() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let registered = account_registered_log(&manifest, 9);
        let broad_false_positive = RawLog::new(
            manifest.contracts.account_manager,
            vec![crate::DepositedEvent::topic0()],
            vec![],
        );
        let transport = MockTransport::new([json!({
            "jsonrpc":"2.0",
            "id":1,
            "result":[registered.clone(), broad_false_positive]
        })]);
        let mut client = TangentClient::new(plan, transport);

        let decoded = client
            .decoded_event_logs(Some(100), Some(123))
            .expect("event logs decode");

        assert_eq!(decoded.known_logs(), 1);
        assert_eq!(decoded.unknown_logs, 1);
        assert!(decoded.contains_kind(crate::TangentEventKind::AccountRegistered));
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 1);
        assert_eq!(transport.seen[0].method, "eth_getLogs");
    }

    #[test]
    fn tangent_client_resumes_decoded_event_logs_after_cursor() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let old_log = account_registered_log(&manifest, 9);
        let new_log = account_registered_log(&manifest, 10);
        let cursor = old_log.cursor().expect("old log cursor");
        let transport = MockTransport::new([json!({
            "jsonrpc":"2.0",
            "id":1,
            "result":[old_log, new_log]
        })]);
        let mut client = TangentClient::new(plan, transport);

        let decoded = client
            .resume_decoded_event_logs(cursor, Some(130))
            .expect("resume event logs decode");

        assert_eq!(decoded.known_logs(), 1);
        assert_eq!(decoded.unknown_logs, 0);
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen[0].method, "eth_getLogs");
        let params = &transport.seen[0].params[0];
        assert_eq!(params["fromBlock"], "0x7b");
        assert_eq!(params["toBlock"], "0x82");
    }

    #[test]
    fn tangent_client_fetches_chunked_decoded_event_logs() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let registered_a = account_registered_log_at(&manifest, 100, 9, 100);
        let registered_b = account_registered_log_at(&manifest, 105, 10, 105);
        let false_positive = RawLog::new(
            Address::repeat_byte(0x99),
            vec![crate::AccountRegisteredEvent::topic0()],
            vec![],
        );
        let transport = MockTransport::new([
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "result":[registered_a]
            }),
            json!({
                "jsonrpc":"2.0",
                "id":2,
                "result":[registered_b, false_positive]
            }),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let decoded = client
            .chunked_decoded_event_logs(100, 105, 3)
            .expect("chunked event logs decode");

        assert_eq!(decoded.known_logs(), 2);
        assert_eq!(decoded.unknown_logs, 1);
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 2);
        assert_eq!(transport.seen[0].method, "eth_getLogs");
        assert_eq!(transport.seen[0].params[0]["fromBlock"], "0x64");
        assert_eq!(transport.seen[0].params[0]["toBlock"], "0x66");
        assert_eq!(transport.seen[1].params[0]["fromBlock"], "0x67");
        assert_eq!(transport.seen[1].params[0]["toBlock"], "0x69");
    }

    #[test]
    fn tangent_client_fetches_chunked_resume_decoded_event_logs() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let old_log = account_registered_log_at(&manifest, 123, 9, 123);
        let new_same_block_log = account_registered_log_at(&manifest, 123, 10, 124);
        let later_log = account_registered_log_at(&manifest, 125, 1, 125);
        let cursor = old_log.cursor().expect("old log cursor");
        let transport = MockTransport::new([
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "result":[old_log, new_same_block_log]
            }),
            json!({
                "jsonrpc":"2.0",
                "id":2,
                "result":[later_log]
            }),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let decoded = client
            .chunked_resume_decoded_event_logs(cursor, 126, 2)
            .expect("chunked resume event logs decode");

        assert_eq!(decoded.known_logs(), 2);
        assert_eq!(decoded.unknown_logs, 0);
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 2);
        assert_eq!(transport.seen[0].params[0]["fromBlock"], "0x7b");
        assert_eq!(transport.seen[0].params[0]["toBlock"], "0x7c");
        assert_eq!(transport.seen[1].params[0]["fromBlock"], "0x7d");
        assert_eq!(transport.seen[1].params[0]["toBlock"], "0x7e");
    }

    #[test]
    fn tangent_client_fetches_chunked_decoded_event_log_records() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let old_log = account_registered_log_at(&manifest, 123, 9, 123);
        let new_log = account_registered_log_at(&manifest, 123, 10, 124);
        let later_log = account_registered_log_at(&manifest, 125, 1, 125);
        let cursor = old_log.cursor().expect("old log cursor");
        let transport = MockTransport::new([
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "result":[old_log, new_log]
            }),
            json!({
                "jsonrpc":"2.0",
                "id":2,
                "result":[later_log]
            }),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let records = client
            .chunked_resume_decoded_event_log_records(cursor, 126, 2)
            .expect("chunked resume event log records decode");

        assert_eq!(records.known_logs(), 2);
        assert_eq!(records.last_cursor(), Some(RawLogCursor::new(125, 1)));
        assert_eq!(
            records.records[0].cursor(),
            Some(RawLogCursor::new(123, 10))
        );
        assert_eq!(records.records[1].block_number(), Some(125));
        assert_eq!(records.records[1].log_index(), Some(1));
        assert!(records.contains_kind(crate::TangentEventKind::AccountRegistered));
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 2);
        assert_eq!(transport.seen[0].method, "eth_getLogs");
    }

    #[test]
    fn tangent_client_projects_chunked_event_log_records() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let registered = account_registered_log_at(&manifest, 105, 10, 105);
        let transport = MockTransport::new([json!({
            "jsonrpc":"2.0",
            "id":1,
            "result":[registered]
        })]);
        let mut client = TangentClient::new(plan, transport);

        let projection = client
            .chunked_event_projection(100, 105, 10)
            .expect("chunked projection builds");

        let account = projection.accounts.get(&7).expect("projected account");
        assert_eq!(account.owner, Some(manifest.deployer));
        assert_eq!(account.registered_at, Some(105));
        assert_eq!(projection.summary().accounts, 1);
        assert_eq!(projection.summary().applied_records, 1);
        assert_eq!(projection.last_cursor, Some(RawLogCursor::new(105, 10)));
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 1);
        assert_eq!(transport.seen[0].method, "eth_getLogs");
    }

    #[test]
    fn tangent_client_projects_chunked_resume_event_log_records() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let old_log = account_registered_log_at(&manifest, 123, 9, 123);
        let new_log = account_registered_log_at(&manifest, 123, 10, 124);
        let later_log = account_registered_log_at(&manifest, 125, 1, 125);
        let cursor = old_log.cursor().expect("old log cursor");
        let transport = MockTransport::new([
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "result":[old_log, new_log]
            }),
            json!({
                "jsonrpc":"2.0",
                "id":2,
                "result":[later_log]
            }),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let projection = client
            .chunked_resume_event_projection(cursor, 126, 2)
            .expect("chunked resume projection builds");

        let account = projection.accounts.get(&7).expect("projected account");
        assert_eq!(account.registered_at, Some(125));
        assert_eq!(projection.summary().applied_records, 2);
        assert_eq!(projection.last_cursor, Some(RawLogCursor::new(125, 1)));
        assert_eq!(
            projection
                .account_market_keys()
                .into_iter()
                .map(TangentKeeperLiquidationCandidate::from)
                .collect::<Vec<_>>(),
            Vec::<TangentKeeperLiquidationCandidate>::new()
        );
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 2);
        assert_eq!(transport.seen[0].params[0]["fromBlock"], "0x7b");
        assert_eq!(transport.seen[1].params[0]["fromBlock"], "0x7d");
    }

    #[test]
    fn tangent_client_chunked_event_logs_surface_query_errors() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let transport = MockTransport::default();
        let mut client = TangentClient::new(plan, transport);

        let error = client
            .chunked_decoded_event_logs(100, 105, 0)
            .expect_err("zero chunk size fails query planning");

        assert!(matches!(
            error,
            TangentClientEventLogError::Query(EventQueryError::ZeroChunkSize)
        ));
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert!(transport.seen.is_empty());
    }

    #[test]
    fn tangent_client_event_log_helper_surfaces_decode_errors() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let malformed_known_log = RawLog::new(
            manifest.contracts.account_manager,
            vec![
                crate::AccountRegisteredEvent::topic0(),
                B256::from_slice(&topic_u128(7)),
                B256::from_slice(&topic_address(manifest.deployer)),
            ],
            vec![],
        );
        let transport = MockTransport::new([json!({
            "jsonrpc":"2.0",
            "id":1,
            "result":[malformed_known_log]
        })]);
        let mut client = TangentClient::new(plan, transport);

        let error = client
            .decoded_event_logs(None, None)
            .expect_err("malformed exact event fails decode");

        assert!(matches!(error, TangentClientEventLogError::Decode(_)));
    }

    #[test]
    fn tangent_client_prepares_order_placement_from_market_reads() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(topic_u128(2)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(market_return(false)))}),
            json!({"jsonrpc":"2.0","id":3,"result": format!("0x{}", hex::encode(topic_u128(65_000 * PRICE_SCALE)))}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let preparation = client
            .prepare_order_placement(order_params(), 1_716_999_000, RpcBlockTag::Latest)
            .expect("order placement prepares");

        assert_eq!(
            preparation.plan.order_book,
            manifest.contracts.order_book.unwrap()
        );
        assert_eq!(preparation.market.total_markets, 2);
        assert_eq!(preparation.prepared_order.order.market_id, 1);
        assert_eq!(
            preparation.prepared_order.domain.verifying_contract,
            manifest.contracts.order_book.unwrap()
        );
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec!["eth_call", "eth_call", "eth_call"]
        );
    }

    #[test]
    fn tangent_client_order_placement_prepare_reports_market_validation_errors() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(topic_u128(2)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(market_return(true)))}),
            json!({"jsonrpc":"2.0","id":3,"result": format!("0x{}", hex::encode(topic_u128(65_000 * PRICE_SCALE)))}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let error = client
            .prepare_order_placement(order_params(), 1_716_999_000, RpcBlockTag::Latest)
            .expect_err("paused market rejects order placement");

        assert!(matches!(
            error,
            TangentOrderPlacementPrepareError::Order(OrderError::Invalid(message))
                if message == "market is paused"
        ));
    }

    #[test]
    fn tangent_client_collateral_status_helper_surfaces_decode_errors() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x"}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let error = client
            .collateral_status(Address::repeat_byte(0x33), 7, RpcBlockTag::Latest)
            .expect_err("bad collateral data fails decode");

        assert!(matches!(
            error,
            TangentReadPlanExecutionError::Decode(AbiDecodeError::InvalidLength { .. })
        ));
    }

    #[test]
    fn tangent_client_full_stack_helpers_report_missing_contracts_without_rpc() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let transport = MockTransport::default();
        let mut client = TangentClient::new(plan, transport);

        let order_error = client
            .order_lifecycle_status(signed_order(), RpcBlockTag::Latest)
            .expect_err("missing orderbook is reported");
        let settlement_error = client
            .settlement_status(7, 1, RpcBlockTag::Latest)
            .expect_err("missing settlement is reported");
        let liquidation_error = client
            .liquidation_status(7, 1, RpcBlockTag::Latest)
            .expect_err("missing liquidation keeper is reported");

        assert!(matches!(
            order_error,
            TangentClientReadError::Unavailable("OrderBook")
        ));
        assert!(matches!(
            settlement_error,
            TangentClientReadError::Unavailable("SettlementEngine")
        ));
        assert!(matches!(
            liquidation_error,
            TangentClientReadError::Unavailable("LiquidationKeeper")
        ));
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert!(transport.seen.is_empty());
    }

    #[test]
    fn tangent_client_workflow_uses_plan_confirmation_policy() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let policies = TangentClientPolicies {
            confirmation: TxConfirmationPolicy::new(5).with_timeout_blocks(50),
            ..TangentClientPolicies::default()
        };
        let plan =
            TangentClientPlan::with_policies(manifest, endpoint, policies).expect("client plan");
        let hash = TxHash::new(B256::repeat_byte(0x77));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x3d090"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":6,"result": hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());
        let tx = UnsignedCall {
            to: Address::repeat_byte(0x44),
            data: vec![0x12, 0x34, 0x56, 0x78],
        };

        let submission = workflow
            .preflight_sign_and_submit(&tx, Address::repeat_byte(0x33), RpcBlockTag::Pending)
            .expect("workflow submits");

        assert_eq!(submission.transaction_hash, hash);
        assert_eq!(
            submission.confirmation_plan.policy,
            TxConfirmationPolicy::new(5).with_timeout_blocks(50)
        );
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 1);
        assert_eq!(signer.seen[0].transaction.nonce.as_deref(), Some("0x7"));
        assert_eq!(
            signer.seen[0].transaction.max_fee_per_gas.as_deref(),
            Some("0xee6b2800")
        );
        assert_eq!(
            signer.seen[0]
                .transaction
                .max_priority_fee_per_gas
                .as_deref(),
            Some("0x3b9aca00")
        );
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_estimateGas",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_sendRawTransaction",
            ]
        );
    }

    #[test]
    fn tangent_client_builds_batch_preflight_plans_with_configured_policies() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let policies = TangentClientPolicies {
            confirmation: TxConfirmationPolicy::new(5).with_timeout_blocks(50),
            ..TangentClientPolicies::default()
        };
        let plan =
            TangentClientPlan::with_policies(manifest, endpoint, policies).expect("client plan");
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x7530"}),
        ]);
        let mut client = TangentClient::new(plan, transport);
        let first = UnsignedCall {
            to: Address::repeat_byte(0x44),
            data: vec![0x11, 0x11, 0x11, 0x11],
        };
        let second = UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x22, 0x22, 0x22, 0x22],
        };

        let plans = client
            .preflight_transaction_plans(
                &[first, second],
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect("client preflights batch");

        assert_eq!(plans[0].request.nonce.as_deref(), Some("0x7"));
        assert_eq!(plans[1].request.nonce.as_deref(), Some("0x8"));
        assert_eq!(
            plans[0].request.max_fee_per_gas.as_deref(),
            Some("0xee6b2800")
        );
        assert_eq!(
            plans[0].confirmation_policy,
            TxConfirmationPolicy::new(5).with_timeout_blocks(50)
        );
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_estimateGas",
                "eth_estimateGas"
            ]
        );
    }

    #[test]
    fn tangent_client_builds_preflight_summaries() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x7530"}),
        ]);
        let mut client = TangentClient::new(plan, transport);
        let first = UnsignedCall {
            to: Address::repeat_byte(0x44),
            data: vec![0x11, 0x11, 0x11, 0x11, 0xaa],
        };
        let second = UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x22, 0x22, 0x22, 0x22],
        };

        let summary = client
            .preflight_transaction_summary(
                &[first, second],
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect("client builds preflight summary");

        assert_eq!(summary.len, 2);
        assert_eq!(summary.total_calldata_bytes, Some(9));
        assert_eq!(summary.total_gas, Some(51_000));
        assert_eq!(summary.first_nonce.as_deref(), Some("0x7"));
        assert_eq!(summary.last_nonce.as_deref(), Some("0x8"));
        assert_eq!(summary.chain_id.as_deref(), Some("0x2b67"));
        assert!(summary.all_same_chain_id);
        assert_eq!(summary.eip1559_transactions, 2);
        assert_eq!(summary.plans[0].selector.as_deref(), Some("0x11111111"));
        assert_eq!(summary.plans[1].selector.as_deref(), Some("0x22222222"));

        let json = serde_json::to_string(&summary).expect("summary serializes");
        let restored: TxSubmissionPlanBatchSummary =
            serde_json::from_str(&json).expect("summary deserializes");
        assert_eq!(restored, summary);

        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_estimateGas",
                "eth_estimateGas"
            ]
        );
    }

    #[test]
    fn tangent_client_preflights_account_registration_plan() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let plans = client
            .account_registration_plans(owner, owner, RpcBlockTag::Pending)
            .expect("account registration preflights");

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].request.from, Some(owner));
        assert_eq!(plans[0].request.to, manifest.contracts.account_manager);
        assert_eq!(plans[0].request.nonce.as_deref(), Some("0x7"));
        assert_eq!(plans[0].request.gas.as_deref(), Some("0x5208"));
        assert_eq!(
            plans[0].request.max_fee_per_gas.as_deref(),
            Some("0xee6b2800")
        );
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_estimateGas"
            ]
        );
    }

    #[test]
    fn tangent_client_summarizes_account_registration_preflight() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let summary = client
            .account_registration_summary(owner, owner, RpcBlockTag::Pending)
            .expect("account registration summary");

        assert_eq!(summary.len, 1);
        assert_eq!(summary.total_gas, Some(21_000));
        assert_eq!(summary.first_nonce.as_deref(), Some("0x7"));
        assert_eq!(summary.last_nonce.as_deref(), Some("0x7"));
        assert_eq!(summary.plans[0].to, manifest.contracts.account_manager);
        assert_eq!(summary.plans[0].from, Some(owner));
        assert_eq!(summary.plans[0].calldata_bytes, Some(4));
        assert!(summary.plans[0].uses_eip1559_fees);
    }

    #[test]
    fn tangent_client_summarizes_signed_order_and_keeper_preflights() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":7,"result":"0x8"}),
            json!({"jsonrpc":"2.0","id":8,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":9,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":10,"result":"0x7530"}),
            json!({"jsonrpc":"2.0","id":11,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":12,"result":"0x9"}),
            json!({"jsonrpc":"2.0","id":13,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":14,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":15,"result":"0x9c40"}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let submit = client
            .submit_order_summary(signed_order(), owner, RpcBlockTag::Pending)
            .expect("submit order summary");
        let cancel = client
            .cancel_order_summary(signed_order(), owner, RpcBlockTag::Pending)
            .expect("cancel order summary");
        let tick = client
            .tick_orderbook_summary(owner, RpcBlockTag::Pending)
            .expect("tick summary");

        let order_book = manifest.contracts.order_book.expect("orderbook");
        assert_eq!(submit.len, 1);
        assert_eq!(submit.plans[0].to, order_book);
        assert_eq!(submit.plans[0].from, Some(owner));
        assert_eq!(submit.plans[0].nonce.as_deref(), Some("0x7"));
        assert_eq!(submit.total_gas, Some(21_000));
        assert!(submit.plans[0].calldata_bytes.unwrap_or_default() > 4);

        assert_eq!(cancel.len, 1);
        assert_eq!(cancel.plans[0].to, order_book);
        assert_eq!(cancel.plans[0].nonce.as_deref(), Some("0x8"));
        assert_eq!(cancel.total_gas, Some(30_000));

        assert_eq!(tick.len, 1);
        assert_eq!(tick.plans[0].to, order_book);
        assert_eq!(tick.plans[0].nonce.as_deref(), Some("0x9"));
        assert_eq!(tick.total_gas, Some(40_000));
        assert!(tick.all_same_chain_id);
    }

    #[test]
    fn tangent_client_summary_helpers_report_missing_orderbook() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let mut client = TangentClient::new(plan, MockTransport::new([]));

        let error = client
            .tick_orderbook_summary(Address::repeat_byte(0x33), RpcBlockTag::Pending)
            .expect_err("missing orderbook");

        assert!(matches!(
            error,
            TangentClientPreflightSummaryError::Context(TangentContextError::MissingOrderBook)
        ));
    }

    #[test]
    fn tangent_client_preflights_collateral_deposit_plans_with_sequential_nonces() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x7530"}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let plans = client
            .collateral_deposit_plans(7, 1_000_000, owner, RpcBlockTag::Pending)
            .expect("collateral deposit preflights");

        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].request.to, manifest.constants.usdc);
        assert_eq!(plans[1].request.to, manifest.contracts.usdc_vault);
        assert_eq!(plans[0].request.nonce.as_deref(), Some("0x7"));
        assert_eq!(plans[1].request.nonce.as_deref(), Some("0x8"));
        assert_eq!(plans[0].request.gas.as_deref(), Some("0x5208"));
        assert_eq!(plans[1].request.gas.as_deref(), Some("0x7530"));
        let (_, rpc) = client.into_parts();
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 6);
        assert_eq!(transport.seen[4].method, "eth_estimateGas");
        assert_eq!(transport.seen[5].method, "eth_estimateGas");
    }

    #[test]
    fn tangent_client_workflow_places_order_with_market_read_and_submit() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let hash = TxHash::new(B256::repeat_byte(0xcc));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(topic_u128(2)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(market_return(false)))}),
            json!({"jsonrpc":"2.0","id":3,"result": format!("0x{}", hex::encode(topic_u128(65_000 * PRICE_SCALE)))}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":7,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":8,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":9,"result": hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let order = workflow
            .place_order(
                order_params(),
                1_716_999_000,
                RpcBlockTag::Latest,
                owner,
                RpcBlockTag::Pending,
            )
            .expect("order placement submits");

        assert_eq!(order.submission.transaction_hash, hash);
        assert_eq!(
            order.placement.lifecycle.order_book,
            manifest.contracts.order_book.unwrap()
        );
        assert_eq!(order.placement.signed_order().order.account_id, 7);
        assert_eq!(
            order.placement.signed_order().signature,
            OrderSignature::from_bytes([3u8; OrderSignature::LEN]).unwrap()
        );
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 1);
        assert_eq!(
            signer.seen[0].transaction.to,
            manifest.contracts.order_book.unwrap()
        );
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_call",
                "eth_call",
                "eth_call",
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_estimateGas",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_sendRawTransaction"
            ]
        );
    }

    #[test]
    fn tangent_client_workflow_submits_signed_order_from_manifest_orderbook() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let hash = TxHash::new(B256::repeat_byte(0xcd));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":6,"result": hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let submission = workflow
            .submit_order(signed_order(), owner, RpcBlockTag::Pending)
            .expect("signed order submits");

        assert_eq!(submission.transaction_hash, hash);
        assert_eq!(
            submission.plan.request.to,
            manifest.contracts.order_book.unwrap()
        );
        assert!(submission.plan.request.data.starts_with("0xe8357b2d"));
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 1);
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_estimateGas",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_sendRawTransaction"
            ]
        );
    }

    #[test]
    fn tangent_client_workflow_cancels_signed_order_from_manifest_orderbook() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let hash = TxHash::new(B256::repeat_byte(0xce));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":6,"result": hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let submission = workflow
            .cancel_order(signed_order(), owner, RpcBlockTag::Pending)
            .expect("signed order cancels");

        assert_eq!(submission.transaction_hash, hash);
        assert_eq!(
            submission.plan.request.to,
            manifest.contracts.order_book.unwrap()
        );
        assert!(submission.plan.request.data.starts_with("0x7489ec23"));
    }

    #[test]
    fn tangent_client_workflow_order_lifecycle_submit_reports_missing_orderbook() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let client = TangentClient::new(plan, MockTransport::default());
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let error = workflow
            .submit_order(
                signed_order(),
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect_err("missing orderbook is reported");

        assert!(matches!(
            error,
            TangentOrderLifecycleSubmitError::Context(TangentContextError::MissingOrderBook)
        ));
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert!(signer.seen.is_empty());
        assert!(rpc.into_transport().seen.is_empty());
    }

    #[test]
    fn tangent_client_workflow_ticks_orderbook_from_manifest_plan() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let hash = TxHash::new(B256::repeat_byte(0xd1));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":6,"result": hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let submission = workflow
            .tick_orderbook(owner, RpcBlockTag::Pending)
            .expect("orderbook tick submits");

        assert_eq!(submission.transaction_hash, hash);
        assert_eq!(
            submission.plan.request.to,
            manifest.contracts.order_book.unwrap()
        );
        assert!(submission.plan.request.data.starts_with("0x3eaf5d9f"));
    }

    #[test]
    fn tangent_client_workflow_liquidates_ready_candidate() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let hash = TxHash::new(B256::repeat_byte(0xd2));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(bool_return(true)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(liquidation_state_return(true, -7, 9)))}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":7,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":8,"result": hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let liquidation = workflow
            .liquidate_if_ready(7, 1, RpcBlockTag::Latest, owner, RpcBlockTag::Pending)
            .expect("liquidation submits");

        assert!(liquidation.status.is_liquidatable);
        assert_eq!(liquidation.status.equity, -7);
        assert_eq!(liquidation.submission.transaction_hash, hash);
        assert_eq!(
            liquidation.submission.plan.request.to,
            manifest.contracts.liquidation_keeper.unwrap()
        );
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 1);
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_call",
                "eth_call",
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_estimateGas",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_sendRawTransaction"
            ]
        );
    }

    #[test]
    fn tangent_client_builds_ready_liquidation_dry_run() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(bool_return(true)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(liquidation_state_return(true, -7, 9)))}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":7,"result":"0x5208"}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let dry_run = client
            .liquidation_dry_run(7, 1, RpcBlockTag::Latest, owner, RpcBlockTag::Pending)
            .expect("liquidation dry run");

        assert_eq!(
            dry_run.candidate,
            TangentKeeperLiquidationCandidate::new(7, 1)
        );
        assert_eq!(dry_run.readiness, crate::LiquidationReadiness::Ready);
        assert_eq!(dry_run.status.equity, -7);
        let dry_run_summary = dry_run.summary();
        assert_eq!(dry_run_summary.candidate, dry_run.candidate);
        assert_eq!(
            dry_run_summary.readiness,
            crate::LiquidationReadiness::Ready
        );
        assert!(dry_run_summary.is_liquidatable);
        assert!(dry_run_summary.below_maintenance);
        assert_eq!(dry_run_summary.equity, -7);
        assert_eq!(dry_run_summary.maintenance_margin, 9);
        assert!(dry_run_summary.transaction_planned);
        assert!(dry_run_summary.has_transaction_summary);
        let summary_json =
            serde_json::to_string(&dry_run_summary).expect("dry run summary serializes");
        assert!(summary_json.contains("\"has_transaction_summary\":true"));
        let restored_summary: TangentLiquidationDryRunSummary =
            serde_json::from_str(&summary_json).expect("dry run summary deserializes");
        assert_eq!(restored_summary, dry_run_summary);
        let mut legacy_summary_json =
            serde_json::to_value(&dry_run_summary).expect("dry run summary value");
        let legacy_summary_object = legacy_summary_json
            .as_object_mut()
            .expect("dry run summary object");
        legacy_summary_object.remove("transaction_planned");
        legacy_summary_object.remove("has_transaction_summary");
        let legacy_summary: TangentLiquidationDryRunSummary =
            serde_json::from_value(legacy_summary_json).expect("legacy dry run summary");
        assert!(!legacy_summary.transaction_planned);
        assert!(!legacy_summary.has_transaction_summary);
        let summary = dry_run.transaction_summary.expect("ready tx summary");
        assert_eq!(summary.len, 1);
        assert_eq!(summary.total_gas, Some(21_000));
        assert_eq!(
            summary.plans[0].to,
            manifest.contracts.liquidation_keeper.expect("keeper")
        );
        assert_eq!(summary.plans[0].from, Some(owner));
        assert_eq!(summary.plans[0].nonce.as_deref(), Some("0x7"));
        assert!(summary.plans[0].uses_eip1559_fees);
    }

    #[test]
    fn tangent_client_liquidation_dry_run_blocks_not_ready_candidate() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(bool_return(false)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(liquidation_state_return(false, 10, 9)))}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let dry_run = client
            .liquidation_dry_run(
                7,
                1,
                RpcBlockTag::Latest,
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect("blocked dry run");

        assert_eq!(
            dry_run.readiness,
            crate::LiquidationReadiness::NotLiquidatable
        );
        assert_eq!(
            dry_run.status,
            LiquidationStatus {
                is_liquidatable: false,
                equity: 10,
                maintenance_margin: 9,
            }
        );
        assert_eq!(dry_run.transaction_summary, None);
        let (_, rpc) = client.into_parts();
        assert_eq!(rpc.into_transport().seen.len(), 2);
    }

    #[test]
    fn tangent_client_builds_liquidation_dry_run_batch() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(bool_return(true)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(liquidation_state_return(true, -7, 9)))}),
            json!({"jsonrpc":"2.0","id":3,"result": format!("0x{}", hex::encode(bool_return(false)))}),
            json!({"jsonrpc":"2.0","id":4,"result": format!("0x{}", hex::encode(liquidation_state_return(false, 12, 9)))}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":7,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":8,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":9,"result":"0x5208"}),
        ]);
        let mut client = TangentClient::new(plan, transport);
        let candidates = [
            TangentKeeperLiquidationCandidate::new(7, 1),
            TangentKeeperLiquidationCandidate::new(8, 1),
        ];

        let batch = client
            .liquidation_dry_run_batch(
                &candidates,
                RpcBlockTag::Latest,
                owner,
                RpcBlockTag::Pending,
            )
            .expect("liquidation batch dry run");

        assert_eq!(batch.candidates, 2);
        assert_eq!(batch.ready, 1);
        assert_eq!(batch.blocked, 1);
        assert_eq!(batch.ready_transaction_summary.len, 1);
        assert_eq!(batch.ready_transaction_summary.total_gas, Some(21_000));
        let batch_summary = batch.summary();
        assert_eq!(batch_summary.candidates, 2);
        assert_eq!(batch_summary.ready, 1);
        assert_eq!(batch_summary.blocked, 1);
        assert!(batch_summary.has_ready);
        assert!(batch_summary.has_blocked);
        assert!(!batch_summary.all_ready);
        assert_eq!(batch_summary.below_maintenance, 1);
        assert_eq!(batch_summary.transaction_plans, 1);
        assert!(batch_summary.has_transaction_plans);
        assert_eq!(batch_summary.ready_transaction_summary.len, 1);
        assert_eq!(batch_summary.reports.len(), 2);
        assert!(batch_summary.reports[0].transaction_planned);
        assert!(batch_summary.reports[0].has_transaction_summary);
        assert!(!batch_summary.reports[1].transaction_planned);
        assert!(!batch_summary.reports[1].has_transaction_summary);
        assert_eq!(batch_summary.reports[0].equity, -7);
        assert_eq!(batch_summary.reports[1].equity, 12);
        let batch_summary_json =
            serde_json::to_string(&batch_summary).expect("batch summary serializes");
        assert!(batch_summary_json.contains("\"has_transaction_plans\":true"));
        let restored_batch_summary: TangentLiquidationDryRunBatchSummary =
            serde_json::from_str(&batch_summary_json).expect("batch summary deserializes");
        assert_eq!(restored_batch_summary, batch_summary);
        let mut legacy_batch_summary_json =
            serde_json::to_value(&batch_summary).expect("batch summary value");
        let legacy_batch_summary_object = legacy_batch_summary_json
            .as_object_mut()
            .expect("batch summary object");
        legacy_batch_summary_object.remove("has_ready");
        legacy_batch_summary_object.remove("has_blocked");
        legacy_batch_summary_object.remove("all_ready");
        legacy_batch_summary_object.remove("has_transaction_plans");
        let legacy_reports = legacy_batch_summary_object
            .get_mut("reports")
            .and_then(serde_json::Value::as_array_mut)
            .expect("legacy dry run reports");
        for report in legacy_reports {
            let report_object = report.as_object_mut().expect("legacy dry run report");
            report_object.remove("transaction_planned");
            report_object.remove("has_transaction_summary");
        }
        let legacy_batch_summary: TangentLiquidationDryRunBatchSummary =
            serde_json::from_value(legacy_batch_summary_json)
                .expect("legacy batch summary deserializes");
        assert!(!legacy_batch_summary.has_ready);
        assert!(!legacy_batch_summary.has_blocked);
        assert!(!legacy_batch_summary.all_ready);
        assert!(!legacy_batch_summary.has_transaction_plans);
        assert!(legacy_batch_summary
            .reports
            .iter()
            .all(|report| !report.transaction_planned && !report.has_transaction_summary));
        assert_eq!(batch.reports.len(), 2);
        assert_eq!(batch.reports[0].candidate, candidates[0]);
        assert_eq!(
            batch.reports[0].readiness,
            crate::LiquidationReadiness::Ready
        );
        assert_eq!(
            batch.reports[0]
                .transaction_summary
                .as_ref()
                .expect("ready report summary")
                .first_nonce
                .as_deref(),
            Some("0x7")
        );
        assert_eq!(
            batch.reports[1].readiness,
            crate::LiquidationReadiness::NotLiquidatable
        );
        assert_eq!(batch.reports[1].transaction_summary, None);
        assert_eq!(
            batch.ready_transaction_summary.plans[0].to,
            manifest.contracts.liquidation_keeper.expect("keeper")
        );

        let json = serde_json::to_string(&batch).expect("batch serializes");
        let restored: TangentLiquidationDryRunBatch =
            serde_json::from_str(&json).expect("batch deserializes");
        assert_eq!(restored, batch);
    }

    #[test]
    fn tangent_client_builds_projection_liquidation_dry_run_batch() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let mut projection = TangentEventProjection::default();
        let active_hash = B256::repeat_byte(0xa1);
        let cancelled_hash = B256::repeat_byte(0xc1);
        projection
            .apply_event(&crate::TangentEvent::OrderSubmitted(
                crate::OrderSubmittedEvent {
                    order_hash: active_hash,
                    account_id: 7,
                    market_id: 1,
                    is_buy: true,
                    limit_price: 65_000 * PRICE_SCALE,
                    size: BASE_SCALE,
                },
            ))
            .expect("active order projection");
        projection
            .apply_event(&crate::TangentEvent::OrderSubmitted(
                crate::OrderSubmittedEvent {
                    order_hash: cancelled_hash,
                    account_id: 8,
                    market_id: 1,
                    is_buy: false,
                    limit_price: 65_000 * PRICE_SCALE,
                    size: BASE_SCALE,
                },
            ))
            .expect("cancelled order projection");
        projection
            .apply_event(&crate::TangentEvent::OrderCancelled(
                crate::OrderCancelledEvent {
                    order_hash: cancelled_hash,
                    account_id: 8,
                    reason: "expired".to_owned(),
                },
            ))
            .expect("cancel projection");

        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(bool_return(true)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(liquidation_state_return(true, -7, 9)))}),
            json!({"jsonrpc":"2.0","id":3,"result": format!("0x{}", hex::encode(bool_return(false)))}),
            json!({"jsonrpc":"2.0","id":4,"result": format!("0x{}", hex::encode(liquidation_state_return(false, 12, 9)))}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":7,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":8,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":9,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":10,"result": format!("0x{}", hex::encode(bool_return(true)))}),
            json!({"jsonrpc":"2.0","id":11,"result": format!("0x{}", hex::encode(liquidation_state_return(true, -7, 9)))}),
            json!({"jsonrpc":"2.0","id":12,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":13,"result":"0x8"}),
            json!({"jsonrpc":"2.0","id":14,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":15,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":16,"result":"0x7530"}),
        ]);
        let mut client = TangentClient::new(plan, transport);

        let all = client
            .projection_liquidation_dry_run_batch(
                &projection,
                RpcBlockTag::Latest,
                owner,
                RpcBlockTag::Pending,
            )
            .expect("all projection dry run");
        let active_only = client
            .active_projection_liquidation_dry_run_batch(
                &projection,
                RpcBlockTag::Latest,
                owner,
                RpcBlockTag::Pending,
            )
            .expect("active projection dry run");

        assert_eq!(all.candidates, 2);
        assert_eq!(all.ready, 1);
        assert_eq!(all.blocked, 1);
        let all_summary = all.summary();
        assert!(all_summary.has_ready);
        assert!(all_summary.has_blocked);
        assert!(!all_summary.all_ready);
        assert_eq!(
            all.reports
                .iter()
                .map(|report| report.candidate)
                .collect::<Vec<_>>(),
            vec![
                TangentKeeperLiquidationCandidate::new(7, 1),
                TangentKeeperLiquidationCandidate::new(8, 1)
            ]
        );
        assert_eq!(all.ready_transaction_summary.total_gas, Some(21_000));

        assert_eq!(active_only.candidates, 1);
        assert_eq!(active_only.ready, 1);
        assert_eq!(active_only.blocked, 0);
        let active_only_summary = active_only.summary();
        assert!(active_only_summary.has_ready);
        assert!(!active_only_summary.has_blocked);
        assert!(active_only_summary.all_ready);
        assert_eq!(
            active_only.reports[0].candidate,
            TangentKeeperLiquidationCandidate::new(7, 1)
        );
        assert_eq!(
            active_only.ready_transaction_summary.plans[0].to,
            manifest.contracts.liquidation_keeper.expect("keeper")
        );
        assert_eq!(
            active_only.ready_transaction_summary.total_gas,
            Some(30_000)
        );
    }

    #[test]
    fn tangent_client_workflow_does_not_submit_not_ready_liquidation() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(bool_return(false)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(liquidation_state_return(false, 10, 9)))}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let error = workflow
            .liquidate_if_ready(
                7,
                1,
                RpcBlockTag::Latest,
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect_err("not-ready liquidation is blocked");

        assert!(matches!(
            error,
            TangentKeeperWorkflowError::NotLiquidatable(LiquidationStatus {
                is_liquidatable: false,
                equity: 10,
                maintenance_margin: 9,
            })
        ));
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert!(signer.seen.is_empty());
        assert_eq!(rpc.into_transport().seen.len(), 2);
    }

    #[test]
    fn tangent_client_workflow_keeper_helpers_report_missing_contracts() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let client = TangentClient::new(plan, MockTransport::default());
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let tick_error = workflow
            .tick_orderbook(Address::repeat_byte(0x33), RpcBlockTag::Pending)
            .expect_err("missing orderbook blocks tick");
        let liquidation_error = workflow
            .liquidate_if_ready(
                7,
                1,
                RpcBlockTag::Latest,
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect_err("missing liquidation keeper blocks liquidation");

        assert!(matches!(
            tick_error,
            TangentKeeperWorkflowError::Context(TangentContextError::MissingOrderBook)
        ));
        assert!(matches!(
            liquidation_error,
            TangentKeeperWorkflowError::Context(TangentContextError::MissingLiquidationKeeper)
        ));
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert!(signer.seen.is_empty());
        assert!(rpc.into_transport().seen.is_empty());
    }

    #[test]
    fn tangent_client_workflow_executes_event_only_keeper_polling_pass() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let registered = account_registered_log_at(&manifest, 123, 10, 124);
        let transport = MockTransport::new([json!({
            "jsonrpc":"2.0",
            "id":1,
            "result":[registered]
        })]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());
        let snapshot = KeeperPollingSnapshot::at_block(123).with_event_from_block(123);

        let execution = workflow
            .execute_keeper_polling_pass(
                snapshot,
                &[],
                RpcBlockTag::Latest,
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect("keeper event polling pass executes");

        assert_eq!(execution.events.known_logs(), 1);
        assert_eq!(
            execution.outcome.latest_event_cursor,
            Some(RawLogCursor::new(123, 10))
        );
        assert!(execution.maintenance_submission.is_none());
        assert!(execution.liquidation_results.is_empty());
        let next = execution.outcome.next_snapshot(snapshot);
        assert_eq!(next.event_cursor, Some(RawLogCursor::new(123, 10)));
        assert_eq!(next.event_from_block, None);
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert!(signer.seen.is_empty());
        let transport = rpc.into_transport();
        assert_eq!(transport.seen.len(), 1);
        assert_eq!(transport.seen[0].method, "eth_getLogs");
    }

    #[test]
    fn tangent_client_workflow_executes_full_keeper_polling_pass() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let policies = TangentClientPolicies {
            keeper_polling: KeeperPollingPolicy::new(10, 1, 1),
            ..TangentClientPolicies::default()
        };
        let plan =
            TangentClientPlan::with_policies(manifest, endpoint, policies).expect("client plan");
        let tick_hash = TxHash::new(B256::repeat_byte(0x81));
        let liquidation_hash = TxHash::new(B256::repeat_byte(0x82));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(bool_return(true)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(liquidation_state_return(true, -10, 100)))}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":7,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":8,"result":"0x5209"}),
            json!({"jsonrpc":"2.0","id":9,"result": tick_hash.to_hex()}),
            json!({"jsonrpc":"2.0","id":10,"result": liquidation_hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());
        let snapshot = KeeperPollingSnapshot::at_block(130).with_event_from_block(131);
        let candidate = TangentKeeperLiquidationCandidate::new(7, 1);

        let execution = workflow
            .execute_keeper_polling_pass(
                snapshot,
                &[candidate],
                RpcBlockTag::Latest,
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect("full keeper polling pass executes");

        assert!(execution.events.is_empty());
        assert!(execution.outcome.completed_maintenance);
        assert!(execution.outcome.completed_liquidation_scan);
        assert_eq!(
            execution
                .maintenance_submission
                .as_ref()
                .expect("maintenance submitted")
                .submissions[0]
                .transaction_hash,
            tick_hash
        );
        assert_eq!(execution.liquidation_results.len(), 1);
        assert_eq!(execution.liquidation_results[0].candidate, candidate);
        assert_eq!(
            execution.liquidation_results[0]
                .submission
                .as_ref()
                .expect("liquidation submitted")
                .transaction_hash,
            liquidation_hash
        );
        let report = execution.report(snapshot);
        assert_eq!(report.plan_summary.maintenance_transaction_count, 1);
        assert_eq!(report.event_records, 0);
        assert_eq!(report.decoded_events, 0);
        assert_eq!(report.unknown_logs, 0);
        assert_eq!(report.maintenance_submissions, 1);
        assert_eq!(report.maintenance_transaction_hashes, vec![tick_hash]);
        assert!(report.has_maintenance_submission_report);
        assert_eq!(
            report
                .maintenance_submission_report
                .as_ref()
                .expect("maintenance report")
                .transaction_hashes,
            vec![tick_hash]
        );
        assert_eq!(report.liquidation_scans, 1);
        assert!(report.has_liquidation_reports);
        assert_eq!(report.ready_liquidations, 1);
        assert!(report.has_ready_liquidations);
        assert_eq!(report.submitted_liquidations, 1);
        assert!(report.has_submitted_liquidations);
        assert_eq!(
            report.liquidation_reports[0]
                .submitted_transaction_report
                .as_ref()
                .expect("liquidation report")
                .transaction_hash,
            liquidation_hash
        );
        assert_eq!(
            report.liquidation_reports,
            vec![TangentKeeperLiquidationScanReport {
                candidate,
                readiness: crate::LiquidationReadiness::Ready,
                has_submission: true,
                has_submitted_transaction_hash: true,
                submitted_transaction_hash: Some(liquidation_hash),
                has_submitted_transaction_report: true,
                submitted_transaction_report: execution.liquidation_results[0]
                    .submission
                    .as_ref()
                    .map(TxWorkflowSubmission::report),
            }]
        );
        assert!(report.liquidation_reports[0].has_submission);
        assert!(report.liquidation_reports[0].has_submitted_transaction_hash);
        assert!(report.liquidation_reports[0].has_submitted_transaction_report);
        assert_eq!(report.checkpoint.snapshot.last_tick_block, Some(130));
        assert_eq!(
            report.checkpoint.snapshot.last_liquidation_scan_block,
            Some(130)
        );
        let json = serde_json::to_string(&report).expect("report serializes");
        assert!(json.contains("\"has_submission\":true"));
        assert!(json.contains("\"has_maintenance_submission_report\":true"));
        assert!(json.contains("\"has_submitted_liquidations\":true"));
        let restored: TangentKeeperPollingExecutionReport =
            serde_json::from_str(&json).expect("report deserializes");
        assert_eq!(restored, report);
        let mut legacy_report_json = serde_json::to_value(&report).expect("report value");
        let legacy_report_object = legacy_report_json
            .as_object_mut()
            .expect("report value object");
        legacy_report_object.remove("has_maintenance_submission_report");
        legacy_report_object.remove("has_liquidation_reports");
        legacy_report_object.remove("has_ready_liquidations");
        legacy_report_object.remove("has_submitted_liquidations");
        let legacy_liquidation_reports = legacy_report_object
            .get_mut("liquidation_reports")
            .and_then(serde_json::Value::as_array_mut)
            .expect("liquidation reports array");
        let legacy_liquidation_report = legacy_liquidation_reports[0]
            .as_object_mut()
            .expect("liquidation report object");
        legacy_liquidation_report.remove("has_submission");
        legacy_liquidation_report.remove("has_submitted_transaction_hash");
        legacy_liquidation_report.remove("has_submitted_transaction_report");
        let legacy_report: TangentKeeperPollingExecutionReport =
            serde_json::from_value(legacy_report_json).expect("legacy report deserializes");
        assert!(!legacy_report.has_maintenance_submission_report);
        assert!(!legacy_report.has_liquidation_reports);
        assert!(!legacy_report.has_ready_liquidations);
        assert!(!legacy_report.has_submitted_liquidations);
        assert!(!legacy_report.liquidation_reports[0].has_submission);
        assert!(!legacy_report.liquidation_reports[0].has_submitted_transaction_hash);
        assert!(!legacy_report.liquidation_reports[0].has_submitted_transaction_report);
        let summary = report.summary();
        assert_eq!(summary.checkpoint, report.checkpoint);
        assert!(summary.planned_work);
        assert_eq!(summary.event_query_count, 0);
        assert_eq!(summary.maintenance_transaction_count, 1);
        assert!(summary.should_scan_liquidations);
        assert_eq!(summary.event_records, 0);
        assert!(!summary.has_event_records);
        assert_eq!(summary.decoded_events, 0);
        assert!(!summary.has_decoded_events);
        assert_eq!(summary.unknown_logs, 0);
        assert!(!summary.has_unknown_logs);
        assert_eq!(summary.derived_liquidation_candidates, 0);
        assert!(!summary.has_derived_liquidation_candidates);
        assert_eq!(summary.maintenance_submissions, 1);
        assert_eq!(summary.maintenance_transaction_hashes, vec![tick_hash]);
        assert_eq!(summary.liquidation_scans, 1);
        assert!(summary.has_liquidation_scans);
        assert_eq!(summary.ready_liquidations, 1);
        assert!(summary.has_ready_liquidations);
        assert_eq!(summary.submitted_liquidations, 1);
        assert_eq!(
            summary.liquidation_transaction_hashes,
            vec![liquidation_hash]
        );
        assert_eq!(summary.submitted_transactions, 2);
        assert!(summary.has_submissions);
        assert!(!summary.advanced_event_cursor);
        assert!(summary.completed_maintenance);
        assert!(summary.completed_liquidation_scan);
        let summary_json = serde_json::to_string(&summary).expect("summary serializes");
        let restored_summary: TangentKeeperPollingExecutionSummary =
            serde_json::from_str(&summary_json).expect("summary deserializes");
        assert_eq!(restored_summary, summary);
        let mut legacy_summary_json =
            serde_json::to_value(&summary).expect("summary value serializes");
        let legacy_summary_object = legacy_summary_json
            .as_object_mut()
            .expect("summary value object");
        legacy_summary_object.remove("maintenance_transaction_hashes");
        legacy_summary_object.remove("liquidation_transaction_hashes");
        legacy_summary_object.remove("has_event_records");
        legacy_summary_object.remove("has_decoded_events");
        legacy_summary_object.remove("has_unknown_logs");
        legacy_summary_object.remove("has_derived_liquidation_candidates");
        legacy_summary_object.remove("has_liquidation_scans");
        legacy_summary_object.remove("has_ready_liquidations");
        legacy_summary_object.remove("has_submissions");
        let legacy_summary: TangentKeeperPollingExecutionSummary =
            serde_json::from_value(legacy_summary_json).expect("legacy summary deserializes");
        assert_eq!(legacy_summary.maintenance_transaction_hashes, Vec::new());
        assert_eq!(legacy_summary.liquidation_transaction_hashes, Vec::new());
        assert!(!legacy_summary.has_event_records);
        assert!(!legacy_summary.has_decoded_events);
        assert!(!legacy_summary.has_unknown_logs);
        assert!(!legacy_summary.has_derived_liquidation_candidates);
        assert!(!legacy_summary.has_liquidation_scans);
        assert!(!legacy_summary.has_ready_liquidations);
        assert!(!legacy_summary.has_submissions);
        let next = execution.outcome.next_snapshot(snapshot);
        assert_eq!(next.last_tick_block, Some(130));
        assert_eq!(next.last_liquidation_scan_block, Some(130));
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 2);
        let transport = rpc.into_transport();
        assert_eq!(transport.seen[0].method, "eth_call");
        assert_eq!(transport.seen[1].method, "eth_call");
        assert_eq!(transport.seen[8].method, "eth_sendRawTransaction");
        assert_eq!(transport.seen[9].method, "eth_sendRawTransaction");
    }

    #[test]
    fn tangent_client_workflow_derives_keeper_candidates_from_event_projection() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let policies = TangentClientPolicies {
            keeper_polling: KeeperPollingPolicy::new(10, 1, 1),
            ..TangentClientPolicies::default()
        };
        let plan = TangentClientPlan::with_policies(manifest.clone(), endpoint, policies)
            .expect("client plan");
        let order_hash = B256::repeat_byte(0xd7);
        let submitted = order_submitted_log_at(&manifest, 130, 7, order_hash, 7, 1);
        let transport = MockTransport::new([
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "result":[submitted]
            }),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(bool_return(false)))}),
            json!({"jsonrpc":"2.0","id":3,"result": format!("0x{}", hex::encode(liquidation_state_return(false, 10, 100)))}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());
        let snapshot = KeeperPollingSnapshot::at_block(130)
            .with_event_from_block(130)
            .with_last_tick_block(130);

        let execution = workflow
            .execute_keeper_polling_pass(
                snapshot,
                &[],
                RpcBlockTag::Latest,
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect("keeper polling pass derives candidate");

        assert_eq!(execution.event_records.known_logs(), 1);
        assert_eq!(execution.events.known_logs(), 1);
        assert_eq!(
            execution.derived_liquidation_candidates,
            vec![TangentKeeperLiquidationCandidate::new(7, 1)]
        );
        assert_eq!(execution.liquidation_results.len(), 1);
        assert_eq!(
            execution.liquidation_results[0].candidate,
            TangentKeeperLiquidationCandidate::new(7, 1)
        );
        assert!(!execution.liquidation_results[0].status.is_liquidatable);
        assert!(execution.maintenance_submission.is_none());
        assert_eq!(
            execution.projection.orders[&order_hash].remaining_size(),
            Some(BASE_SCALE)
        );
        assert_eq!(
            execution.projection.last_cursor,
            Some(RawLogCursor::new(130, 7))
        );
        let report = execution.report(snapshot);
        let summary = report.summary();
        assert!(summary.planned_work);
        assert_eq!(summary.event_query_count, 1);
        assert_eq!(summary.event_records, 1);
        assert!(summary.has_event_records);
        assert_eq!(summary.decoded_events, 1);
        assert!(summary.has_decoded_events);
        assert_eq!(summary.unknown_logs, 0);
        assert!(!summary.has_unknown_logs);
        assert_eq!(summary.derived_liquidation_candidates, 1);
        assert!(summary.has_derived_liquidation_candidates);
        assert_eq!(summary.liquidation_scans, 1);
        assert!(summary.has_liquidation_scans);
        assert_eq!(summary.ready_liquidations, 0);
        assert!(!summary.has_ready_liquidations);
        assert_eq!(summary.submitted_transactions, 0);
        assert!(!summary.has_submissions);
        assert!(summary.advanced_event_cursor);
        assert!(summary.completed_liquidation_scan);
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert!(signer.seen.is_empty());
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec!["eth_getLogs", "eth_call", "eth_call"]
        );
    }

    #[test]
    fn tangent_client_workflow_uses_existing_projection_for_keeper_candidates() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let policies = TangentClientPolicies {
            keeper_polling: KeeperPollingPolicy::new(10, 1, 1),
            ..TangentClientPolicies::default()
        };
        let plan =
            TangentClientPlan::with_policies(manifest, endpoint, policies).expect("client plan");
        let order_hash = B256::repeat_byte(0xe7);
        let mut projection = TangentEventProjection::default();
        projection
            .apply_event(&crate::TangentEvent::OrderSubmitted(
                crate::OrderSubmittedEvent {
                    order_hash,
                    account_id: 9,
                    market_id: 2,
                    is_buy: false,
                    limit_price: 70_000 * PRICE_SCALE,
                    size: BASE_SCALE,
                },
            ))
            .expect("seed projection");
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": format!("0x{}", hex::encode(bool_return(false)))}),
            json!({"jsonrpc":"2.0","id":2,"result": format!("0x{}", hex::encode(liquidation_state_return(false, 20, 100)))}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());
        let snapshot = KeeperPollingSnapshot::at_block(130)
            .with_event_from_block(131)
            .with_last_tick_block(130);

        let state = TangentKeeperPollingState::new(snapshot, projection);
        let state_execution = workflow
            .execute_keeper_polling_state(
                state,
                &[],
                RpcBlockTag::Latest,
                Address::repeat_byte(0x33),
                RpcBlockTag::Pending,
            )
            .expect("keeper polling state executes");
        let execution = &state_execution.execution;

        assert!(execution.event_records.is_empty());
        assert_eq!(
            execution.derived_liquidation_candidates,
            vec![TangentKeeperLiquidationCandidate::new(9, 2)]
        );
        assert_eq!(execution.liquidation_results.len(), 1);
        assert_eq!(
            execution.liquidation_results[0].candidate,
            TangentKeeperLiquidationCandidate::new(9, 2)
        );
        assert_eq!(
            execution.projection.orders[&order_hash].remaining_size(),
            Some(BASE_SCALE)
        );
        assert_eq!(
            state_execution.next_state.projection.orders[&order_hash].remaining_size(),
            Some(BASE_SCALE)
        );
        assert_eq!(
            state_execution
                .next_state
                .snapshot
                .last_liquidation_scan_block,
            Some(130)
        );
        let report = state_execution.report();
        assert_eq!(report.checkpoint, state_execution.checkpoint());
        assert_eq!(
            report.derived_liquidation_candidates,
            vec![TangentKeeperLiquidationCandidate::new(9, 2)]
        );
        assert_eq!(report.liquidation_scans, 1);
        assert_eq!(report.ready_liquidations, 0);
        assert_eq!(report.submitted_liquidations, 0);
        assert_eq!(report.maintenance_submissions, 0);
        assert_eq!(
            report.checkpoint.snapshot.last_liquidation_scan_block,
            Some(130)
        );
        let json = serde_json::to_string(&report).expect("state report serializes");
        let restored: TangentKeeperPollingExecutionReport =
            serde_json::from_str(&json).expect("state report deserializes");
        assert_eq!(restored, report);
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert!(signer.seen.is_empty());
        assert_eq!(
            rpc.into_transport()
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec!["eth_call", "eth_call"]
        );
    }

    #[test]
    fn keeper_polling_state_exposes_compact_checkpoint() {
        let mut projection = TangentEventProjection {
            applied_records: 3,
            unknown_logs: 1,
            last_cursor: Some(RawLogCursor::new(124, 2)),
            ..TangentEventProjection::default()
        };
        projection.accounts.insert(
            7,
            crate::AccountEventProjection {
                account_id: 7,
                ..crate::AccountEventProjection::default()
            },
        );
        let state = TangentKeeperPollingState::new(
            KeeperPollingSnapshot::at_block(125)
                .with_event_cursor(RawLogCursor::new(123, 9))
                .with_event_from_block(100)
                .with_last_tick_block(120),
            projection,
        );

        let checkpoint = state.checkpoint();

        assert_eq!(checkpoint.snapshot.current_block, 125);
        assert_eq!(checkpoint.projection.accounts, 1);
        assert_eq!(checkpoint.projection.applied_records, 3);
        assert_eq!(checkpoint.projection.unknown_logs, 1);
        assert_eq!(
            checkpoint.effective_event_cursor(),
            Some(RawLogCursor::new(124, 2))
        );
        assert!(!checkpoint.projection_cursor_is_checkpointed());

        let reconciled = checkpoint.reconciled_snapshot();
        assert_eq!(reconciled.event_cursor, Some(RawLogCursor::new(124, 2)));
        assert_eq!(reconciled.event_from_block, None);
        assert_eq!(reconciled.last_tick_block, Some(120));

        let resume_snapshot = checkpoint.resume_snapshot_at(130);
        assert_eq!(resume_snapshot.current_block, 130);
        assert_eq!(
            resume_snapshot.event_cursor,
            Some(RawLogCursor::new(124, 2))
        );
        assert_eq!(resume_snapshot.event_from_block, None);

        let resumed_projection = TangentEventProjection {
            last_cursor: Some(RawLogCursor::new(126, 4)),
            ..TangentEventProjection::default()
        };
        let resumed_state = checkpoint.resume_state_at(131, resumed_projection);
        assert_eq!(resumed_state.snapshot.current_block, 131);
        assert_eq!(
            resumed_state.snapshot.event_cursor,
            Some(RawLogCursor::new(126, 4))
        );
        assert_eq!(resumed_state.snapshot.event_from_block, None);
        assert_eq!(
            resumed_state.projection.last_cursor,
            Some(RawLogCursor::new(126, 4))
        );

        let json = serde_json::to_string(&checkpoint).expect("checkpoint serializes");
        let restored: TangentKeeperPollingCheckpoint =
            serde_json::from_str(&json).expect("checkpoint deserializes");
        assert_eq!(restored, checkpoint);
    }

    #[test]
    fn keeper_polling_preview_summarizes_state_and_scan_candidates() {
        let manifest = full_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let policies = TangentClientPolicies {
            keeper_polling: KeeperPollingPolicy::new(10, 1, 1),
            ..TangentClientPolicies::default()
        };
        let plan =
            TangentClientPlan::with_policies(manifest, endpoint, policies).expect("client plan");
        let mut projection = TangentEventProjection::default();
        let mut first_order = crate::OrderEventProjection::new(B256::repeat_byte(0x71));
        first_order.account_id = Some(7);
        first_order.market_id = Some(1);
        let mut second_order = crate::OrderEventProjection::new(B256::repeat_byte(0x82));
        second_order.account_id = Some(8);
        second_order.market_id = Some(2);
        projection
            .orders
            .insert(first_order.order_hash, first_order);
        projection
            .orders
            .insert(second_order.order_hash, second_order);
        projection.last_cursor = Some(RawLogCursor::new(124, 2));
        let state = TangentKeeperPollingState::new(
            KeeperPollingSnapshot::at_block(130)
                .with_event_cursor(RawLogCursor::new(124, 2))
                .with_last_liquidation_scan_block(128),
            projection,
        );
        let explicit = [
            TangentKeeperLiquidationCandidate::new(8, 2),
            TangentKeeperLiquidationCandidate::new(9, 3),
        ];

        let preview = plan
            .keeper_polling_preview(&state, &explicit)
            .expect("preview builds");

        assert_eq!(
            preview.checkpoint.effective_event_cursor(),
            Some(RawLogCursor::new(124, 2))
        );
        assert_eq!(preview.plan_summary.event_query_count, 1);
        assert!(preview.plan_summary.should_scan_liquidations);
        assert_eq!(preview.explicit_liquidation_candidates, explicit);
        assert_eq!(
            preview.derived_liquidation_candidates,
            vec![
                TangentKeeperLiquidationCandidate::new(7, 1),
                TangentKeeperLiquidationCandidate::new(8, 2),
            ]
        );
        assert_eq!(
            preview.scan_candidates,
            vec![
                TangentKeeperLiquidationCandidate::new(7, 1),
                TangentKeeperLiquidationCandidate::new(8, 2),
                TangentKeeperLiquidationCandidate::new(9, 3),
            ]
        );

        let json = serde_json::to_string(&preview).expect("preview serializes");
        let restored: TangentKeeperPollingPreview =
            serde_json::from_str(&json).expect("preview deserializes");
        assert_eq!(restored, preview);
        let preview_summary = preview.summary();
        assert_eq!(preview_summary.checkpoint, preview.checkpoint);
        assert!(preview_summary.planned_work);
        assert_eq!(preview_summary.event_query_count, 1);
        assert_eq!(preview_summary.maintenance_transaction_count, 1);
        assert!(preview_summary.should_scan_liquidations);
        assert_eq!(preview_summary.explicit_liquidation_candidates, 2);
        assert!(preview_summary.has_explicit_liquidation_candidates);
        assert_eq!(preview_summary.derived_liquidation_candidates, 2);
        assert!(preview_summary.has_derived_liquidation_candidates);
        assert_eq!(preview_summary.scan_candidates, 3);
        assert!(preview_summary.has_scan_candidates);
        let summary_json =
            serde_json::to_string(&preview_summary).expect("preview summary serializes");
        let restored_summary: TangentKeeperPollingPreviewSummary =
            serde_json::from_str(&summary_json).expect("preview summary deserializes");
        assert_eq!(restored_summary, preview_summary);
        let mut legacy_summary_json =
            serde_json::to_value(&preview_summary).expect("preview summary value");
        let legacy_summary_object = legacy_summary_json
            .as_object_mut()
            .expect("preview summary object");
        legacy_summary_object.remove("has_explicit_liquidation_candidates");
        legacy_summary_object.remove("has_derived_liquidation_candidates");
        let legacy_summary: TangentKeeperPollingPreviewSummary =
            serde_json::from_value(legacy_summary_json).expect("legacy preview summary");
        assert!(!legacy_summary.has_explicit_liquidation_candidates);
        assert!(!legacy_summary.has_derived_liquidation_candidates);

        let client = TangentClient::new(plan, MockTransport::default());
        let facade_preview = client
            .keeper_polling_preview(&state, &explicit)
            .expect("facade preview builds");
        assert_eq!(facade_preview, preview);

        let workflow_preview = client
            .into_workflow(MockRawSigner::default())
            .keeper_polling_preview(&state, &explicit)
            .expect("workflow preview builds");
        assert_eq!(workflow_preview, preview);
    }

    #[test]
    fn tangent_client_workflow_submits_one_prepared_raw_plan() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let hash = TxHash::new(B256::repeat_byte(0x88));
        let transport =
            MockTransport::new([json!({"jsonrpc":"2.0","id":1,"result": hash.to_hex()})]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());
        let tx_plan = TxSubmissionPlan::new(
            UnsignedTxRequest {
                from: Some(Address::repeat_byte(0x33)),
                to: Address::repeat_byte(0x44),
                data: "0x11111111".to_owned(),
                value: "0x0".to_owned(),
                nonce: Some("0x7".to_owned()),
                gas: Some("0x5208".to_owned()),
                gas_price: Some("0x3b9aca00".to_owned()),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                chain_id: Some("0x2b67".to_owned()),
            },
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );

        let submission = workflow
            .submit_raw_plan(&tx_plan)
            .expect("prepared plan submits");

        assert_eq!(submission.transaction_hash, hash);
        assert_eq!(submission.plan, tx_plan);
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 1);
        let transport = rpc.into_transport();
        assert_eq!(transport.seen[0].method, "eth_sendRawTransaction");
    }

    #[test]
    fn tangent_client_workflow_submits_prepared_raw_plan_batches() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let first_hash = TxHash::new(B256::repeat_byte(0x88));
        let second_hash = TxHash::new(B256::repeat_byte(0x99));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": first_hash.to_hex()}),
            json!({"jsonrpc":"2.0","id":2,"result": second_hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());
        let first_plan = TxSubmissionPlan::new(
            UnsignedTxRequest {
                from: Some(Address::repeat_byte(0x33)),
                to: Address::repeat_byte(0x44),
                data: "0x11111111".to_owned(),
                value: "0x0".to_owned(),
                nonce: Some("0x7".to_owned()),
                gas: Some("0x5208".to_owned()),
                gas_price: Some("0x3b9aca00".to_owned()),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                chain_id: Some("0x2b67".to_owned()),
            },
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );
        let second_plan = TxSubmissionPlan::new(
            UnsignedTxRequest {
                from: Some(Address::repeat_byte(0x33)),
                to: Address::repeat_byte(0x55),
                data: "0x22222222".to_owned(),
                value: "0x0".to_owned(),
                nonce: Some("0x8".to_owned()),
                gas: Some("0x7530".to_owned()),
                gas_price: Some("0x3b9aca00".to_owned()),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                chain_id: Some("0x2b67".to_owned()),
            },
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );

        let batch = workflow
            .submit_raw_plans(&[first_plan.clone(), second_plan.clone()])
            .expect("batch submits");

        assert_eq!(batch.transaction_hashes(), vec![first_hash, second_hash]);
        let partial_batch = TxWorkflowBatchSubmission::new(vec![batch.submissions[0].clone()]);
        let resume = workflow
            .resume_raw_plan_batch(&[first_plan.clone(), second_plan.clone()], &partial_batch);
        assert_eq!(resume.next_plan_index, 1);
        assert_eq!(resume.remaining_len, 1);
        assert_eq!(
            resume.remaining_plans[0].request.to,
            Address::repeat_byte(0x55)
        );
        assert_eq!(resume.submitted_transaction_hashes, vec![first_hash]);
        let resume_summary =
            workflow.resume_raw_plan_batch_summary(&[first_plan, second_plan], &partial_batch);
        assert_eq!(resume_summary.original_len, 2);
        assert_eq!(resume_summary.submitted_len, 1);
        assert!(resume_summary.has_submitted);
        assert_eq!(resume_summary.next_plan_index, 1);
        assert_eq!(resume_summary.remaining_len, 1);
        assert!(resume_summary.has_remaining);
        assert!(resume_summary.can_continue);
        assert_eq!(
            resume_summary.submitted_transaction_hashes,
            vec![first_hash]
        );
        assert_eq!(resume_summary.submitted_plan_summary.len, 1);
        assert_eq!(resume_summary.remaining_plan_summary.len, 1);
        let summary_json =
            serde_json::to_string(&resume_summary).expect("resume summary serializes");
        let restored_summary: TxWorkflowBatchResumePlanSummary =
            serde_json::from_str(&summary_json).expect("resume summary deserializes");
        assert_eq!(restored_summary, resume_summary);
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 2);
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec!["eth_sendRawTransaction", "eth_sendRawTransaction"]
        );
    }

    #[test]
    fn tangent_client_workflow_fetches_confirmation_batch_snapshot() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest, endpoint).expect("client plan");
        let first_hash = TxHash::new(B256::repeat_byte(0x88));
        let second_hash = TxHash::new(B256::repeat_byte(0x99));
        let first_receipt = crate::TxReceipt::new(first_hash)
            .with_block_number(10)
            .with_status(true);
        let second_receipt = crate::TxReceipt::new(second_hash).with_block_number(12);
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result": first_hash.to_hex()}),
            json!({"jsonrpc":"2.0","id":2,"result": second_hash.to_hex()}),
            json!({"jsonrpc":"2.0","id":3,"result": first_receipt}),
            json!({"jsonrpc":"2.0","id":4,"result":"0xc"}),
            json!({"jsonrpc":"2.0","id":5,"result": second_receipt}),
            json!({"jsonrpc":"2.0","id":6,"result":"0xc"}),
            json!({"jsonrpc":"2.0","id":7,"result": first_receipt}),
            json!({"jsonrpc":"2.0","id":8,"result":"0xc"}),
            json!({"jsonrpc":"2.0","id":9,"result": second_receipt}),
            json!({"jsonrpc":"2.0","id":10,"result":"0xc"}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());
        let first_plan = TxSubmissionPlan::new(
            UnsignedTxRequest {
                from: Some(Address::repeat_byte(0x33)),
                to: Address::repeat_byte(0x44),
                data: "0x11111111".to_owned(),
                value: "0x0".to_owned(),
                nonce: Some("0x7".to_owned()),
                gas: Some("0x5208".to_owned()),
                gas_price: Some("0x3b9aca00".to_owned()),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                chain_id: Some("0x2b67".to_owned()),
            },
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );
        let second_plan = TxSubmissionPlan::new(
            UnsignedTxRequest {
                from: Some(Address::repeat_byte(0x33)),
                to: Address::repeat_byte(0x55),
                data: "0x22222222".to_owned(),
                value: "0x0".to_owned(),
                nonce: Some("0x8".to_owned()),
                gas: Some("0x7530".to_owned()),
                gas_price: Some("0x3b9aca00".to_owned()),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                chain_id: Some("0x2b67".to_owned()),
            },
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );

        let batch = workflow
            .submit_raw_plans(&[first_plan, second_plan])
            .expect("batch submits");
        let confirmations = workflow
            .confirmation_batch_snapshot(&batch)
            .expect("confirmation batch");
        let report = workflow
            .confirmation_batch_report(&batch)
            .expect("confirmation report");

        assert_eq!(confirmations.snapshots.len(), 2);
        assert_eq!(
            confirmations.status,
            crate::TxConfirmationBatchStatus::Pending {
                confirmed: 1,
                pending: 1,
                total: 2,
            }
        );
        assert_eq!(report.status, confirmations.status);
        assert_eq!(report.reports[0].transaction_hash, Some(first_hash));
        assert_eq!(report.reports[1].transaction_hash, Some(second_hash));
    }

    #[test]
    fn tangent_client_workflow_registers_account_from_manifest_plan() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let hash = TxHash::new(B256::repeat_byte(0xaa));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result": hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let batch = workflow
            .register_account(owner, owner, RpcBlockTag::Pending)
            .expect("account registration workflow submits");

        assert_eq!(batch.transaction_hashes(), vec![hash]);
        assert_eq!(
            batch.submissions[0].plan.request.to,
            manifest.contracts.account_manager
        );
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 1);
        assert_eq!(signer.seen[0].transaction.nonce.as_deref(), Some("0x7"));
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_estimateGas",
                "eth_sendRawTransaction"
            ]
        );
    }

    #[test]
    fn tangent_client_workflow_deposits_collateral_with_sequential_raw_sends() {
        let manifest = current_manifest();
        let endpoint = RpcEndpointConfig::new("https://rpc.example.invalid").expect("endpoint");
        let plan = TangentClientPlan::new(manifest.clone(), endpoint).expect("client plan");
        let owner = Address::repeat_byte(0x33);
        let first_hash = TxHash::new(B256::repeat_byte(0xaa));
        let second_hash = TxHash::new(B256::repeat_byte(0xbb));
        let transport = MockTransport::new([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x7530"}),
            json!({"jsonrpc":"2.0","id":7,"result": first_hash.to_hex()}),
            json!({"jsonrpc":"2.0","id":8,"result": second_hash.to_hex()}),
        ]);
        let client = TangentClient::new(plan, transport);
        let mut workflow = client.into_workflow(MockRawSigner::default());

        let batch = workflow
            .deposit_collateral(7, 1_000_000, owner, RpcBlockTag::Pending)
            .expect("collateral deposit workflow submits");

        assert_eq!(batch.transaction_hashes(), vec![first_hash, second_hash]);
        assert_eq!(
            batch.submissions[0].plan.request.to,
            manifest.constants.usdc
        );
        assert_eq!(
            batch.submissions[1].plan.request.to,
            manifest.contracts.usdc_vault
        );
        let (_, workflow) = workflow.into_parts();
        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 2);
        assert_eq!(signer.seen[0].transaction.nonce.as_deref(), Some("0x7"));
        assert_eq!(signer.seen[1].transaction.nonce.as_deref(), Some("0x8"));
        let transport = rpc.into_transport();
        assert_eq!(
            transport
                .seen
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "eth_chainId",
                "eth_getTransactionCount",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_estimateGas",
                "eth_estimateGas",
                "eth_sendRawTransaction",
                "eth_sendRawTransaction"
            ]
        );
    }
}
