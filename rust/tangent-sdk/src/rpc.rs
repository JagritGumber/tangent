//! Transport-neutral JSON-RPC execution helpers.
//!
//! This module still does not open sockets or choose an HTTP/WebSocket client.
//! It gives downstream clients a small trait boundary so the SDK can own
//! request construction and typed response decoding while callers plug in
//! Alloy, a relayer, a wallet service, or a test double.

use alloy_primitives::Address;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::events::{EventLogRpcQuery, RawLog};
use crate::signing::RawTransactionSigner;
use crate::tx::{
    CallReturn, CallReturnBatch, JsonRpcBatchError, JsonRpcErrorObject, JsonRpcRequest,
    JsonRpcRequestError, JsonRpcResponse, JsonRpcResultDecodeError, RpcBlockTag,
    SignedRawTransaction, TxConfirmationPlan, TxConfirmationPlanSummary, TxConfirmationPolicy,
    TxConfirmationStatus, TxFeePolicy, TxHash, TxPreflight, TxPreflightError, TxReceipt,
    TxReceiptSummary, TxRequestMetadata, TxRequestMetadataError, TxSubmissionPlan,
    TxSubmissionPlanBatchSummary, TxSubmissionPlanSummary, UnsignedCall, UnsignedTx,
    UnsignedTxRequest,
};

/// Caller-provided JSON-RPC transport.
///
/// The SDK intentionally leaves networking, authentication, retries, and
/// observability to the application. Implement this trait for the HTTP client,
/// WebSocket client, relayer, wallet service, or in-memory harness that should
/// execute SDK-built JSON-RPC envelopes.
pub trait JsonRpcTransport {
    type Error;

    fn send<T: DeserializeOwned + Default>(
        &mut self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse<T>, Self::Error>;

    fn send_batch<T: DeserializeOwned + Default>(
        &mut self,
        requests: &[JsonRpcRequest],
    ) -> Result<Vec<JsonRpcResponse<T>>, Self::Error> {
        requests.iter().map(|request| self.send(request)).collect()
    }
}

/// Retry policy for transport-neutral JSON-RPC execution.
///
/// The SDK still does not sleep, spawn tasks, or choose an HTTP client. This
/// policy only tells [`RetryingJsonRpcTransport`] which failed attempts are safe
/// to replay immediately; callers own any outer backoff or rate-limit timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRetryPolicy {
    pub max_attempts: u32,
    pub retry_transport_errors: bool,
    pub retry_server_errors: bool,
    pub retry_rate_limited: bool,
}

/// Attempt counters recorded by [`RetryingJsonRpcTransport`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRetryStats {
    pub single_attempts: u64,
    pub batch_attempts: u64,
    pub single_retries: u64,
    pub batch_retries: u64,
}

/// Capped exponential backoff values for caller-managed retry scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcBackoffPolicy {
    pub base_delay_millis: u64,
    pub max_delay_millis: u64,
}

/// JSON-RPC transport wrapper that replays retryable attempts.
#[derive(Debug, Clone)]
pub struct RetryingJsonRpcTransport<T> {
    inner: T,
    policy: JsonRpcRetryPolicy,
    stats: JsonRpcRetryStats,
}

/// Errors surfaced by [`JsonRpcExecutor`].
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum JsonRpcExecutorError<E> {
    #[error("JSON-RPC request id overflow at batch index {index}")]
    IdOverflow { index: usize },
    #[error("failed to build JSON-RPC request")]
    Request(#[from] JsonRpcRequestError),
    #[error("JSON-RPC transport failed")]
    Transport(E),
    #[error("failed to decode JSON-RPC result")]
    Result(#[from] JsonRpcResultDecodeError),
    #[error("failed to decode JSON-RPC batch")]
    Batch(#[from] JsonRpcBatchError),
    #[error("failed to decode transaction preflight")]
    Preflight(#[from] TxPreflightError),
    #[error("failed to build transaction batch metadata")]
    BatchMetadata(#[from] TxRequestMetadataError),
}

/// Receipt plus block-number snapshot used for confirmation classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxConfirmationSnapshot {
    pub receipt: Option<TxReceipt>,
    pub current_block: Option<u64>,
    pub status: TxConfirmationStatus,
}

/// Compact serializable confirmation snapshot for persistence and logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxConfirmationSnapshotReport {
    pub transaction_hash: Option<TxHash>,
    pub current_block: Option<u64>,
    pub receipt_block_number: Option<u64>,
    pub receipt_status: Option<bool>,
    pub confirmations: Option<u64>,
    pub gas_used: Option<u64>,
    pub effective_gas_price: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_summary: Option<TxReceiptSummary>,
    pub status: TxConfirmationStatus,
}

/// Aggregate confirmation state for an ordered transaction batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxConfirmationBatchSnapshot {
    pub snapshots: Vec<TxConfirmationSnapshot>,
    pub status: TxConfirmationBatchStatus,
}

/// Compact serializable confirmation snapshot for an ordered transaction batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxConfirmationBatchReport {
    pub len: usize,
    pub is_empty: bool,
    #[serde(default)]
    pub should_continue_polling: bool,
    #[serde(default)]
    pub is_terminal: bool,
    pub status: TxConfirmationBatchStatus,
    pub reports: Vec<TxConfirmationSnapshotReport>,
}

/// Caller-facing decision for whether a submitted transaction batch is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxConfirmationBatchStatus {
    Pending {
        confirmed: usize,
        pending: usize,
        total: usize,
    },
    Confirmed {
        total: usize,
    },
    Reverted {
        index: usize,
        confirmations: u64,
    },
    TimedOut {
        index: usize,
    },
}

/// Result of signing and submitting one transaction through the SDK workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxWorkflowSubmission {
    pub plan: TxSubmissionPlan,
    pub signed_transaction: SignedRawTransaction,
    pub transaction_hash: TxHash,
    pub confirmation_plan: TxConfirmationPlan,
}

/// Compact serializable report for one submitted transaction workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxWorkflowSubmissionReport {
    pub transaction_hash: TxHash,
    pub confirmation_plan: TxConfirmationPlan,
    pub confirmation_plan_summary: TxConfirmationPlanSummary,
    pub plan_summary: TxSubmissionPlanSummary,
}

/// Result of sequentially signing and submitting multiple prepared plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxWorkflowBatchSubmission {
    pub submissions: Vec<TxWorkflowSubmission>,
}

/// Compact serializable report for an ordered transaction workflow batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxWorkflowBatchSubmissionReport {
    pub len: usize,
    pub is_empty: bool,
    #[serde(default)]
    pub has_submissions: bool,
    pub transaction_hashes: Vec<TxHash>,
    pub plan_summary: TxSubmissionPlanBatchSummary,
    pub submissions: Vec<TxWorkflowSubmissionReport>,
}

/// Serializable continuation plan for an interrupted ordered transaction batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxWorkflowBatchResumePlan {
    pub original_len: usize,
    pub submitted_len: usize,
    pub next_plan_index: usize,
    pub remaining_len: usize,
    pub is_complete: bool,
    pub submitted_exceeds_original: bool,
    pub submitted_transaction_hashes: Vec<TxHash>,
    pub submitted_report: TxWorkflowBatchSubmissionReport,
    pub remaining_plans: Vec<TxSubmissionPlan>,
    pub remaining_plan_summary: TxSubmissionPlanBatchSummary,
}

/// Compact serializable progress report for resuming an ordered transaction batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxWorkflowBatchResumePlanSummary {
    pub original_len: usize,
    pub submitted_len: usize,
    #[serde(default)]
    pub has_submitted: bool,
    pub next_plan_index: usize,
    pub remaining_len: usize,
    #[serde(default)]
    pub has_remaining: bool,
    #[serde(default)]
    pub can_continue: bool,
    pub is_complete: bool,
    pub submitted_exceeds_original: bool,
    pub submitted_transaction_hashes: Vec<TxHash>,
    pub submitted_plan_summary: TxSubmissionPlanBatchSummary,
    pub remaining_plan_summary: TxSubmissionPlanBatchSummary,
}

/// Errors surfaced by [`TxWorkflowExecutor`].
#[derive(Debug, thiserror::Error)]
pub enum TxWorkflowError<RpcError, SignerError> {
    #[error("transaction workflow RPC step failed")]
    Rpc(JsonRpcExecutorError<RpcError>),
    #[error("transaction workflow signer step failed")]
    Signer(SignerError),
}

