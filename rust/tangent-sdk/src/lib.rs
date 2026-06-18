//! # tangent-sdk
//!
//! Rust SDK for [Tangent](https://github.com/JagritGumber/tangent).
//!
//! Designed to be the low-level dependency a downstream agent (Selbo,
//! CapitalArc, future Arc-native agents) can use to integrate against the
//! on-chain Solidity primitives without copying Tangent ABI details. The
//! Solidity side lives at `../../src/`; this crate wraps the current raw
//! integration surface in typed Rust APIs.
//!
//! ## Current status (v0.1 of the parent repo)
//!
//! Pre-1.0. This crate currently ships the canonical EIP-712 [`Order`] type
//! mirroring `OrderTypes.sol`, signed-order calldata helpers, deployment
//! manifest parsing, manifest-bound context helpers, primitive contract
//! calldata helpers, typed workflow plans for
//! account/collateral/market/order/settlement/liquidation flows, order
//! placement planning, keeper polling execution helpers, source-preserving
//! event records, and minimal ABI return decoders. It also provides
//! transport-neutral JSON-RPC envelopes, executor helpers, signer traits, and
//! transaction workflow orchestration for callers that bring their own
//! provider, wallet, KMS, relayer, or test harness.
//! It does not yet open RPC connections, run a daemon loop, or sign with Circle
//! Dev Wallets. Concrete transports, signer backends, and process runtimes land
//! outside this low-level crate boundary.
//!
//! See [`ARCHITECTURE.md`](https://github.com/JagritGumber/tangent/blob/main/ARCHITECTURE.md)
//! for the full system design and roadmap.

#![doc(html_root_url = "https://docs.rs/tangent-sdk")]

pub mod abi;
pub mod account;
pub mod client;
pub mod collateral;
pub mod context;
pub mod contracts;
pub mod domain;
mod eip712;
pub mod events;
pub mod keeper;
pub mod lifecycle;
pub mod liquidation;
pub mod manifest;
pub mod market;
pub mod order;
pub mod orderbook;
pub mod placement;
pub mod projection;
pub mod rpc;
pub mod settlement;
pub mod signing;
pub mod tx;