/// Small typed executor over a caller-provided JSON-RPC transport.
#[derive(Debug, Clone)]
pub struct JsonRpcExecutor<T> {
    transport: T,
    next_id: u64,
}

/// Transaction workflow over caller-provided RPC and signer backends.
#[derive(Debug, Clone)]
pub struct TxWorkflowExecutor<T, S> {
    rpc: JsonRpcExecutor<T>,
    signer: S,
}

impl Default for JsonRpcRetryPolicy {
    fn default() -> Self {
        Self::new(3)
    }
}

impl JsonRpcRetryPolicy {
    #[must_use]
    pub const fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            retry_transport_errors: true,
            retry_server_errors: true,
            retry_rate_limited: true,
        }
    }

    #[must_use]
    pub const fn no_retries() -> Self {
        Self {
            max_attempts: 1,
            retry_transport_errors: false,
            retry_server_errors: false,
            retry_rate_limited: false,
        }
    }

    #[must_use]
    pub const fn with_retry_transport_errors(mut self, retry_transport_errors: bool) -> Self {
        self.retry_transport_errors = retry_transport_errors;
        self
    }

    #[must_use]
    pub const fn with_retry_server_errors(mut self, retry_server_errors: bool) -> Self {
        self.retry_server_errors = retry_server_errors;
        self
    }

    #[must_use]
    pub const fn with_retry_rate_limited(mut self, retry_rate_limited: bool) -> Self {
        self.retry_rate_limited = retry_rate_limited;
        self
    }

    #[must_use]
    pub const fn attempts(self) -> u32 {
        if self.max_attempts == 0 {
            1
        } else {
            self.max_attempts
        }
    }

    #[must_use]
    pub fn should_retry_transport_error(self) -> bool {
        self.retry_transport_errors
    }

    #[must_use]
    pub fn should_retry_rpc_error(self, error: &JsonRpcErrorObject) -> bool {
        (self.retry_rate_limited && is_rate_limited_error(error))
            || (self.retry_server_errors && is_server_error(error))
    }
}

impl JsonRpcBackoffPolicy {
    #[must_use]
    pub const fn new(base_delay_millis: u64, max_delay_millis: u64) -> Self {
        Self {
            base_delay_millis,
            max_delay_millis,
        }
    }

    #[must_use]
    pub const fn no_delay() -> Self {
        Self {
            base_delay_millis: 0,
            max_delay_millis: 0,
        }
    }

    /// Return the capped delay for a retry attempt.
    ///
    /// `retry_index` is zero-based: `0` is the first retry after the initial
    /// failed attempt, `1` is the second retry, and so on.
    #[must_use]
    pub const fn delay_millis(self, retry_index: u32) -> u64 {
        if self.base_delay_millis == 0 || self.max_delay_millis == 0 {
            return 0;
        }

        let multiplier = if retry_index >= 63 {
            u64::MAX
        } else {
            1u64 << retry_index
        };
        let delay = self.base_delay_millis.saturating_mul(multiplier);

        if delay > self.max_delay_millis {
            self.max_delay_millis
        } else {
            delay
        }
    }
}

impl<T> RetryingJsonRpcTransport<T> {
    #[must_use]
    pub const fn new(inner: T, policy: JsonRpcRetryPolicy) -> Self {
        Self {
            inner,
            policy,
            stats: JsonRpcRetryStats {
                single_attempts: 0,
                batch_attempts: 0,
                single_retries: 0,
                batch_retries: 0,
            },
        }
    }

    #[must_use]
    pub const fn with_default_policy(inner: T) -> Self {
        Self::new(inner, JsonRpcRetryPolicy::new(3))
    }

    #[must_use]
    pub const fn policy(&self) -> JsonRpcRetryPolicy {
        self.policy
    }

    #[must_use]
    pub const fn stats(&self) -> JsonRpcRetryStats {
        self.stats
    }

    #[must_use]
    pub const fn inner(&self) -> &T {
        &self.inner
    }

    #[must_use]
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: JsonRpcTransport> JsonRpcTransport for RetryingJsonRpcTransport<T> {
    type Error = T::Error;

    fn send<R: DeserializeOwned + Default>(
        &mut self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse<R>, Self::Error> {
        let attempts = self.policy.attempts();
        let mut attempt = 1;

        loop {
            self.stats.single_attempts = self.stats.single_attempts.saturating_add(1);
            match self.inner.send(request) {
                Ok(response) => {
                    if response
                        .error
                        .as_ref()
                        .is_some_and(|error| self.policy.should_retry_rpc_error(error))
                        && attempt < attempts
                    {
                        self.stats.single_retries = self.stats.single_retries.saturating_add(1);
                        attempt += 1;
                        continue;
                    }
                    return Ok(response);
                }
                Err(error) => {
                    if self.policy.should_retry_transport_error() && attempt < attempts {
                        self.stats.single_retries = self.stats.single_retries.saturating_add(1);
                        attempt += 1;
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    fn send_batch<R: DeserializeOwned + Default>(
        &mut self,
        requests: &[JsonRpcRequest],
    ) -> Result<Vec<JsonRpcResponse<R>>, Self::Error> {
        let attempts = self.policy.attempts();
        let mut attempt = 1;

        loop {
            self.stats.batch_attempts = self.stats.batch_attempts.saturating_add(1);
            match self.inner.send_batch(requests) {
                Ok(responses) => {
                    if responses.iter().any(|response| {
                        response
                            .error
                            .as_ref()
                            .is_some_and(|error| self.policy.should_retry_rpc_error(error))
                    }) && attempt < attempts
                    {
                        self.stats.batch_retries = self.stats.batch_retries.saturating_add(1);
                        attempt += 1;
                        continue;
                    }
                    return Ok(responses);
                }
                Err(error) => {
                    if self.policy.should_retry_transport_error() && attempt < attempts {
                        self.stats.batch_retries = self.stats.batch_retries.saturating_add(1);
                        attempt += 1;
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }
}

impl TxWorkflowSubmission {
    #[must_use]
    pub fn report(&self) -> TxWorkflowSubmissionReport {
        TxWorkflowSubmissionReport {
            transaction_hash: self.transaction_hash,
            confirmation_plan: self.confirmation_plan,
            confirmation_plan_summary: self.confirmation_plan.summary(),
            plan_summary: self.plan.summary(),
        }
    }
}

impl TxWorkflowBatchSubmission {
    #[must_use]
    pub const fn new(submissions: Vec<TxWorkflowSubmission>) -> Self {
        Self { submissions }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.submissions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.submissions.is_empty()
    }

    #[must_use]
    pub fn transaction_hashes(&self) -> Vec<TxHash> {
        self.submissions
            .iter()
            .map(|submission| submission.transaction_hash)
            .collect()
    }

    #[must_use]
    pub fn confirmation_plans(&self) -> Vec<TxConfirmationPlan> {
        self.submissions
            .iter()
            .map(|submission| submission.confirmation_plan)
            .collect()
    }

    #[must_use]
    pub fn report(&self) -> TxWorkflowBatchSubmissionReport {
        let submissions = self
            .submissions
            .iter()
            .map(TxWorkflowSubmission::report)
            .collect::<Vec<_>>();
        let plans = self
            .submissions
            .iter()
            .map(|submission| submission.plan.clone())
            .collect::<Vec<_>>();

        TxWorkflowBatchSubmissionReport {
            len: self.len(),
            is_empty: self.is_empty(),
            has_submissions: !self.is_empty(),
            transaction_hashes: self.transaction_hashes(),
            plan_summary: TxSubmissionPlan::summarize_batch(&plans),
            submissions,
        }
    }

    #[must_use]
    pub fn resume_plan(&self, original_plans: &[TxSubmissionPlan]) -> TxWorkflowBatchResumePlan {
        TxWorkflowBatchResumePlan::from_submission(original_plans, self)
    }
}

impl TxWorkflowBatchResumePlan {
    #[must_use]
    pub fn from_submission(
        original_plans: &[TxSubmissionPlan],
        submitted: &TxWorkflowBatchSubmission,
    ) -> Self {
        let original_len = original_plans.len();
        let submitted_len = submitted.len();
        let next_plan_index = submitted_len.min(original_len);
        let remaining_plans = original_plans[next_plan_index..].to_vec();
        let submitted_exceeds_original = submitted_len > original_len;
        let is_complete = submitted_len == original_len;
        let remaining_plan_summary = TxSubmissionPlan::summarize_batch(&remaining_plans);

        Self {
            original_len,
            submitted_len,
            next_plan_index,
            remaining_len: remaining_plans.len(),
            is_complete,
            submitted_exceeds_original,
            submitted_transaction_hashes: submitted.transaction_hashes(),
            submitted_report: submitted.report(),
            remaining_plans,
            remaining_plan_summary,
        }
    }

    #[must_use]
    pub fn summary(&self) -> TxWorkflowBatchResumePlanSummary {
        TxWorkflowBatchResumePlanSummary {
            original_len: self.original_len,
            submitted_len: self.submitted_len,
            has_submitted: self.submitted_len > 0,
            next_plan_index: self.next_plan_index,
            remaining_len: self.remaining_len,
            has_remaining: self.remaining_len > 0,
            can_continue: self.can_continue(),
            is_complete: self.is_complete,
            submitted_exceeds_original: self.submitted_exceeds_original,
            submitted_transaction_hashes: self.submitted_transaction_hashes.clone(),
            submitted_plan_summary: self.submitted_report.plan_summary.clone(),
            remaining_plan_summary: self.remaining_plan_summary.clone(),
        }
    }

    #[must_use]
    pub const fn can_continue(&self) -> bool {
        self.remaining_len > 0 && !self.submitted_exceeds_original
    }
}

impl TxConfirmationSnapshot {
    #[must_use]
    pub const fn should_continue_polling(&self) -> bool {
        self.status.should_continue_polling()
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    #[must_use]
    pub fn report(
        &self,
        confirmation_plan: Option<TxConfirmationPlan>,
    ) -> TxConfirmationSnapshotReport {
        TxConfirmationSnapshotReport {
            transaction_hash: confirmation_plan
                .map(|plan| plan.transaction_hash)
                .or_else(|| {
                    self.receipt
                        .as_ref()
                        .map(|receipt| receipt.transaction_hash)
                }),
            current_block: self.current_block,
            receipt_block_number: self
                .receipt
                .as_ref()
                .and_then(|receipt| receipt.block_number),
            receipt_status: self.receipt.as_ref().and_then(|receipt| receipt.status),
            confirmations: self.status.confirmations(),
            gas_used: self.receipt.as_ref().and_then(|receipt| receipt.gas_used),
            effective_gas_price: self
                .receipt
                .as_ref()
                .and_then(|receipt| receipt.effective_gas_price),
            receipt_summary: self.receipt.as_ref().map(TxReceipt::summary),
            status: self.status,
        }
    }
}

impl TxConfirmationBatchSnapshot {
    #[must_use]
    pub fn new(snapshots: Vec<TxConfirmationSnapshot>) -> Self {
        let status = TxConfirmationBatchStatus::from_snapshots(&snapshots);
        Self { snapshots, status }
    }

    #[must_use]
    pub const fn should_continue_polling(&self) -> bool {
        self.status.should_continue_polling()
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    #[must_use]
    pub fn report(&self, confirmation_plans: &[TxConfirmationPlan]) -> TxConfirmationBatchReport {
        let reports = self
            .snapshots
            .iter()
            .enumerate()
            .map(|(index, snapshot)| snapshot.report(confirmation_plans.get(index).copied()))
            .collect::<Vec<_>>();

        TxConfirmationBatchReport {
            len: reports.len(),
            is_empty: reports.is_empty(),
            should_continue_polling: self.should_continue_polling(),
            is_terminal: self.is_terminal(),
            status: self.status,
            reports,
        }
    }
}

impl TxConfirmationBatchStatus {
    #[must_use]
    pub fn from_snapshots(snapshots: &[TxConfirmationSnapshot]) -> Self {
        let total = snapshots.len();
        let mut confirmed = 0usize;
        let mut pending = 0usize;

        for (index, snapshot) in snapshots.iter().enumerate() {
            match snapshot.status {
                TxConfirmationStatus::Pending { .. } => pending += 1,
                TxConfirmationStatus::Confirmed { .. } => confirmed += 1,
                TxConfirmationStatus::Reverted { confirmations } => {
                    return Self::Reverted {
                        index,
                        confirmations,
                    };
                }
                TxConfirmationStatus::TimedOut => return Self::TimedOut { index },
            }
        }

        if confirmed == total {
            Self::Confirmed { total }
        } else {
            Self::Pending {
                confirmed,
                pending,
                total,
            }
        }
    }

    #[must_use]
    pub const fn should_continue_polling(self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        !self.should_continue_polling()
    }
}

impl<T> JsonRpcExecutor<T> {
    #[must_use]
    pub const fn new(transport: T) -> Self {
        Self {
            transport,
            next_id: 1,
        }
    }

    #[must_use]
    pub const fn with_next_id(transport: T, next_id: u64) -> Self {
        Self { transport, next_id }
    }

    #[must_use]
    pub const fn next_id(&self) -> u64 {
        self.next_id
    }

    #[must_use]
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    #[must_use]
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T, S> TxWorkflowExecutor<T, S> {
    #[must_use]
    pub const fn new(rpc: JsonRpcExecutor<T>, signer: S) -> Self {
        Self { rpc, signer }
    }

    #[must_use]
    pub const fn with_transport(transport: T, signer: S) -> Self {
        Self {
            rpc: JsonRpcExecutor::new(transport),
            signer,
        }
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
    pub const fn signer(&self) -> &S {
        &self.signer
    }

    #[must_use]
    pub fn signer_mut(&mut self) -> &mut S {
        &mut self.signer
    }

    #[must_use]
    pub fn into_parts(self) -> (JsonRpcExecutor<T>, S) {
        (self.rpc, self.signer)
    }
}

impl<T: JsonRpcTransport> JsonRpcExecutor<T> {
    /// Execute one SDK call as `eth_call`.
    pub fn call(
        &mut self,
        call: &UnsignedCall,
        block: RpcBlockTag,
    ) -> Result<CallReturn, JsonRpcExecutorError<T::Error>> {
        let request = JsonRpcRequest::eth_call_at(self.allocate_id()?, call, block);
        self.send_string(&request)?
            .into_call_return()
            .map_err(Into::into)
    }

    /// Execute a fixed-order SDK read batch as multiple `eth_call` requests.
    pub fn call_batch(
        &mut self,
        calls: &[UnsignedCall],
        block: RpcBlockTag,
    ) -> Result<CallReturnBatch, JsonRpcExecutorError<T::Error>> {
        if calls.is_empty() {
            return Ok(CallReturnBatch::new(Vec::new()));
        }

        let first_id = self.allocate_id_batch(calls.len())?;
        let requests = JsonRpcRequest::eth_call_batch(calls, block, first_id)?;
        let responses = self
            .transport
            .send_batch::<String>(&requests)
            .map_err(JsonRpcExecutorError::Transport)?;
        JsonRpcResponse::into_call_return_batch_for_requests(responses, &requests)
            .map_err(Into::into)
    }

    /// Execute one `eth_getLogs` query.
    pub fn logs(
        &mut self,
        query: &EventLogRpcQuery,
    ) -> Result<Vec<RawLog>, JsonRpcExecutorError<T::Error>> {
        let request = JsonRpcRequest::eth_get_logs(self.allocate_id()?, query);
        self.transport
            .send::<Vec<RawLog>>(&request)
            .map_err(JsonRpcExecutorError::Transport)?
            .into_result()
            .map_err(JsonRpcResultDecodeError::from)
            .map_err(Into::into)
    }

    /// Fetch common transaction preflight values for a prepared request.
    ///
    /// The method fetches chain id, nonce, gas estimate, legacy gas price, and
    /// priority fee. The returned [`TxPreflight`] leaves `max_fee_per_gas`
    /// unset because deriving a max fee policy from base-fee history is a
    /// caller-owned policy decision.
    pub fn preflight_transaction(
        &mut self,
        request: &UnsignedTxRequest,
        from: Address,
        nonce_block: RpcBlockTag,
    ) -> Result<TxPreflight, JsonRpcExecutorError<T::Error>> {
        let chain_id_request = JsonRpcRequest::eth_chain_id(self.allocate_id()?);
        let chain_id = self.send_string(&chain_id_request)?;
        let nonce_request =
            JsonRpcRequest::eth_get_transaction_count(self.allocate_id()?, from, nonce_block);
        let nonce = self.send_string(&nonce_request)?;
        let gas_request = JsonRpcRequest::eth_estimate_gas(self.allocate_id()?, request);
        let gas = self.send_string(&gas_request)?;
        let gas_price_request = JsonRpcRequest::eth_gas_price(self.allocate_id()?);
        let gas_price = self.send_string(&gas_price_request)?;
        let max_priority_fee_per_gas_request =
            JsonRpcRequest::eth_max_priority_fee_per_gas(self.allocate_id()?);
        let max_priority_fee_per_gas = self.send_string(&max_priority_fee_per_gas_request)?;

        TxPreflight::from_rpc_responses(
            Some(chain_id),
            Some(nonce),
            Some(gas),
            Some(gas_price),
            None,
            Some(max_priority_fee_per_gas),
        )
        .map_err(Into::into)
    }

    /// Build ordered submission plans for a multi-transaction workflow.
    ///
    /// Chain id, starting nonce, gas price, and priority fee are fetched once.
    /// Gas is estimated per transaction, then nonces are assigned as
    /// `start_nonce + index`. The returned plans are ready for external
    /// signing and raw submission.
    pub fn preflight_transaction_plans(
        &mut self,
        txs: &[UnsignedTx],
        from: Address,
        nonce_block: RpcBlockTag,
        fee_policy: TxFeePolicy,
        confirmation_policy: TxConfirmationPolicy,
    ) -> Result<Vec<TxSubmissionPlan>, JsonRpcExecutorError<T::Error>> {
        if txs.is_empty() {
            return Ok(Vec::new());
        }

        let chain_id_request = JsonRpcRequest::eth_chain_id(self.allocate_id()?);
        let chain_id = self.send_string(&chain_id_request)?;
        let nonce_request =
            JsonRpcRequest::eth_get_transaction_count(self.allocate_id()?, from, nonce_block);
        let nonce = self.send_string(&nonce_request)?;
        let gas_price_request = JsonRpcRequest::eth_gas_price(self.allocate_id()?);
        let gas_price = self.send_string(&gas_price_request)?;
        let max_priority_fee_per_gas_request =
            JsonRpcRequest::eth_max_priority_fee_per_gas(self.allocate_id()?);
        let max_priority_fee_per_gas = self.send_string(&max_priority_fee_per_gas_request)?;
        let common = TxPreflight::from_rpc_responses(
            Some(chain_id),
            Some(nonce),
            None,
            Some(gas_price),
            None,
            Some(max_priority_fee_per_gas),
        )?
        .with_fee_policy(fee_policy);

        txs.iter()
            .enumerate()
            .map(|(index, tx)| {
                let request =
                    tx.to_tx_request_with_metadata(TxRequestMetadata::new().with_from(from));
                let gas_request = JsonRpcRequest::eth_estimate_gas(self.allocate_id()?, &request);
                let gas = self
                    .send_string(&gas_request)?
                    .into_quantity_u64()
                    .map_err(JsonRpcExecutorError::from)?;
                let index_u64 = u64::try_from(index)
                    .map_err(|_| TxRequestMetadataError::NonceOverflow { index })?;
                let nonce = common
                    .nonce
                    .map(|nonce| {
                        nonce
                            .checked_add(index_u64)
                            .ok_or(TxRequestMetadataError::NonceOverflow { index })
                    })
                    .transpose()?;
                let preflight = TxPreflight {
                    gas: Some(gas),
                    nonce,
                    ..common
                };
                Ok(TxSubmissionPlan::from_unsigned_tx(
                    tx,
                    Some(from),
                    preflight,
                    confirmation_policy,
                ))
            })
            .collect()
    }

    /// Execute `eth_sendTransaction` for node-managed signing.
    pub fn send_transaction(
        &mut self,
        plan: &TxSubmissionPlan,
    ) -> Result<TxHash, JsonRpcExecutorError<T::Error>> {
        let request = plan.send_transaction_request(self.allocate_id()?);
        self.send_string(&request)?
            .into_tx_hash()
            .map_err(Into::into)
    }

    /// Execute `eth_sendRawTransaction` for externally signed bytes.
    pub fn send_raw_transaction(
        &mut self,
        plan: &TxSubmissionPlan,
        signed_transaction: &SignedRawTransaction,
    ) -> Result<TxHash, JsonRpcExecutorError<T::Error>> {
        let request = plan.send_raw_transaction_request(self.allocate_id()?, signed_transaction);
        self.send_string(&request)?
            .into_tx_hash()
            .map_err(Into::into)
    }

    /// Fetch receipt/block snapshots and classify confirmation status.
    pub fn confirmation_snapshot(
        &mut self,
        plan: &TxConfirmationPlan,
    ) -> Result<TxConfirmationSnapshot, JsonRpcExecutorError<T::Error>> {
        let receipt_request = plan.receipt_request(self.allocate_id()?);
        let block_request = plan.block_number_request(self.allocate_id()?);
        let receipt = self
            .transport
            .send::<Option<TxReceipt>>(&receipt_request)
            .map_err(JsonRpcExecutorError::Transport)?
            .into_result()
            .map_err(JsonRpcResultDecodeError::from)?;
        let current_block = self
            .send_string(&block_request)?
            .into_quantity_u64()
            .map_err(JsonRpcExecutorError::from)?;

        Ok(TxConfirmationSnapshot {
            receipt: receipt.clone(),
            current_block: Some(current_block),
            status: plan.classify(receipt.as_ref(), Some(current_block)),
        })
    }

    fn send_string(
        &mut self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse<String>, JsonRpcExecutorError<T::Error>> {
        self.transport
            .send(request)
            .map_err(JsonRpcExecutorError::Transport)
    }

    fn allocate_id(&mut self) -> Result<u64, JsonRpcExecutorError<T::Error>> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(JsonRpcExecutorError::IdOverflow { index: 1 })?;
        Ok(id)
    }

    fn allocate_id_batch(&mut self, len: usize) -> Result<u64, JsonRpcExecutorError<T::Error>> {
        let first_id = self.next_id;
        let len_u64 =
            u64::try_from(len).map_err(|_| JsonRpcExecutorError::IdOverflow { index: len })?;
        self.next_id = self
            .next_id
            .checked_add(len_u64)
            .ok_or(JsonRpcExecutorError::IdOverflow { index: len })?;
        Ok(first_id)
    }
}

impl<T, S> TxWorkflowExecutor<T, S>
where
    T: JsonRpcTransport,
    S: RawTransactionSigner,
{
    /// Sign a prepared submission plan, broadcast the raw transaction, and
    /// return the hash plus confirmation plan.
    pub fn submit_raw_plan(
        &mut self,
        plan: &TxSubmissionPlan,
    ) -> Result<TxWorkflowSubmission, TxWorkflowError<T::Error, S::Error>> {
        let signed_transaction = plan
            .sign_with(&mut self.signer)
            .map_err(TxWorkflowError::Signer)?;
        let transaction_hash = self
            .rpc
            .send_raw_transaction(plan, &signed_transaction)
            .map_err(TxWorkflowError::Rpc)?;
        let confirmation_plan = plan.confirmation_plan(transaction_hash);

        Ok(TxWorkflowSubmission {
            plan: plan.clone(),
            signed_transaction,
            transaction_hash,
            confirmation_plan,
        })
    }

    /// Sign and broadcast prepared submission plans in order.
    ///
    /// The workflow stops at the first signer or RPC error. Callers that need
    /// retry/resume behavior can keep the original plan slice and continue from
    /// the first plan that did not produce a submission.
    pub fn submit_raw_plans(
        &mut self,
        plans: &[TxSubmissionPlan],
    ) -> Result<TxWorkflowBatchSubmission, TxWorkflowError<T::Error, S::Error>> {
        let mut submissions = Vec::with_capacity(plans.len());
        for plan in plans {
            submissions.push(self.submit_raw_plan(plan)?);
        }
        Ok(TxWorkflowBatchSubmission::new(submissions))
    }

    /// Preflight an SDK unsigned transaction, sign it, and broadcast it through
    /// `eth_sendRawTransaction`.
    pub fn preflight_sign_and_submit(
        &mut self,
        tx: &UnsignedTx,
        from: Address,
        nonce_block: RpcBlockTag,
        confirmation_policy: TxConfirmationPolicy,
    ) -> Result<TxWorkflowSubmission, TxWorkflowError<T::Error, S::Error>> {
        let preflight_request = tx.to_tx_request();
        let preflight = self
            .rpc
            .preflight_transaction(&preflight_request, from, nonce_block)
            .map_err(TxWorkflowError::Rpc)?;
        let plan =
            TxSubmissionPlan::from_unsigned_tx(tx, Some(from), preflight, confirmation_policy);
        self.submit_raw_plan(&plan)
    }

    /// Preflight an SDK unsigned transaction, apply a local fee policy, sign it,
    /// and broadcast it through `eth_sendRawTransaction`.
    pub fn preflight_sign_and_submit_with_fee_policy(
        &mut self,
        tx: &UnsignedTx,
        from: Address,
        nonce_block: RpcBlockTag,
        fee_policy: TxFeePolicy,
        confirmation_policy: TxConfirmationPolicy,
    ) -> Result<TxWorkflowSubmission, TxWorkflowError<T::Error, S::Error>> {
        let preflight_request = tx.to_tx_request();
        let preflight = self
            .rpc
            .preflight_transaction(&preflight_request, from, nonce_block)
            .map_err(TxWorkflowError::Rpc)?;
        let plan = TxSubmissionPlan::from_unsigned_tx_with_fee_policy(
            tx,
            Some(from),
            preflight,
            fee_policy,
            confirmation_policy,
        );
        self.submit_raw_plan(&plan)
    }

    /// Preflight SDK unsigned transactions, sign them, and broadcast them
    /// through `eth_sendRawTransaction` with preserved provider fee fields.
    pub fn preflight_sign_and_submit_batch(
        &mut self,
        txs: &[UnsignedTx],
        from: Address,
        nonce_block: RpcBlockTag,
        confirmation_policy: TxConfirmationPolicy,
    ) -> Result<TxWorkflowBatchSubmission, TxWorkflowError<T::Error, S::Error>> {
        self.preflight_sign_and_submit_batch_with_fee_policy(
            txs,
            from,
            nonce_block,
            TxFeePolicy::Preserve,
            confirmation_policy,
        )
    }

    /// Preflight SDK unsigned transactions as one ordered workflow, then sign
    /// and broadcast each raw transaction in sequence.
    pub fn preflight_sign_and_submit_batch_with_fee_policy(
        &mut self,
        txs: &[UnsignedTx],
        from: Address,
        nonce_block: RpcBlockTag,
        fee_policy: TxFeePolicy,
        confirmation_policy: TxConfirmationPolicy,
    ) -> Result<TxWorkflowBatchSubmission, TxWorkflowError<T::Error, S::Error>> {
        let plans = self
            .rpc
            .preflight_transaction_plans(txs, from, nonce_block, fee_policy, confirmation_policy)
            .map_err(TxWorkflowError::Rpc)?;
        self.submit_raw_plans(&plans)
    }

    /// Fetch the latest confirmation snapshot for a submitted transaction.
    pub fn confirmation_snapshot(
        &mut self,
        submission: &TxWorkflowSubmission,
    ) -> Result<TxConfirmationSnapshot, TxWorkflowError<T::Error, S::Error>> {
        self.rpc
            .confirmation_snapshot(&submission.confirmation_plan)
            .map_err(TxWorkflowError::Rpc)
    }

    /// Fetch and compactly summarize the confirmation snapshot for a submitted transaction.
    pub fn confirmation_report(
        &mut self,
        submission: &TxWorkflowSubmission,
    ) -> Result<TxConfirmationSnapshotReport, TxWorkflowError<T::Error, S::Error>> {
        Ok(self
            .confirmation_snapshot(submission)?
            .report(Some(submission.confirmation_plan)))
    }

    /// Fetch confirmation snapshots for each submitted transaction in order.
    pub fn confirmation_snapshots(
        &mut self,
        batch: &TxWorkflowBatchSubmission,
    ) -> Result<Vec<TxConfirmationSnapshot>, TxWorkflowError<T::Error, S::Error>> {
        batch
            .submissions
            .iter()
            .map(|submission| self.confirmation_snapshot(submission))
            .collect()
    }

    /// Fetch confirmation snapshots and aggregate the ordered batch status.
    pub fn confirmation_batch_snapshot(
        &mut self,
        batch: &TxWorkflowBatchSubmission,
    ) -> Result<TxConfirmationBatchSnapshot, TxWorkflowError<T::Error, S::Error>> {
        Ok(TxConfirmationBatchSnapshot::new(
            self.confirmation_snapshots(batch)?,
        ))
    }

    /// Fetch confirmation snapshots and compactly summarize ordered batch status.
    pub fn confirmation_batch_report(
        &mut self,
        batch: &TxWorkflowBatchSubmission,
    ) -> Result<TxConfirmationBatchReport, TxWorkflowError<T::Error, S::Error>> {
        Ok(self
            .confirmation_batch_snapshot(batch)?
            .report(&batch.confirmation_plans()))
    }
}

fn is_rate_limited_error(error: &JsonRpcErrorObject) -> bool {
    error.code == 429 || error.code == -32005
}

fn is_server_error(error: &JsonRpcErrorObject) -> bool {
    (-32099..=-32000).contains(&error.code) || error.code == -32603
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::{RawTransactionSigner, RawTransactionSigningRequest};
    use crate::tx::{TxConfirmationPolicy, TxPreflight, TxSubmissionPlan};
    use alloy_primitives::B256;
    use serde_json::json;
    use std::collections::VecDeque;

    #[derive(Debug, Clone, Default)]
    struct MockTransport {
        single_responses: VecDeque<serde_json::Value>,
        batch_responses: VecDeque<Vec<serde_json::Value>>,
        seen: Vec<JsonRpcRequest>,
    }

    impl MockTransport {
        fn with_single_responses(responses: impl IntoIterator<Item = serde_json::Value>) -> Self {
            Self {
                single_responses: responses.into_iter().collect(),
                batch_responses: VecDeque::new(),
                seen: Vec::new(),
            }
        }

        fn with_batch_response(responses: impl IntoIterator<Item = serde_json::Value>) -> Self {
            Self {
                single_responses: VecDeque::new(),
                batch_responses: VecDeque::from([responses.into_iter().collect()]),
                seen: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    struct FlakyTransport {
        single_outcomes: VecDeque<Result<serde_json::Value, String>>,
        seen: Vec<JsonRpcRequest>,
    }

    impl FlakyTransport {
        fn with_single_outcomes(
            outcomes: impl IntoIterator<Item = Result<serde_json::Value, String>>,
        ) -> Self {
            Self {
                single_outcomes: outcomes.into_iter().collect(),
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
            let value = self
                .single_responses
                .pop_front()
                .ok_or_else(|| format!("missing response for {}", request.method))?;
            serde_json::from_value(value).map_err(|error| error.to_string())
        }

        fn send_batch<T: DeserializeOwned + Default>(
            &mut self,
            requests: &[JsonRpcRequest],
        ) -> Result<Vec<JsonRpcResponse<T>>, Self::Error> {
            self.seen.extend(requests.iter().cloned());
            let values = self
                .batch_responses
                .pop_front()
                .ok_or_else(|| "missing batch response".to_owned())?;
            values
                .into_iter()
                .map(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
                .collect()
        }
    }

    impl JsonRpcTransport for FlakyTransport {
        type Error = String;

        fn send<T: DeserializeOwned + Default>(
            &mut self,
            request: &JsonRpcRequest,
        ) -> Result<JsonRpcResponse<T>, Self::Error> {
            self.seen.push(request.clone());
            let value = self
                .single_outcomes
                .pop_front()
                .ok_or_else(|| format!("missing response for {}", request.method))??;
            serde_json::from_value(value).map_err(|error| error.to_string())
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

    #[test]
    fn executes_call_batch_and_restores_request_order() {
        let first = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0xaa],
        };
        let second = UnsignedCall {
            to: Address::repeat_byte(0x22),
            data: vec![0xbb],
        };
        let transport = MockTransport::with_batch_response([
            json!({"jsonrpc":"2.0","id":2,"result":"0x02"}),
            json!({"jsonrpc":"2.0","id":1,"result":"0x01"}),
        ]);
        let mut executor = JsonRpcExecutor::new(transport);

        let returns = executor
            .call_batch(&[first, second], RpcBlockTag::Finalized)
            .expect("batch call returns");

        assert_eq!(returns.data_hexes(), vec!["0x01", "0x02"]);
        let transport = executor.into_transport();
        assert_eq!(transport.seen.len(), 2);
        assert_eq!(transport.seen[0].id, 1);
        assert_eq!(transport.seen[1].id, 2);
        assert_eq!(transport.seen[0].method, "eth_call");
    }

    #[test]
    fn backoff_policy_returns_capped_exponential_delays() {
        let policy = JsonRpcBackoffPolicy::new(250, 2_000);

        assert_eq!(policy.delay_millis(0), 250);
        assert_eq!(policy.delay_millis(1), 500);
        assert_eq!(policy.delay_millis(2), 1_000);
        assert_eq!(policy.delay_millis(3), 2_000);
        assert_eq!(policy.delay_millis(4), 2_000);
        assert_eq!(policy.delay_millis(63), 2_000);
        assert_eq!(JsonRpcBackoffPolicy::no_delay().delay_millis(0), 0);
    }

    #[test]
    fn retrying_transport_retries_transport_errors_before_success() {
        let call = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0xaa],
        };
        let transport = FlakyTransport::with_single_outcomes([
            Err("connection reset".to_owned()),
            Ok(json!({"jsonrpc":"2.0","id":1,"result":"0x1234"})),
        ]);
        let retrying = RetryingJsonRpcTransport::new(transport, JsonRpcRetryPolicy::new(3));
        let mut executor = JsonRpcExecutor::new(retrying);

        let returned = executor
            .call(&call, RpcBlockTag::Latest)
            .expect("retry succeeds");

        assert_eq!(returned.data_hex(), "0x1234");
        let retrying = executor.into_transport();
        assert_eq!(
            retrying.stats(),
            JsonRpcRetryStats {
                single_attempts: 2,
                batch_attempts: 0,
                single_retries: 1,
                batch_retries: 0,
            }
        );
        assert_eq!(retrying.inner().seen.len(), 2);
    }

    #[test]
    fn retrying_transport_retries_retryable_rpc_errors_before_success() {
        let call = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0xaa],
        };
        let transport = FlakyTransport::with_single_outcomes([
            Ok(json!({
                "jsonrpc":"2.0",
                "id":1,
                "error":{"code":-32005,"message":"rate limited"}
            })),
            Ok(json!({"jsonrpc":"2.0","id":1,"result":"0xabcd"})),
        ]);
        let retrying = RetryingJsonRpcTransport::new(transport, JsonRpcRetryPolicy::new(2));
        let mut executor = JsonRpcExecutor::new(retrying);

        let returned = executor
            .call(&call, RpcBlockTag::Latest)
            .expect("retry succeeds");

        assert_eq!(returned.data_hex(), "0xabcd");
        let retrying = executor.into_transport();
        assert_eq!(retrying.stats().single_attempts, 2);
        assert_eq!(retrying.stats().single_retries, 1);
    }

    #[test]
    fn retrying_transport_does_not_retry_non_retryable_rpc_errors() {
        let call = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0xaa],
        };
        let transport = FlakyTransport::with_single_outcomes([Ok(json!({
            "jsonrpc":"2.0",
            "id":1,
            "error":{"code":-32602,"message":"invalid params"}
        }))]);
        let retrying = RetryingJsonRpcTransport::new(transport, JsonRpcRetryPolicy::new(3));
        let mut executor = JsonRpcExecutor::new(retrying);

        let error = executor
            .call(&call, RpcBlockTag::Latest)
            .expect_err("invalid params is not retried");

        assert!(matches!(error, JsonRpcExecutorError::Result(_)));
        let retrying = executor.into_transport();
        assert_eq!(retrying.stats().single_attempts, 1);
        assert_eq!(retrying.stats().single_retries, 0);
    }

    #[test]
    fn executes_transaction_preflight_and_submission() {
        let from = Address::repeat_byte(0x33);
        let request = UnsignedTxRequest {
            from: Some(from),
            to: Address::repeat_byte(0x44),
            data: "0x12345678".to_owned(),
            value: "0x0".to_owned(),
            nonce: None,
            gas: None,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: None,
        };
        let hash_hex = format!("0x{}", "55".repeat(32));
        let transport = MockTransport::with_single_responses([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x3d090"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":6,"result": hash_hex}),
        ]);
        let mut executor = JsonRpcExecutor::new(transport);

        let preflight = executor
            .preflight_transaction(&request, from, RpcBlockTag::Pending)
            .expect("preflight decodes");
        assert_eq!(
            preflight,
            TxPreflight {
                chain_id: Some(11111),
                nonce: Some(7),
                gas: Some(250_000),
                gas_price: Some(2_000_000_000),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: Some(1_000_000_000),
            }
        );

        let plan = TxSubmissionPlan::new(
            request,
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );
        let hash = executor
            .send_transaction(&plan)
            .expect("send transaction hash");
        assert_eq!(
            hash,
            TxHash::from_hex(format!("0x{}", "55".repeat(32))).unwrap()
        );
        assert_eq!(executor.next_id(), 7);
        let transport = executor.into_transport();
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
                "eth_sendTransaction"
            ]
        );
    }

    #[test]
    fn fetches_confirmation_snapshot() {
        let hash = TxHash::new(B256::repeat_byte(0x66));
        let receipt = TxReceipt::new(hash)
            .with_block_number(123)
            .with_status(true);
        let transport = MockTransport::with_single_responses([
            json!({"jsonrpc":"2.0","id":40,"result": receipt}),
            json!({"jsonrpc":"2.0","id":41,"result":"0x7c"}),
        ]);
        let mut executor = JsonRpcExecutor::with_next_id(transport, 40);
        let plan =
            TxConfirmationPlan::new(hash, TxConfirmationPolicy::new(2).with_timeout_blocks(20))
                .with_submitted_at_block(100);

        let snapshot = executor
            .confirmation_snapshot(&plan)
            .expect("confirmation snapshot");

        assert_eq!(snapshot.receipt, Some(receipt));
        assert_eq!(snapshot.current_block, Some(124));
        assert_eq!(
            snapshot.status,
            TxConfirmationStatus::Confirmed { confirmations: 2 }
        );
    }

    #[test]
    fn preflights_transaction_plans_with_sequential_nonces_and_per_tx_gas() {
        let from = Address::repeat_byte(0x33);
        let first = UnsignedCall {
            to: Address::repeat_byte(0x44),
            data: vec![0x11, 0x11, 0x11, 0x11],
        };
        let second = UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x22, 0x22, 0x22, 0x22],
        };
        let transport = MockTransport::with_single_responses([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x7530"}),
        ]);
        let mut executor = JsonRpcExecutor::new(transport);

        let plans = executor
            .preflight_transaction_plans(
                &[first, second],
                from,
                RpcBlockTag::Pending,
                TxFeePolicy::Eip1559FromGasPrice {
                    max_fee_multiplier: 2,
                    min_priority_fee_per_gas: None,
                },
                TxConfirmationPolicy::new(2).with_timeout_blocks(20),
            )
            .expect("batch preflight plans");

        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].request.from, Some(from));
        assert_eq!(plans[0].request.nonce.as_deref(), Some("0x7"));
        assert_eq!(plans[1].request.nonce.as_deref(), Some("0x8"));
        assert_eq!(plans[0].request.gas.as_deref(), Some("0x5208"));
        assert_eq!(plans[1].request.gas.as_deref(), Some("0x7530"));
        assert_eq!(
            plans[0].request.max_fee_per_gas.as_deref(),
            Some("0xee6b2800")
        );
        assert_eq!(
            plans[1].request.max_priority_fee_per_gas.as_deref(),
            Some("0x3b9aca00")
        );
        assert_eq!(
            plans[0].confirmation_policy,
            TxConfirmationPolicy::new(2).with_timeout_blocks(20)
        );
        let transport = executor.into_transport();
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
    fn workflow_preflights_signs_submits_and_confirms_raw_transaction() {
        let from = Address::repeat_byte(0x33);
        let tx = UnsignedCall {
            to: Address::repeat_byte(0x44),
            data: vec![0x12, 0x34, 0x56, 0x78],
        };
        let hash = TxHash::new(B256::repeat_byte(0x77));
        let receipt = TxReceipt::new(hash)
            .with_block_number(123)
            .with_status(true);
        let transport = MockTransport::with_single_responses([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x3d090"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":6,"result": hash.to_hex()}),
            json!({"jsonrpc":"2.0","id":7,"result": receipt}),
            json!({"jsonrpc":"2.0","id":8,"result":"0x7c"}),
        ]);
        let mut workflow = TxWorkflowExecutor::with_transport(transport, MockRawSigner::default());

        let submission = workflow
            .preflight_sign_and_submit(
                &tx,
                from,
                RpcBlockTag::Pending,
                TxConfirmationPolicy::new(2).with_timeout_blocks(20),
            )
            .expect("workflow submits raw transaction");
        let snapshot = workflow
            .confirmation_snapshot(&submission)
            .expect("workflow confirms transaction");

        assert_eq!(submission.transaction_hash, hash);
        assert_eq!(submission.signed_transaction.to_hex(), "0x02abcd");
        assert_eq!(submission.confirmation_plan.transaction_hash, hash);
        assert_eq!(
            snapshot.status,
            TxConfirmationStatus::Confirmed { confirmations: 2 }
        );

        let (rpc, signer) = workflow.into_parts();
        assert_eq!(signer.seen.len(), 1);
        assert_eq!(signer.seen[0].transaction.from, Some(from));
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
                "eth_estimateGas",
                "eth_gasPrice",
                "eth_maxPriorityFeePerGas",
                "eth_sendRawTransaction",
                "eth_getTransactionReceipt",
                "eth_blockNumber"
            ]
        );
    }

    #[test]
    fn workflow_preflights_signs_and_submits_transaction_batch() {
        let from = Address::repeat_byte(0x33);
        let first = UnsignedCall {
            to: Address::repeat_byte(0x44),
            data: vec![0x11, 0x11, 0x11, 0x11],
        };
        let second = UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x22, 0x22, 0x22, 0x22],
        };
        let first_hash = TxHash::new(B256::repeat_byte(0x88));
        let second_hash = TxHash::new(B256::repeat_byte(0x99));
        let transport = MockTransport::with_single_responses([
            json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
            json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
            json!({"jsonrpc":"2.0","id":3,"result":"0x77359400"}),
            json!({"jsonrpc":"2.0","id":4,"result":"0x3b9aca00"}),
            json!({"jsonrpc":"2.0","id":5,"result":"0x5208"}),
            json!({"jsonrpc":"2.0","id":6,"result":"0x7530"}),
            json!({"jsonrpc":"2.0","id":7,"result": first_hash.to_hex()}),
            json!({"jsonrpc":"2.0","id":8,"result": second_hash.to_hex()}),
        ]);
        let mut workflow = TxWorkflowExecutor::with_transport(transport, MockRawSigner::default());

        let batch = workflow
            .preflight_sign_and_submit_batch_with_fee_policy(
                &[first, second],
                from,
                RpcBlockTag::Pending,
                TxFeePolicy::Eip1559FromGasPrice {
                    max_fee_multiplier: 2,
                    min_priority_fee_per_gas: None,
                },
                TxConfirmationPolicy::new(2).with_timeout_blocks(20),
            )
            .expect("batch workflow submits");

        assert_eq!(batch.transaction_hashes(), vec![first_hash, second_hash]);
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

    #[test]
    fn confirmation_batch_snapshot_classifies_ordered_batch_status() {
        let confirmed_hash = TxHash::new(B256::repeat_byte(0x88));
        let pending_hash = TxHash::new(B256::repeat_byte(0x99));
        let reverted_hash = TxHash::new(B256::repeat_byte(0xaa));
        let confirmed = TxConfirmationSnapshot {
            receipt: Some(
                TxReceipt::new(confirmed_hash)
                    .with_block_number(10)
                    .with_status(true),
            ),
            current_block: Some(12),
            status: TxConfirmationStatus::Confirmed { confirmations: 3 },
        };
        let pending = TxConfirmationSnapshot {
            receipt: Some(TxReceipt::new(pending_hash).with_block_number(12)),
            current_block: Some(12),
            status: TxConfirmationStatus::Pending { confirmations: 1 },
        };
        let reverted = TxConfirmationSnapshot {
            receipt: Some(
                TxReceipt::new(reverted_hash)
                    .with_block_number(11)
                    .with_status(false),
            ),
            current_block: Some(12),
            status: TxConfirmationStatus::Reverted { confirmations: 2 },
        };

        let pending_batch = TxConfirmationBatchSnapshot::new(vec![confirmed.clone(), pending]);
        assert_eq!(
            pending_batch.status,
            TxConfirmationBatchStatus::Pending {
                confirmed: 1,
                pending: 1,
                total: 2,
            }
        );
        let pending_report = pending_batch.report(&[
            TxConfirmationPlan::new(confirmed_hash, TxConfirmationPolicy::new(3)),
            TxConfirmationPlan::new(pending_hash, TxConfirmationPolicy::new(3)),
        ]);
        assert_eq!(pending_report.len, 2);
        assert!(!pending_report.is_empty);
        assert!(pending_report.should_continue_polling);
        assert!(!pending_report.is_terminal);
        assert_eq!(pending_report.status, pending_batch.status);
        assert_eq!(
            pending_report.reports[0].transaction_hash,
            Some(confirmed_hash)
        );
        assert_eq!(pending_report.reports[0].receipt_block_number, Some(10));
        assert_eq!(pending_report.reports[0].receipt_status, Some(true));
        assert_eq!(pending_report.reports[0].confirmations, Some(3));
        let receipt_summary = pending_report.reports[0]
            .receipt_summary
            .expect("receipt summary");
        assert_eq!(receipt_summary.transaction_hash, confirmed_hash);
        assert!(receipt_summary.mined);
        assert!(receipt_summary.success);
        assert_eq!(receipt_summary.block_number, Some(10));
        assert_eq!(
            pending_report.reports[1].transaction_hash,
            Some(pending_hash)
        );
        assert_eq!(
            pending_report.reports[1].status,
            TxConfirmationStatus::Pending { confirmations: 1 }
        );
        let json = serde_json::to_string(&pending_report).expect("confirmation report serializes");
        assert!(json.contains("\"receipt_summary\""));
        assert!(json.contains("\"should_continue_polling\":true"));
        assert!(json.contains("\"is_terminal\":false"));
        let restored: TxConfirmationBatchReport =
            serde_json::from_str(&json).expect("confirmation report deserializes");
        assert_eq!(restored, pending_report);
        let mut legacy_batch_json =
            serde_json::to_value(&pending_report).expect("batch report value");
        let legacy_batch_object = legacy_batch_json
            .as_object_mut()
            .expect("batch report object");
        legacy_batch_object.remove("should_continue_polling");
        legacy_batch_object.remove("is_terminal");
        let legacy_batch_report: TxConfirmationBatchReport =
            serde_json::from_value(legacy_batch_json).expect("legacy batch report deserializes");
        assert!(!legacy_batch_report.should_continue_polling);
        assert!(!legacy_batch_report.is_terminal);
        let legacy_json = serde_json::json!({
            "transaction_hash": null,
            "current_block": 12,
            "receipt_block_number": null,
            "receipt_status": null,
            "confirmations": null,
            "gas_used": null,
            "effective_gas_price": null,
            "status": {
                "Pending": {
                    "confirmations": 0
                }
            }
        });
        let legacy_report: TxConfirmationSnapshotReport =
            serde_json::from_value(legacy_json).expect("legacy confirmation report deserializes");
        assert_eq!(legacy_report.receipt_summary, None);
        assert!(pending_batch.should_continue_polling());
        assert!(!pending_batch.is_terminal());

        let confirmed_batch = TxConfirmationBatchSnapshot::new(vec![confirmed.clone()]);
        assert_eq!(
            confirmed_batch.status,
            TxConfirmationBatchStatus::Confirmed { total: 1 }
        );
        assert!(confirmed_batch.is_terminal());
        let confirmed_report = confirmed_batch.report(&[TxConfirmationPlan::new(
            confirmed_hash,
            TxConfirmationPolicy::new(3),
        )]);
        assert!(!confirmed_report.should_continue_polling);
        assert!(confirmed_report.is_terminal);

        let reverted_batch = TxConfirmationBatchSnapshot::new(vec![confirmed, reverted]);
        assert_eq!(
            reverted_batch.status,
            TxConfirmationBatchStatus::Reverted {
                index: 1,
                confirmations: 2,
            }
        );
        assert!(reverted_batch.is_terminal());
        assert!(reverted_batch.report(&[]).is_terminal);

        let timed_out_batch = TxConfirmationBatchSnapshot::new(vec![TxConfirmationSnapshot {
            receipt: None,
            current_block: Some(130),
            status: TxConfirmationStatus::TimedOut,
        }]);
        assert_eq!(
            timed_out_batch.status,
            TxConfirmationBatchStatus::TimedOut { index: 0 }
        );
        assert!(timed_out_batch.report(&[]).is_terminal);
    }

    #[test]
    fn workflow_fetches_confirmation_batch_snapshot() {
        let first_hash = TxHash::new(B256::repeat_byte(0x88));
        let second_hash = TxHash::new(B256::repeat_byte(0x99));
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
        let first_receipt = TxReceipt::new(first_hash)
            .with_block_number(10)
            .with_status(true);
        let second_receipt = TxReceipt::new(second_hash).with_block_number(12);
        let transport = MockTransport::with_single_responses([
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
        let mut workflow = TxWorkflowExecutor::with_transport(transport, MockRawSigner::default());
        let batch = workflow
            .submit_raw_plans(&[first_plan, second_plan])
            .expect("batch submits");

        let confirmations = workflow
            .confirmation_batch_snapshot(&batch)
            .expect("batch confirmations");
        let report = workflow
            .confirmation_batch_report(&batch)
            .expect("batch confirmation report");

        assert_eq!(confirmations.snapshots.len(), 2);
        assert_eq!(
            confirmations.status,
            TxConfirmationBatchStatus::Pending {
                confirmed: 1,
                pending: 1,
                total: 2,
            }
        );
        assert_eq!(report.len, 2);
        assert_eq!(report.status, confirmations.status);
        assert_eq!(report.reports[0].transaction_hash, Some(first_hash));
        assert_eq!(
            report.reports[0].status,
            TxConfirmationStatus::Confirmed { confirmations: 3 }
        );
        assert_eq!(report.reports[1].transaction_hash, Some(second_hash));
    }

    #[test]
    fn workflow_submits_prepared_raw_plans_in_order() {
        let first_hash = TxHash::new(B256::repeat_byte(0x88));
        let second_hash = TxHash::new(B256::repeat_byte(0x99));
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
        let transport = MockTransport::with_single_responses([
            json!({"jsonrpc":"2.0","id":1,"result": first_hash.to_hex()}),
            json!({"jsonrpc":"2.0","id":2,"result": second_hash.to_hex()}),
        ]);
        let mut workflow = TxWorkflowExecutor::with_transport(transport, MockRawSigner::default());

        let batch = workflow
            .submit_raw_plans(&[first_plan.clone(), second_plan.clone()])
            .expect("batch submits");

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
        assert_eq!(batch.transaction_hashes(), vec![first_hash, second_hash]);
        assert_eq!(batch.confirmation_plans()[0].transaction_hash, first_hash);
        assert_eq!(batch.submissions[0].plan, first_plan);
        assert_eq!(batch.submissions[1].plan, second_plan);
        let report = batch.report();
        assert_eq!(report.len, 2);
        assert!(!report.is_empty);
        assert!(report.has_submissions);
        assert_eq!(report.transaction_hashes, vec![first_hash, second_hash]);
        assert_eq!(report.plan_summary.len, 2);
        assert_eq!(report.plan_summary.total_gas, Some(51_000));
        assert_eq!(report.submissions[0].transaction_hash, first_hash);
        assert_eq!(
            report.submissions[0].plan_summary.to,
            Address::repeat_byte(0x44)
        );
        assert_eq!(
            report.submissions[0].confirmation_plan.transaction_hash,
            first_hash
        );
        assert_eq!(
            report.submissions[0].confirmation_plan_summary,
            report.submissions[0].confirmation_plan.summary()
        );
        assert_eq!(
            report.submissions[0]
                .confirmation_plan_summary
                .required_confirmations,
            2
        );
        assert_eq!(
            report.submissions[0]
                .confirmation_plan_summary
                .request_count,
            2
        );
        let json = serde_json::to_string(&report).expect("workflow report serializes");
        assert!(json.contains("\"confirmation_plan_summary\""));
        assert!(json.contains("\"has_submissions\":true"));
        let restored: TxWorkflowBatchSubmissionReport =
            serde_json::from_str(&json).expect("workflow report deserializes");
        assert_eq!(restored, report);
        let mut legacy_report_json = serde_json::to_value(&report).expect("workflow report value");
        let legacy_report_object = legacy_report_json
            .as_object_mut()
            .expect("workflow report object");
        legacy_report_object.remove("has_submissions");
        let legacy_report: TxWorkflowBatchSubmissionReport =
            serde_json::from_value(legacy_report_json).expect("legacy workflow report");
        assert!(!legacy_report.has_submissions);
        let partial_batch = TxWorkflowBatchSubmission::new(vec![batch.submissions[0].clone()]);
        let resume = partial_batch.resume_plan(&[first_plan.clone(), second_plan.clone()]);
        assert_eq!(resume.original_len, 2);
        assert_eq!(resume.submitted_len, 1);
        assert_eq!(resume.next_plan_index, 1);
        assert_eq!(resume.remaining_len, 1);
        assert!(resume.can_continue());
        assert!(!resume.is_complete);
        assert!(!resume.submitted_exceeds_original);
        assert_eq!(resume.submitted_transaction_hashes, vec![first_hash]);
        assert_eq!(resume.remaining_plans, vec![second_plan.clone()]);
        assert_eq!(resume.remaining_plan_summary.len, 1);
        assert_eq!(resume.remaining_plan_summary.total_gas, Some(30_000));
        let resume_summary = resume.summary();
        assert_eq!(resume_summary.original_len, 2);
        assert_eq!(resume_summary.submitted_len, 1);
        assert!(resume_summary.has_submitted);
        assert_eq!(resume_summary.next_plan_index, 1);
        assert_eq!(resume_summary.remaining_len, 1);
        assert!(resume_summary.has_remaining);
        assert!(resume_summary.can_continue);
        assert!(!resume_summary.is_complete);
        assert_eq!(
            resume_summary.submitted_transaction_hashes,
            vec![first_hash]
        );
        assert_eq!(resume_summary.submitted_plan_summary.len, 1);
        assert_eq!(resume_summary.remaining_plan_summary.len, 1);
        let resume_summary_json =
            serde_json::to_string(&resume_summary).expect("resume summary serializes");
        let restored_resume_summary: TxWorkflowBatchResumePlanSummary =
            serde_json::from_str(&resume_summary_json).expect("resume summary deserializes");
        assert_eq!(restored_resume_summary, resume_summary);
        let mut legacy_resume_summary_json =
            serde_json::to_value(&resume_summary).expect("resume summary value");
        let legacy_resume_summary_object = legacy_resume_summary_json
            .as_object_mut()
            .expect("resume summary object");
        legacy_resume_summary_object.remove("has_submitted");
        legacy_resume_summary_object.remove("has_remaining");
        legacy_resume_summary_object.remove("can_continue");
        let legacy_resume_summary: TxWorkflowBatchResumePlanSummary =
            serde_json::from_value(legacy_resume_summary_json).expect("legacy resume summary");
        assert!(!legacy_resume_summary.has_submitted);
        assert!(!legacy_resume_summary.has_remaining);
        assert!(!legacy_resume_summary.can_continue);
        let resume_json = serde_json::to_string(&resume).expect("resume serializes");
        let restored_resume: TxWorkflowBatchResumePlan =
            serde_json::from_str(&resume_json).expect("resume deserializes");
        assert_eq!(restored_resume, resume);

        let complete_resume = batch.resume_plan(&[first_plan.clone(), second_plan.clone()]);
        assert!(complete_resume.is_complete);
        assert_eq!(complete_resume.next_plan_index, 2);
        assert_eq!(complete_resume.remaining_len, 0);
        assert!(!complete_resume.can_continue());
        let complete_resume_summary = complete_resume.summary();
        assert!(complete_resume_summary.has_submitted);
        assert!(!complete_resume_summary.has_remaining);
        assert!(!complete_resume_summary.can_continue);
        assert!(complete_resume.remaining_plans.is_empty());
        assert!(complete_resume.remaining_plan_summary.is_empty);

        let over_submitted_resume = batch.resume_plan(&[first_plan]);
        assert_eq!(over_submitted_resume.original_len, 1);
        assert_eq!(over_submitted_resume.submitted_len, 2);
        assert!(over_submitted_resume.submitted_exceeds_original);
        assert!(!over_submitted_resume.can_continue());
        assert!(!over_submitted_resume.summary().can_continue);
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
            vec!["eth_sendRawTransaction", "eth_sendRawTransaction"]
        );
    }
}