pub use abi::AbiDecodeError;
pub use account::{
    AccountBindingStatus, AccountOnboardingAction, AccountOnboardingNextStep,
    AccountOnboardingPlan, AccountOnboardingSummary, AccountStatus, AccountStatusPlan,
};
pub use client::{
    RpcEndpointConfig, RpcEndpointConfigReport, RpcHeader, SignerBackendConfigReport,
    TangentClient, TangentClientConfig, TangentClientConfigError, TangentClientConfigReport,
    TangentClientEventLogError, TangentClientEventProjectionError, TangentClientPlan,
    TangentClientPolicies, TangentClientPreflightSummaryError, TangentClientReadError,
    TangentClientStartupReadiness, TangentClientStartupReport, TangentClientSupportReport,
    TangentClientWorkflow, TangentKeeperLiquidationCandidate, TangentKeeperLiquidationScanReport,
    TangentKeeperLiquidationScanResult, TangentKeeperPollingCheckpoint,
    TangentKeeperPollingExecution, TangentKeeperPollingExecutionError,
    TangentKeeperPollingExecutionReport, TangentKeeperPollingExecutionSummary,
    TangentKeeperPollingPreview, TangentKeeperPollingState, TangentKeeperPollingStateExecution,
    TangentKeeperWorkflowError, TangentLiquidationDryRun, TangentLiquidationDryRunBatch,
    TangentLiquidationDryRunBatchSummary, TangentLiquidationDryRunError,
    TangentLiquidationDryRunSummary, TangentLiquidationSubmission,
    TangentOrderLifecycleSubmitError, TangentOrderPlacementPreparation,
    TangentOrderPlacementPrepareError, TangentOrderPlacementSubmission,
    TangentOrderPlacementSubmitError, TangentReadPlan, TangentReadPlanExecutionError,
};
pub use collateral::{
    CollateralDepositNextStep, CollateralDepositPlan, CollateralDepositReadiness,
    CollateralDepositSummary, CollateralStatus, CollateralStatusPlan, CollateralWithdrawNextStep,
    CollateralWithdrawPlan, CollateralWithdrawSummary, WithdrawalReadiness,
};
pub use context::{TangentContext, TangentContextError, TangentContextSummary};
pub use contracts::{
    AccountManagerCalls, ERC20Calls, LiquidationKeeperCalls, MarketRegistryCalls, SettlementCalls,
    USDCVaultCalls,
};
pub use domain::DomainSeparatorInput;
pub use events::{
    event_topic, AccountRegisteredEvent, DecodedTangentLogRecord, DecodedTangentLogRecords,
    DecodedTangentLogRecordsSummary, DecodedTangentLogs, DecodedTangentLogsSummary, DepositedEvent,
    EventDecodeError, EventFilter, EventFilterRequest, EventFilterSet, EventLogQuery,
    EventLogQueryBatchSummary, EventLogQuerySummary, EventLogRpcQuery,
    EventLogRpcQueryBatchSummary, EventLogRpcQuerySummary, EventQueryError, LiquidatedEvent,
    MarginAmountEvent, MarketParamsUpdatedEvent, MarketPausedEvent, MarketRegisteredEvent,
    MatchedEvent, OrderCancelledEvent, OrderSubmittedEvent, PnlAppliedEvent, RawLog, RawLogCursor,
    RawLogError, RawLogMetadata, SettledEvent, TangentEvent, TangentEventKind,
    TangentEventKindCount, WithdrawnEvent,
};
pub use keeper::{
    KeeperCapability, KeeperLiquidationCandidatePlan, KeeperMaintenanceTransactionSummary,
    KeeperPollingOutcome, KeeperPollingPlan, KeeperPollingPlanSummary, KeeperPollingPolicy,
    KeeperPollingSnapshot, KeeperRuntimePlan,
};
pub use lifecycle::{
    OrderBookMaintenancePlan, OrderLifecyclePlan, OrderLifecyclePlanSummary, OrderLifecycleState,
    OrderLifecycleStatus, OrderLifecycleSummary,
};
pub use liquidation::{
    LiquidationDecodeError, LiquidationNextStep, LiquidationReadPlan, LiquidationReadPlanSummary,
    LiquidationReadiness, LiquidationStatus, LiquidationStatusSummary,
};
pub use manifest::{
    ContractAddresses, DeploymentManifest, FullPerpStackAddresses, ManifestError, NetworkConstants,
    PerpStackAvailability, PerpStackAvailabilitySummary,
};
pub use market::{
    MarketDetails, MarketReadPlan, MarketReadPlanSummary, MarketReadSummary, MarketStatusSummary,
};
pub use order::{
    Order, OrderBuilder, OrderConstraints, OrderError, OrderParams, Side, BASE_SCALE, PRICE_SCALE,
};
pub use orderbook::OrderBookCalls;
pub use placement::{
    OrderPlacement, OrderPlacementPlan, OrderPlacementPlanSummary, OrderPlacementSignError,
    OrderPlacementSummary,
};
pub use projection::{
    AccountEventProjection, AccountMarketProjectionKey, MarketEventProjection,
    OrderEventProjection, OrderEventStatus, TangentEventProjection, TangentEventProjectionError,
    TangentEventProjectionSummary,
};
pub use rpc::{
    JsonRpcBackoffPolicy, JsonRpcExecutor, JsonRpcExecutorError, JsonRpcRetryPolicy,
    JsonRpcRetryStats, JsonRpcTransport, RetryingJsonRpcTransport, TxConfirmationBatchReport,
    TxConfirmationBatchSnapshot, TxConfirmationBatchStatus, TxConfirmationSnapshot,
    TxConfirmationSnapshotReport, TxWorkflowBatchResumePlan, TxWorkflowBatchResumePlanSummary,
    TxWorkflowBatchSubmission, TxWorkflowBatchSubmissionReport, TxWorkflowError,
    TxWorkflowExecutor, TxWorkflowSubmission, TxWorkflowSubmissionReport,
};
pub use settlement::{
    MarginStatus, PositionStatus, SettlementReadPlan, SettlementStatus,
    SettlementWithdrawalNextStep, SettlementWithdrawalReadiness, SettlementWithdrawalSummary,
};
pub use signing::{
    ExternalSignerAdapter, ExternalSignerAdapterError, ExternalSigningClient,
    ExternalSigningPayload, ExternalSigningPayloadKind, ExternalSigningRequest,
    ExternalSigningRequestReport, ExternalSigningResponse, ExternalSigningResponseReport,
    OrderSignature, OrderSigner, OrderSigningRequest, PreparedOrder, RawTransactionSigner,
    RawTransactionSigningRequest, SignatureError, SignedOrder, SignerBackendConfig,
    SignerBackendConfigError, SignerBackendKind, SignerBackendMetadata, SignerBackendReport,
};
pub use tx::{
    CallDataError, CallReturn, CallReturnBatch, CallReturnError, JsonRpcBatchError,
    JsonRpcErrorObject, JsonRpcRequest, JsonRpcRequestError, JsonRpcResponse, JsonRpcResponseError,
    JsonRpcResultDecodeError, RpcBlockTag, RpcQuantityError, SignedRawTransaction,
    SignedRawTransactionError, TxBatchRequestMetadata, TxConfirmationPlan,
    TxConfirmationPlanSummary, TxConfirmationPolicy, TxConfirmationStatus, TxFeePolicy, TxHash,
    TxHashError, TxPreflight, TxPreflightError, TxPreflightSummary, TxReceipt, TxReceiptError,
    TxReceiptSummary, TxRequestMetadata, TxRequestMetadataError, TxSubmissionPlan,
    TxSubmissionPlanBatchSummary, TxSubmissionPlanSummary, UnsignedCall, UnsignedCallBatchSummary,
    UnsignedCallContractSummary, UnsignedCallQuery, UnsignedCallRequest, UnsignedCallSummary,
    UnsignedTx, UnsignedTxRequest,
};
