//! Transport-agnostic call inputs and transaction outputs produced by SDK workflow helpers.

use std::collections::BTreeMap;

use alloy_primitives::{Address, B256};
use serde::{Deserialize, Serialize};

use crate::events::{
    DecodedTangentLogRecords, DecodedTangentLogs, EventDecodeError, EventFilterSet,
    EventLogRpcQuery, RawLog, RawLogCursor,
};

/// Unsigned contract call input produced by the SDK.
///
/// The SDK deliberately does not choose a transport, signer, nonce, gas limit,
/// fee policy, or `eth_call` execution policy. Callers can pass these fields
/// into Alloy, Circle Dev Wallets, a relayer, or their own transaction/call
/// builder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedCall {
    pub to: Address,
    #[serde(with = "call_data")]
    pub data: Vec<u8>,
}

/// RPC-friendly view of an unsigned contract call.
///
/// `UnsignedCall` keeps calldata as bytes for local inspection. Most JSON-RPC,
/// relayer, and wallet APIs expect calldata as `0x` hex, so this type provides
/// a serializable boundary shape without choosing a transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedCallRequest {
    pub to: Address,
    pub data: String,
}

/// Provider block selector for read-only calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpcBlockTag {
    Latest,
    Earliest,
    Pending,
    Safe,
    Finalized,
    Number(u64),
}

/// RPC-friendly view of an unsigned contract call plus block selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedCallQuery {
    pub call: UnsignedCallRequest,
    pub block: String,
}

/// Compact review shape for one read-only contract call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedCallSummary {
    pub to: Address,
    pub selector: Option<String>,
    pub calldata_bytes: usize,
}

/// Per-contract call count inside a read-only call batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedCallContractSummary {
    pub to: Address,
    pub calls: usize,
}

/// Compact review shape for a fixed-order read-only contract call batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedCallBatchSummary {
    pub len: usize,
    pub is_empty: bool,
    pub total_calldata_bytes: usize,
    pub unique_contracts: usize,
    pub contracts: Vec<UnsignedCallContractSummary>,
    pub calls: Vec<UnsignedCallSummary>,
}

/// RPC-friendly view of an unsigned transaction.
///
/// This keeps signer-owned fields such as `from`, `nonce`, gas, and fee policy
/// out of the SDK while still exposing the write-call boundary shape expected
/// by wallet, relayer, and JSON-RPC transaction builders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedTxRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Address>,
    pub to: Address,
    pub data: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<String>,
    #[serde(rename = "gasPrice", skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<String>,
    #[serde(rename = "maxFeePerGas", skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<String>,
    #[serde(
        rename = "maxPriorityFeePerGas",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_priority_fee_per_gas: Option<String>,
    #[serde(rename = "chainId", skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
}

/// Transport-neutral JSON-RPC 2.0 request envelope.
///
/// The SDK still does not open a socket, choose retries, sign transactions, or
/// broadcast anything. This type only gives callers a canonical serialized
/// shape for common Ethereum JSON-RPC methods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Vec<serde_json::Value>,
}

/// JSON-RPC 2.0 response envelope returned by a provider or relayer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcErrorObject>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Caller-supplied transaction request metadata.
///
/// This type does not estimate gas or choose a fee policy. It only carries
/// externally selected transaction-builder fields into an RPC/wallet-friendly
/// request view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TxRequestMetadata {
    pub from: Option<Address>,
    pub nonce: Option<u64>,
    pub gas: Option<u64>,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
    pub chain_id: Option<u64>,
}

/// Caller-supplied metadata for a fixed-order transaction batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TxBatchRequestMetadata {
    pub from: Option<Address>,
    pub start_nonce: Option<u64>,
    pub gas: Option<u64>,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
    pub chain_id: Option<u64>,
}

/// Decoded transaction preflight values returned by caller-managed RPC reads.
///
/// This does not fetch chain id, nonce, gas, or fees. It only groups decoded
/// provider responses so callers can apply them to SDK unsigned transactions in
/// one stable shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TxPreflight {
    pub chain_id: Option<u64>,
    pub nonce: Option<u64>,
    pub gas: Option<u64>,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
}

/// Compact review shape for decoded transaction preflight values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxPreflightSummary {
    pub has_chain_id: bool,
    pub has_nonce: bool,
    pub has_gas: bool,
    pub has_gas_price: bool,
    pub has_eip1559_fees: bool,
    pub has_complete_eip1559_fees: bool,
    pub has_any_fee: bool,
    pub ready_for_submission_request: bool,
    pub chain_id: Option<u64>,
    pub nonce: Option<u64>,
    pub gas: Option<u64>,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
}

/// Local fee policy applied to decoded transaction preflight values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TxFeePolicy {
    /// Leave decoded gas/fee fields unchanged.
    #[default]
    Preserve,
    /// Use legacy `gasPrice` only, clearing EIP-1559 fee fields.
    LegacyGasPrice,
    /// Derive missing EIP-1559 max fee from `gasPrice * max_fee_multiplier`
    /// and optionally floor the priority fee.
    Eip1559FromGasPrice {
        max_fee_multiplier: u32,
        min_priority_fee_per_gas: Option<u128>,
    },
}

/// Transport-returned contract call data.
///
/// Most RPC clients and wallet APIs return `eth_call` data as `0x` hex while
/// the SDK's ABI decoders intentionally operate on bytes. This type is the
/// small boundary adapter between those two shapes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallReturn {
    #[serde(with = "call_return_data")]
    pub data: Vec<u8>,
}

/// Ordered batch of transport-returned contract call data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallReturnBatch {
    pub returns: Vec<CallReturn>,
}

/// Transaction hash returned by a signer, relayer, or JSON-RPC transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TxHash(#[serde(with = "tx_hash_hex")] pub B256);

/// Externally signed raw Ethereum transaction bytes.
///
/// The SDK does not sign or RLP/EIP-2718 encode transactions. This type only
/// validates and carries the raw bytes returned by a signer so callers can build
/// `eth_sendRawTransaction` requests without passing unchecked strings around.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedRawTransaction(#[serde(with = "signed_raw_transaction_data")] pub Vec<u8>);

/// Transport-neutral transaction receipt subset used by SDK consumers.
///
/// The SDK does not fetch receipts itself. Callers map their provider receipt
/// into this shape, then use the decoded logs and status helpers consistently
/// across wallet, relayer, and RPC integrations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxReceipt {
    pub transaction_hash: TxHash,
    pub block_number: Option<u64>,
    pub status: Option<bool>,
    pub gas_used: Option<u64>,
    pub effective_gas_price: Option<u128>,
    #[serde(default)]
    pub logs: Vec<RawLog>,
}

/// Compact review shape for a provider-returned transaction receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxReceiptSummary {
    pub transaction_hash: TxHash,
    pub mined: bool,
    pub success: bool,
    pub reverted: bool,
    pub block_number: Option<u64>,
    pub status: Option<bool>,
    pub gas_used: Option<u64>,
    pub effective_gas_price: Option<u128>,
    pub execution_fee_paid: Option<u128>,
    pub log_count: usize,
    pub last_cursor: Option<RawLogCursor>,
}

/// Transport-neutral transaction confirmation policy.
///
/// The SDK does not poll for receipts or blocks. This policy classifies data
/// that a caller already fetched from its provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxConfirmationPolicy {
    pub required_confirmations: u64,
    pub timeout_blocks: Option<u64>,
}

/// Request/classification plan for one submitted transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxConfirmationPlan {
    pub transaction_hash: TxHash,
    pub policy: TxConfirmationPolicy,
    pub submitted_at_block: Option<u64>,
}

/// Compact review shape for one transaction confirmation plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxConfirmationPlanSummary {
    pub transaction_hash: TxHash,
    pub required_confirmations: u64,
    pub timeout_blocks: Option<u64>,
    pub submitted_at_block: Option<u64>,
    pub request_count: usize,
}

/// Transport-neutral submission plan for one prepared transaction.
///
/// This does not sign, send, or poll. It packages the transaction request and
/// confirmation policy so callers can hand the request to a signer/relayer and
/// then reuse the same policy once a transaction hash is returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxSubmissionPlan {
    pub request: UnsignedTxRequest,
    pub confirmation_policy: TxConfirmationPolicy,
    pub submitted_at_block: Option<u64>,
}

/// Compact review shape for one prepared transaction submission plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxSubmissionPlanSummary {
    pub from: Option<Address>,
    pub to: Address,
    pub selector: Option<String>,
    pub calldata_bytes: Option<usize>,
    pub value: String,
    pub nonce: Option<String>,
    pub gas: Option<String>,
    pub gas_price: Option<String>,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub chain_id: Option<String>,
    pub uses_eip1559_fees: bool,
    pub uses_legacy_gas_price: bool,
    pub confirmation_policy: TxConfirmationPolicy,
    pub submitted_at_block: Option<u64>,
}

/// Compact review shape for an ordered transaction submission batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxSubmissionPlanBatchSummary {
    pub len: usize,
    pub is_empty: bool,
    pub total_calldata_bytes: Option<usize>,
    pub total_gas: Option<u128>,
    pub first_nonce: Option<String>,
    pub last_nonce: Option<String>,
    pub chain_id: Option<String>,
    pub all_same_chain_id: bool,
    #[serde(default)]
    pub ready_plans: usize,
    #[serde(default)]
    pub not_ready_plans: usize,
    #[serde(default)]
    pub has_ready_plans: bool,
    #[serde(default)]
    pub all_ready: bool,
    pub eip1559_transactions: usize,
    pub legacy_gas_price_transactions: usize,
    pub plans: Vec<TxSubmissionPlanSummary>,
}

/// Local transaction confirmation classification from a receipt snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxConfirmationStatus {
    Pending { confirmations: u64 },
    Confirmed { confirmations: u64 },
    Reverted { confirmations: u64 },
    TimedOut,
}

impl TxConfirmationStatus {
    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    #[must_use]
    pub const fn is_confirmed(self) -> bool {
        matches!(self, Self::Confirmed { .. })
    }

    #[must_use]
    pub const fn is_reverted(self) -> bool {
        matches!(self, Self::Reverted { .. })
    }

    #[must_use]
    pub const fn is_timed_out(self) -> bool {
        matches!(self, Self::TimedOut)
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        !self.is_pending()
    }

    #[must_use]
    pub const fn should_continue_polling(self) -> bool {
        self.is_pending()
    }

    #[must_use]
    pub const fn confirmations(self) -> Option<u64> {
        match self {
            Self::Pending { confirmations }
            | Self::Confirmed { confirmations }
            | Self::Reverted { confirmations } => Some(confirmations),
            Self::TimedOut => None,
        }
    }
}

/// Errors that can occur while accepting transport-returned call data.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CallReturnError {
    #[error("invalid call return hex: {0}")]
    InvalidHex(#[from] hex::FromHexError),
}

/// Errors that can occur while accepting a transport-returned transaction hash.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TxHashError {
    #[error("invalid transaction hash hex: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    #[error("invalid transaction hash byte length: expected 32, got {actual}")]
    InvalidLength { actual: usize },
}

/// Errors that can occur while accepting an externally signed raw transaction.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SignedRawTransactionError {
    #[error("invalid signed transaction hex: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    #[error("signed transaction bytes must not be empty")]
    Empty,
}

/// Errors that can occur while parsing JSON-RPC quantity strings.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RpcQuantityError {
    #[error("invalid JSON-RPC quantity hex: {0}")]
    InvalidHex(String),
    #[error("JSON-RPC quantity exceeds supported SDK width")]
    Overflow,
}

/// Errors that can occur while mapping a provider receipt into [`TxReceipt`].
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TxReceiptError {
    #[error(transparent)]
    Hash(#[from] TxHashError),
    #[error("invalid {field} quantity")]
    Quantity {
        field: &'static str,
        #[source]
        source: RpcQuantityError,
    },
    #[error("invalid receipt status quantity: expected 0x0 or 0x1, got {0}")]
    InvalidStatus(u128),
}

/// Errors that can occur while building transaction request envelopes.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TxRequestMetadataError {
    #[error("transaction batch nonce overflow at index {index}")]
    NonceOverflow { index: usize },
}

/// Errors surfaced while decoding transaction preflight RPC responses.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TxPreflightError {
    #[error("invalid {field} preflight response")]
    Decode {
        field: &'static str,
        #[source]
        source: JsonRpcResultDecodeError,
    },
}

/// Errors that can occur while building JSON-RPC request envelopes.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JsonRpcRequestError {
    #[error("JSON-RPC request id overflow at batch index {index}")]
    IdOverflow { index: usize },
}

/// Errors surfaced while extracting a JSON-RPC response result.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JsonRpcResponseError {
    #[error("JSON-RPC error {code}: {message}")]
    Rpc {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
    #[error("JSON-RPC response contained neither result nor error")]
    MissingResult,
}

/// Errors surfaced while decoding a successful JSON-RPC result payload.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum JsonRpcResultDecodeError {
    #[error(transparent)]
    Response(#[from] JsonRpcResponseError),
    #[error(transparent)]
    CallReturn(#[from] CallReturnError),
    #[error(transparent)]
    TxHash(#[from] TxHashError),
    #[error(transparent)]
    Quantity(#[from] RpcQuantityError),
}

/// Errors surfaced while matching JSON-RPC batch responses back to request ids.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum JsonRpcBatchError {
    #[error("duplicate JSON-RPC response id {id}")]
    DuplicateResponse { id: u64 },
    #[error("missing JSON-RPC response id {id}")]
    MissingResponse { id: u64 },
    #[error("failed to decode JSON-RPC response id {id}")]
    Decode {
        id: u64,
        #[source]
        source: JsonRpcResultDecodeError,
    },
}

/// Errors that can occur while accepting externally supplied calldata.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CallDataError {
    #[error("invalid calldata hex: {0}")]
    InvalidHex(#[from] hex::FromHexError),
}

impl UnsignedCall {
    /// Construct an unsigned call from provider-style `0x` hex calldata.
    pub fn from_hex_data(to: Address, data: impl AsRef<str>) -> Result<Self, CallDataError> {
        Ok(Self {
            to,
            data: decode_hex_data(data.as_ref()).map_err(CallDataError::InvalidHex)?,
        })
    }

    /// Return the 4-byte function selector when `data` contains calldata.
    #[must_use]
    pub fn selector(&self) -> Option<[u8; 4]> {
        self.data.get(..4).map(|bytes| {
            let mut selector = [0u8; 4];
            selector.copy_from_slice(bytes);
            selector
        })
    }

    /// Return the 4-byte function selector as `0x` hex.
    #[must_use]
    pub fn selector_hex(&self) -> Option<String> {
        self.selector()
            .map(|selector| format!("0x{}", hex::encode(selector)))
    }

    /// True when calldata starts with the expected 4-byte selector.
    #[must_use]
    pub fn has_selector(&self, expected: [u8; 4]) -> bool {
        self.selector() == Some(expected)
    }

    /// Return calldata after the 4-byte selector when present.
    #[must_use]
    pub fn arguments(&self) -> Option<&[u8]> {
        self.data.get(4..)
    }

    /// Return the full calldata as `0x` hex.
    #[must_use]
    pub fn data_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }

    /// Return the calldata byte length.
    #[must_use]
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    /// Return a compact serializable review shape for this call.
    #[must_use]
    pub fn summary(&self) -> UnsignedCallSummary {
        UnsignedCallSummary {
            to: self.to,
            selector: self.selector_hex(),
            calldata_bytes: self.data_len(),
        }
    }

    /// Return a compact serializable review shape for a fixed-order call batch.
    #[must_use]
    pub fn summarize_batch<'a>(
        calls: impl IntoIterator<Item = &'a UnsignedCall>,
    ) -> UnsignedCallBatchSummary {
        UnsignedCallBatchSummary::from_calls(calls)
    }

    /// Return a JSON/RPC-friendly request view with `0x` hex calldata.
    #[must_use]
    pub fn to_request(&self) -> UnsignedCallRequest {
        UnsignedCallRequest {
            to: self.to,
            data: self.data_hex(),
        }
    }

    /// Return a JSON/RPC-friendly `eth_call` query view at a specific block.
    #[must_use]
    pub fn to_query_at(&self, block: RpcBlockTag) -> UnsignedCallQuery {
        UnsignedCallQuery {
            call: self.to_request(),
            block: block.to_rpc_param(),
        }
    }

    /// Convert a batch of calls into JSON/RPC-friendly request views.
    #[must_use]
    pub fn to_requests<'a>(
        calls: impl IntoIterator<Item = &'a UnsignedCall>,
    ) -> Vec<UnsignedCallRequest> {
        calls.into_iter().map(UnsignedCall::to_request).collect()
    }

    /// Convert a batch of calls into JSON/RPC-friendly query views at one block.
    #[must_use]
    pub fn to_queries_at<'a>(
        calls: impl IntoIterator<Item = &'a UnsignedCall>,
        block: RpcBlockTag,
    ) -> Vec<UnsignedCallQuery> {
        calls
            .into_iter()
            .map(|call| call.to_query_at(block))
            .collect()
    }

    /// Return a transaction request view with zero native token value.
    #[must_use]
    pub fn to_tx_request(&self) -> UnsignedTxRequest {
        self.to_tx_request_with_value(0)
    }

    /// Return a transaction request view with an explicit native token value.
    ///
    /// The value is encoded as a JSON-RPC quantity hex string. Tangent's current
    /// contracts do not require native value, but this helper keeps the boundary
    /// usable for forks that add payable entry points.
    #[must_use]
    pub fn to_tx_request_with_value(&self, value: u128) -> UnsignedTxRequest {
        self.to_tx_request_with_value_and_metadata(value, TxRequestMetadata::default())
    }

    /// Return a transaction request view with explicit native value and metadata.
    #[must_use]
    pub fn to_tx_request_with_value_and_metadata(
        &self,
        value: u128,
        metadata: TxRequestMetadata,
    ) -> UnsignedTxRequest {
        UnsignedTxRequest {
            from: metadata.from,
            to: self.to,
            data: self.data_hex(),
            value: quantity_hex(value),
            nonce: metadata.nonce.map(|value| quantity_hex(u128::from(value))),
            gas: metadata.gas.map(|value| quantity_hex(u128::from(value))),
            gas_price: metadata.gas_price.map(quantity_hex),
            max_fee_per_gas: metadata.max_fee_per_gas.map(quantity_hex),
            max_priority_fee_per_gas: metadata.max_priority_fee_per_gas.map(quantity_hex),
            chain_id: metadata
                .chain_id
                .map(|value| quantity_hex(u128::from(value))),
        }
    }

    /// Return a zero-value transaction request view with caller-supplied metadata.
    #[must_use]
    pub fn to_tx_request_with_metadata(&self, metadata: TxRequestMetadata) -> UnsignedTxRequest {
        self.to_tx_request_with_value_and_metadata(0, metadata)
    }

    /// Convert a batch of unsigned transactions into RPC-friendly request views.
    #[must_use]
    pub fn to_tx_requests<'a>(
        transactions: impl IntoIterator<Item = &'a UnsignedTx>,
    ) -> Vec<UnsignedTxRequest> {
        transactions
            .into_iter()
            .map(UnsignedCall::to_tx_request)
            .collect()
    }

    /// Convert a fixed-order transaction batch into RPC-friendly request views.
    ///
    /// When `start_nonce` is present, each request receives `start_nonce + idx`
    /// so callers can preserve workflow order without hand-building envelopes.
    pub fn to_tx_requests_with_batch_metadata<'a>(
        transactions: impl IntoIterator<Item = &'a UnsignedTx>,
        metadata: TxBatchRequestMetadata,
    ) -> Result<Vec<UnsignedTxRequest>, TxRequestMetadataError> {
        transactions
            .into_iter()
            .enumerate()
            .map(|(index, tx)| {
                let nonce = metadata
                    .start_nonce
                    .map(|nonce| {
                        let index_u64 = u64::try_from(index)
                            .map_err(|_| TxRequestMetadataError::NonceOverflow { index })?;
                        nonce
                            .checked_add(index_u64)
                            .ok_or(TxRequestMetadataError::NonceOverflow { index })
                    })
                    .transpose()?;

                Ok(tx.to_tx_request_with_metadata(TxRequestMetadata {
                    from: metadata.from,
                    nonce,
                    gas: metadata.gas,
                    gas_price: metadata.gas_price,
                    max_fee_per_gas: metadata.max_fee_per_gas,
                    max_priority_fee_per_gas: metadata.max_priority_fee_per_gas,
                    chain_id: metadata.chain_id,
                }))
            })
            .collect()
    }
}

/// Backwards-compatible alias for transaction-oriented workflow callers.
pub type UnsignedTx = UnsignedCall;

impl TxRequestMetadata {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            from: None,
            nonce: None,
            gas: None,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: None,
        }
    }

    #[must_use]
    pub const fn with_from(mut self, from: Address) -> Self {
        self.from = Some(from);
        self
    }

    #[must_use]
    pub const fn with_nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    #[must_use]
    pub const fn with_gas(mut self, gas: u64) -> Self {
        self.gas = Some(gas);
        self
    }

    #[must_use]
    pub const fn with_gas_price(mut self, gas_price: u128) -> Self {
        self.gas_price = Some(gas_price);
        self
    }

    #[must_use]
    pub const fn with_eip1559_fees(
        mut self,
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    ) -> Self {
        self.max_fee_per_gas = Some(max_fee_per_gas);
        self.max_priority_fee_per_gas = Some(max_priority_fee_per_gas);
        self
    }

    #[must_use]
    pub const fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }
}

impl TxBatchRequestMetadata {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            from: None,
            start_nonce: None,
            gas: None,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: None,
        }
    }

    #[must_use]
    pub const fn with_from(mut self, from: Address) -> Self {
        self.from = Some(from);
        self
    }

    #[must_use]
    pub const fn with_start_nonce(mut self, start_nonce: u64) -> Self {
        self.start_nonce = Some(start_nonce);
        self
    }

    #[must_use]
    pub const fn with_gas(mut self, gas: u64) -> Self {
        self.gas = Some(gas);
        self
    }

    #[must_use]
    pub const fn with_gas_price(mut self, gas_price: u128) -> Self {
        self.gas_price = Some(gas_price);
        self
    }

    #[must_use]
    pub const fn with_eip1559_fees(
        mut self,
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    ) -> Self {
        self.max_fee_per_gas = Some(max_fee_per_gas);
        self.max_priority_fee_per_gas = Some(max_priority_fee_per_gas);
        self
    }

    #[must_use]
    pub const fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }
}

impl TxConfirmationPolicy {
    #[must_use]
    pub const fn new(required_confirmations: u64) -> Self {
        Self {
            required_confirmations,
            timeout_blocks: None,
        }
    }

    #[must_use]
    pub const fn with_timeout_blocks(mut self, timeout_blocks: u64) -> Self {
        self.timeout_blocks = Some(timeout_blocks);
        self
    }

    #[must_use]
    pub fn classify(
        &self,
        receipt: Option<&TxReceipt>,
        current_block: Option<u64>,
        submitted_at_block: Option<u64>,
    ) -> TxConfirmationStatus {
        if let Some(receipt) = receipt {
            let confirmations = receipt.confirmations(current_block).unwrap_or(0);
            if receipt.is_reverted() {
                return TxConfirmationStatus::Reverted { confirmations };
            }
            if receipt.is_success() && confirmations >= self.required_confirmations {
                return TxConfirmationStatus::Confirmed { confirmations };
            }

            return TxConfirmationStatus::Pending { confirmations };
        }

        if let (Some(timeout_blocks), Some(current_block), Some(submitted_at_block)) =
            (self.timeout_blocks, current_block, submitted_at_block)
        {
            if current_block.saturating_sub(submitted_at_block) >= timeout_blocks {
                return TxConfirmationStatus::TimedOut;
            }
        }

        TxConfirmationStatus::Pending { confirmations: 0 }
    }
}

impl TxConfirmationPlan {
    #[must_use]
    pub const fn new(transaction_hash: TxHash, policy: TxConfirmationPolicy) -> Self {
        Self {
            transaction_hash,
            policy,
            submitted_at_block: None,
        }
    }

    #[must_use]
    pub const fn with_submitted_at_block(mut self, submitted_at_block: u64) -> Self {
        self.submitted_at_block = Some(submitted_at_block);
        self
    }

    #[must_use]
    pub const fn summary(&self) -> TxConfirmationPlanSummary {
        TxConfirmationPlanSummary {
            transaction_hash: self.transaction_hash,
            required_confirmations: self.policy.required_confirmations,
            timeout_blocks: self.policy.timeout_blocks,
            submitted_at_block: self.submitted_at_block,
            request_count: 2,
        }
    }

    #[must_use]
    pub fn receipt_request(&self, id: u64) -> JsonRpcRequest {
        JsonRpcRequest::eth_get_transaction_receipt(id, self.transaction_hash)
    }

    #[must_use]
    pub fn block_number_request(&self, id: u64) -> JsonRpcRequest {
        JsonRpcRequest::eth_block_number(id)
    }

    #[must_use]
    pub fn requests(&self, receipt_id: u64, block_number_id: u64) -> [JsonRpcRequest; 2] {
        [
            self.receipt_request(receipt_id),
            self.block_number_request(block_number_id),
        ]
    }

    #[must_use]
    pub fn classify(
        &self,
        receipt: Option<&TxReceipt>,
        current_block: Option<u64>,
    ) -> TxConfirmationStatus {
        self.policy
            .classify(receipt, current_block, self.submitted_at_block)
    }
}

impl TxSubmissionPlan {
    #[must_use]
    pub fn new(request: UnsignedTxRequest, confirmation_policy: TxConfirmationPolicy) -> Self {
        Self {
            request,
            confirmation_policy,
            submitted_at_block: None,
        }
    }

    #[must_use]
    pub fn from_unsigned_tx(
        tx: &UnsignedTx,
        from: Option<Address>,
        preflight: TxPreflight,
        confirmation_policy: TxConfirmationPolicy,
    ) -> Self {
        Self::new(preflight.apply_to(tx, from), confirmation_policy)
    }

    #[must_use]
    pub fn from_unsigned_tx_with_fee_policy(
        tx: &UnsignedTx,
        from: Option<Address>,
        preflight: TxPreflight,
        fee_policy: TxFeePolicy,
        confirmation_policy: TxConfirmationPolicy,
    ) -> Self {
        Self::from_unsigned_tx(
            tx,
            from,
            preflight.with_fee_policy(fee_policy),
            confirmation_policy,
        )
    }

    #[must_use]
    pub const fn with_submitted_at_block(mut self, submitted_at_block: u64) -> Self {
        self.submitted_at_block = Some(submitted_at_block);
        self
    }

    #[must_use]
    pub fn estimate_gas_request(&self, id: u64) -> JsonRpcRequest {
        JsonRpcRequest::eth_estimate_gas(id, &self.request)
    }

    #[must_use]
    pub fn send_transaction_request(&self, id: u64) -> JsonRpcRequest {
        JsonRpcRequest::eth_send_transaction(id, &self.request)
    }

    #[must_use]
    pub fn send_raw_transaction_request(
        &self,
        id: u64,
        signed_transaction: &SignedRawTransaction,
    ) -> JsonRpcRequest {
        JsonRpcRequest::eth_send_raw_transaction(id, signed_transaction)
    }

    #[must_use]
    pub const fn confirmation_plan(&self, transaction_hash: TxHash) -> TxConfirmationPlan {
        TxConfirmationPlan {
            transaction_hash,
            policy: self.confirmation_policy,
            submitted_at_block: self.submitted_at_block,
        }
    }

    #[must_use]
    pub fn summary(&self) -> TxSubmissionPlanSummary {
        let calldata = decode_hex_data(&self.request.data).ok();
        TxSubmissionPlanSummary {
            from: self.request.from,
            to: self.request.to,
            selector: calldata
                .as_ref()
                .and_then(|data| data.get(..4))
                .map(|selector| format!("0x{}", hex::encode(selector))),
            calldata_bytes: calldata.as_ref().map(Vec::len),
            value: self.request.value.clone(),
            nonce: self.request.nonce.clone(),
            gas: self.request.gas.clone(),
            gas_price: self.request.gas_price.clone(),
            max_fee_per_gas: self.request.max_fee_per_gas.clone(),
            max_priority_fee_per_gas: self.request.max_priority_fee_per_gas.clone(),
            chain_id: self.request.chain_id.clone(),
            uses_eip1559_fees: self.request.max_fee_per_gas.is_some()
                || self.request.max_priority_fee_per_gas.is_some(),
            uses_legacy_gas_price: self.request.gas_price.is_some(),
            confirmation_policy: self.confirmation_policy,
            submitted_at_block: self.submitted_at_block,
        }
    }

    #[must_use]
    pub fn summarize_batch(plans: &[Self]) -> TxSubmissionPlanBatchSummary {
        TxSubmissionPlanBatchSummary::from_plans(plans)
    }
}

impl TxSubmissionPlanBatchSummary {
    #[must_use]
    pub fn from_plans(plans: &[TxSubmissionPlan]) -> Self {
        let plan_summaries = plans
            .iter()
            .map(TxSubmissionPlan::summary)
            .collect::<Vec<_>>();
        let total_calldata_bytes = plan_summaries
            .iter()
            .map(|summary| summary.calldata_bytes)
            .try_fold(0usize, |acc, bytes| {
                bytes.and_then(|bytes| acc.checked_add(bytes))
            });
        let total_gas = plan_summaries
            .iter()
            .map(|summary| summary.gas.as_deref().map(parse_rpc_quantity))
            .try_fold(0u128, |acc, gas| match gas {
                Some(Ok(gas)) => acc.checked_add(gas),
                _ => None,
            });
        let chain_id = plan_summaries
            .first()
            .and_then(|summary| summary.chain_id.clone());
        let all_same_chain_id = plan_summaries
            .iter()
            .all(|summary| summary.chain_id == chain_id);
        let ready_plans = plan_summaries
            .iter()
            .filter(|summary| tx_submission_plan_summary_is_ready(summary))
            .count();
        let not_ready_plans = plan_summaries.len().saturating_sub(ready_plans);

        Self {
            len: plans.len(),
            is_empty: plans.is_empty(),
            total_calldata_bytes,
            total_gas,
            first_nonce: plan_summaries
                .first()
                .and_then(|summary| summary.nonce.clone()),
            last_nonce: plan_summaries
                .last()
                .and_then(|summary| summary.nonce.clone()),
            chain_id,
            all_same_chain_id,
            ready_plans,
            not_ready_plans,
            has_ready_plans: ready_plans > 0,
            all_ready: !plans.is_empty() && not_ready_plans == 0,
            eip1559_transactions: plan_summaries
                .iter()
                .filter(|summary| summary.uses_eip1559_fees)
                .count(),
            legacy_gas_price_transactions: plan_summaries
                .iter()
                .filter(|summary| summary.uses_legacy_gas_price)
                .count(),
            plans: plan_summaries,
        }
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

impl TxPreflight {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            chain_id: None,
            nonce: None,
            gas: None,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        }
    }

    #[must_use]
    pub const fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    #[must_use]
    pub const fn with_nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    #[must_use]
    pub const fn with_gas(mut self, gas: u64) -> Self {
        self.gas = Some(gas);
        self
    }

    #[must_use]
    pub const fn with_gas_price(mut self, gas_price: u128) -> Self {
        self.gas_price = Some(gas_price);
        self
    }

    #[must_use]
    pub const fn with_eip1559_fees(
        mut self,
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    ) -> Self {
        self.max_fee_per_gas = Some(max_fee_per_gas);
        self.max_priority_fee_per_gas = Some(max_priority_fee_per_gas);
        self
    }

    #[must_use]
    pub const fn summary(self) -> TxPreflightSummary {
        let has_eip1559_fees =
            self.max_fee_per_gas.is_some() || self.max_priority_fee_per_gas.is_some();
        let has_complete_eip1559_fees =
            self.max_fee_per_gas.is_some() && self.max_priority_fee_per_gas.is_some();
        let has_any_fee = self.gas_price.is_some() || has_eip1559_fees;

        TxPreflightSummary {
            has_chain_id: self.chain_id.is_some(),
            has_nonce: self.nonce.is_some(),
            has_gas: self.gas.is_some(),
            has_gas_price: self.gas_price.is_some(),
            has_eip1559_fees,
            has_complete_eip1559_fees,
            has_any_fee,
            ready_for_submission_request: self.chain_id.is_some()
                && self.nonce.is_some()
                && self.gas.is_some()
                && has_any_fee,
            chain_id: self.chain_id,
            nonce: self.nonce,
            gas: self.gas,
            gas_price: self.gas_price,
            max_fee_per_gas: self.max_fee_per_gas,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
        }
    }

    #[must_use]
    pub fn with_fee_policy(mut self, policy: TxFeePolicy) -> Self {
        match policy {
            TxFeePolicy::Preserve => self,
            TxFeePolicy::LegacyGasPrice => {
                self.max_fee_per_gas = None;
                self.max_priority_fee_per_gas = None;
                self
            }
            TxFeePolicy::Eip1559FromGasPrice {
                max_fee_multiplier,
                min_priority_fee_per_gas,
            } => {
                let multiplier = u128::from(max_fee_multiplier.max(1));
                if self.max_fee_per_gas.is_none() {
                    self.max_fee_per_gas = self
                        .gas_price
                        .map(|gas_price| gas_price.saturating_mul(multiplier));
                }
                self.max_priority_fee_per_gas =
                    match (self.max_priority_fee_per_gas, min_priority_fee_per_gas) {
                        (Some(priority_fee), Some(min_priority_fee)) => {
                            Some(priority_fee.max(min_priority_fee))
                        }
                        (None, Some(min_priority_fee)) => Some(min_priority_fee),
                        (priority_fee, None) => priority_fee,
                    };
                self
            }
        }
    }

    /// Decode common transaction preflight JSON-RPC responses.
    pub fn from_rpc_responses(
        chain_id: Option<JsonRpcResponse<String>>,
        nonce: Option<JsonRpcResponse<String>>,
        gas: Option<JsonRpcResponse<String>>,
        gas_price: Option<JsonRpcResponse<String>>,
        max_fee_per_gas: Option<JsonRpcResponse<String>>,
        max_priority_fee_per_gas: Option<JsonRpcResponse<String>>,
    ) -> Result<Self, TxPreflightError> {
        Ok(Self {
            chain_id: decode_optional_u64_response("chain_id", chain_id)?,
            nonce: decode_optional_u64_response("nonce", nonce)?,
            gas: decode_optional_u64_response("gas", gas)?,
            gas_price: decode_optional_u128_response("gas_price", gas_price)?,
            max_fee_per_gas: decode_optional_u128_response("max_fee_per_gas", max_fee_per_gas)?,
            max_priority_fee_per_gas: decode_optional_u128_response(
                "max_priority_fee_per_gas",
                max_priority_fee_per_gas,
            )?,
        })
    }

    /// Convert preflight values into transaction request metadata.
    #[must_use]
    pub const fn to_metadata(self, from: Option<Address>) -> TxRequestMetadata {
        TxRequestMetadata {
            from,
            nonce: self.nonce,
            gas: self.gas,
            gas_price: self.gas_price,
            max_fee_per_gas: self.max_fee_per_gas,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
            chain_id: self.chain_id,
        }
    }

    /// Apply this preflight bundle to one SDK unsigned transaction.
    #[must_use]
    pub fn apply_to(self, tx: &UnsignedTx, from: Option<Address>) -> UnsignedTxRequest {
        tx.to_tx_request_with_metadata(self.to_metadata(from))
    }

    #[must_use]
    pub fn apply_to_with_fee_policy(
        self,
        tx: &UnsignedTx,
        from: Option<Address>,
        fee_policy: TxFeePolicy,
    ) -> UnsignedTxRequest {
        self.with_fee_policy(fee_policy).apply_to(tx, from)
    }
}

impl UnsignedCallBatchSummary {
    #[must_use]
    pub fn from_calls<'a>(calls: impl IntoIterator<Item = &'a UnsignedCall>) -> Self {
        let call_summaries = calls
            .into_iter()
            .map(UnsignedCall::summary)
            .collect::<Vec<_>>();
        let total_calldata_bytes = call_summaries
            .iter()
            .map(|summary| summary.calldata_bytes)
            .sum();
        let mut counts = BTreeMap::<Address, usize>::new();
        for summary in &call_summaries {
            *counts.entry(summary.to).or_default() += 1;
        }
        let contracts = counts
            .into_iter()
            .map(|(to, calls)| UnsignedCallContractSummary { to, calls })
            .collect::<Vec<_>>();

        Self {
            len: call_summaries.len(),
            is_empty: call_summaries.is_empty(),
            total_calldata_bytes,
            unique_contracts: contracts.len(),
            contracts,
            calls: call_summaries,
        }
    }
}

impl RpcBlockTag {
    #[must_use]
    pub fn to_rpc_param(self) -> String {
        match self {
            Self::Latest => "latest".to_owned(),
            Self::Earliest => "earliest".to_owned(),
            Self::Pending => "pending".to_owned(),
            Self::Safe => "safe".to_owned(),
            Self::Finalized => "finalized".to_owned(),
            Self::Number(block) => quantity_hex(u128::from(block)),
        }
    }
}

impl JsonRpcRequest {
    #[must_use]
    pub fn new(id: u64, method: impl Into<String>, params: Vec<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            method: method.into(),
            params,
        }
    }

    /// Build an `eth_call` request from a call query.
    #[must_use]
    pub fn eth_call(id: u64, query: &UnsignedCallQuery) -> Self {
        Self::new(
            id,
            "eth_call",
            vec![to_json_value(&query.call), to_json_value(&query.block)],
        )
    }

    /// Build an `eth_call` request directly from an unsigned call and block tag.
    #[must_use]
    pub fn eth_call_at(id: u64, call: &UnsignedCall, block: RpcBlockTag) -> Self {
        Self::eth_call(id, &call.to_query_at(block))
    }

    /// Build fixed-order `eth_call` requests for a call batch.
    ///
    /// Request ids are assigned as `first_id + index`, preserving the order
    /// expected by read-plan batch decoders.
    pub fn eth_call_batch<'a>(
        calls: impl IntoIterator<Item = &'a UnsignedCall>,
        block: RpcBlockTag,
        first_id: u64,
    ) -> Result<Vec<Self>, JsonRpcRequestError> {
        calls
            .into_iter()
            .enumerate()
            .map(|(index, call)| {
                let index_u64 =
                    u64::try_from(index).map_err(|_| JsonRpcRequestError::IdOverflow { index })?;
                let id = first_id
                    .checked_add(index_u64)
                    .ok_or(JsonRpcRequestError::IdOverflow { index })?;
                Ok(Self::eth_call_at(id, call, block))
            })
            .collect()
    }

    /// Return request ids in the same order as the supplied batch.
    #[must_use]
    pub fn ids<'a>(requests: impl IntoIterator<Item = &'a Self>) -> Vec<u64> {
        requests.into_iter().map(|request| request.id).collect()
    }

    /// Build an `eth_getLogs` request from an RPC-friendly log query.
    #[must_use]
    pub fn eth_get_logs(id: u64, query: &EventLogRpcQuery) -> Self {
        Self::new(id, "eth_getLogs", vec![to_json_value(query)])
    }

    /// Build an `eth_getTransactionReceipt` request.
    #[must_use]
    pub fn eth_get_transaction_receipt(id: u64, hash: TxHash) -> Self {
        Self::new(
            id,
            "eth_getTransactionReceipt",
            vec![to_json_value(hash.to_hex())],
        )
    }

    /// Build an `eth_blockNumber` request for confirmation checks.
    #[must_use]
    pub fn eth_block_number(id: u64) -> Self {
        Self::new(id, "eth_blockNumber", Vec::new())
    }

    /// Build an `eth_getTransactionCount` request for nonce discovery.
    #[must_use]
    pub fn eth_get_transaction_count(id: u64, address: Address, block: RpcBlockTag) -> Self {
        Self::new(
            id,
            "eth_getTransactionCount",
            vec![to_json_value(address), to_json_value(block.to_rpc_param())],
        )
    }

    /// Build an `eth_estimateGas` request for an unsigned transaction envelope.
    #[must_use]
    pub fn eth_estimate_gas(id: u64, request: &UnsignedTxRequest) -> Self {
        Self::new(id, "eth_estimateGas", vec![to_json_value(request)])
    }

    /// Build an `eth_gasPrice` request for legacy-fee fallback flows.
    #[must_use]
    pub fn eth_gas_price(id: u64) -> Self {
        Self::new(id, "eth_gasPrice", Vec::new())
    }

    /// Build an `eth_maxPriorityFeePerGas` request for EIP-1559 fee discovery.
    #[must_use]
    pub fn eth_max_priority_fee_per_gas(id: u64) -> Self {
        Self::new(id, "eth_maxPriorityFeePerGas", Vec::new())
    }

    /// Build an `eth_chainId` request.
    #[must_use]
    pub fn eth_chain_id(id: u64) -> Self {
        Self::new(id, "eth_chainId", Vec::new())
    }

    /// Build an `eth_sendTransaction` request for node-managed signing.
    ///
    /// Wallet services and relayers may prefer their own transaction request
    /// envelope. This helper is only the vanilla Ethereum JSON-RPC shape.
    #[must_use]
    pub fn eth_send_transaction(id: u64, request: &UnsignedTxRequest) -> Self {
        Self::new(id, "eth_sendTransaction", vec![to_json_value(request)])
    }

    /// Build an `eth_sendRawTransaction` request for externally signed bytes.
    #[must_use]
    pub fn eth_send_raw_transaction(id: u64, signed_transaction: &SignedRawTransaction) -> Self {
        Self::new(
            id,
            "eth_sendRawTransaction",
            vec![to_json_value(signed_transaction.to_hex())],
        )
    }

    /// Build an `eth_sendRawTransaction` request from provider-style raw hex.
    pub fn eth_send_raw_transaction_hex(
        id: u64,
        signed_transaction: impl AsRef<str>,
    ) -> Result<Self, SignedRawTransactionError> {
        Ok(Self::eth_send_raw_transaction(
            id,
            &SignedRawTransaction::from_hex(signed_transaction)?,
        ))
    }
}

impl<T> JsonRpcResponse<T> {
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.result.is_some() && self.error.is_none()
    }

    #[must_use]
    pub const fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Return the response result, or surface the provider error object.
    pub fn into_result(self) -> Result<T, JsonRpcResponseError> {
        if let Some(error) = self.error {
            return Err(JsonRpcResponseError::Rpc {
                code: error.code,
                message: error.message,
                data: error.data,
            });
        }

        self.result.ok_or(JsonRpcResponseError::MissingResult)
    }
}

impl JsonRpcResponse<String> {
    /// Extract and parse an `eth_call` result.
    pub fn into_call_return(self) -> Result<CallReturn, JsonRpcResultDecodeError> {
        Ok(CallReturn::from_hex(self.into_result()?)?)
    }

    /// Extract and parse an RPC quantity into `u128`.
    pub fn into_quantity_u128(self) -> Result<u128, JsonRpcResultDecodeError> {
        Ok(parse_rpc_quantity(&self.into_result()?)?)
    }

    /// Extract and parse an RPC quantity into `u64`.
    pub fn into_quantity_u64(self) -> Result<u64, JsonRpcResultDecodeError> {
        let quantity = self.into_quantity_u128()?;
        u64::try_from(quantity).map_err(|_| RpcQuantityError::Overflow.into())
    }

    /// Extract and parse an `eth_sendRawTransaction`/`eth_sendTransaction` hash.
    pub fn into_tx_hash(self) -> Result<TxHash, JsonRpcResultDecodeError> {
        Ok(TxHash::from_hex(self.into_result()?)?)
    }

    /// Match unordered `eth_call` batch responses to request ids and return
    /// call data in the original request order.
    pub fn into_call_return_batch_for_ids(
        responses: impl IntoIterator<Item = Self>,
        ordered_ids: impl IntoIterator<Item = u64>,
    ) -> Result<CallReturnBatch, JsonRpcBatchError> {
        let mut responses_by_id = BTreeMap::new();
        for response in responses {
            let id = response.id;
            if responses_by_id.insert(id, response).is_some() {
                return Err(JsonRpcBatchError::DuplicateResponse { id });
            }
        }

        let returns = ordered_ids
            .into_iter()
            .map(|id| {
                let response = responses_by_id
                    .remove(&id)
                    .ok_or(JsonRpcBatchError::MissingResponse { id })?;
                response
                    .into_call_return()
                    .map_err(|source| JsonRpcBatchError::Decode { id, source })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(CallReturnBatch::new(returns))
    }

    /// Match unordered `eth_call` batch responses to a request batch and return
    /// call data in the original request order.
    pub fn into_call_return_batch_for_requests<'a>(
        responses: impl IntoIterator<Item = Self>,
        requests: impl IntoIterator<Item = &'a JsonRpcRequest>,
    ) -> Result<CallReturnBatch, JsonRpcBatchError> {
        Self::into_call_return_batch_for_ids(responses, JsonRpcRequest::ids(requests))
    }
}

impl JsonRpcResponse<Vec<String>> {
    /// Extract and parse an ordered vector of call results.
    pub fn into_call_return_batch(self) -> Result<CallReturnBatch, JsonRpcResultDecodeError> {
        Ok(CallReturnBatch::from_hex(self.into_result()?)?)
    }
}

impl TxHash {
    #[must_use]
    pub const fn new(hash: B256) -> Self {
        Self(hash)
    }

    /// Parse a `0x`-prefixed or bare 32-byte transaction hash.
    pub fn from_hex(hash: impl AsRef<str>) -> Result<Self, TxHashError> {
        let bytes = decode_hex_data(hash.as_ref()).map_err(TxHashError::InvalidHex)?;
        if bytes.len() != 32 {
            return Err(TxHashError::InvalidLength {
                actual: bytes.len(),
            });
        }

        Ok(Self(B256::from_slice(&bytes)))
    }

    #[must_use]
    pub const fn as_b256(self) -> B256 {
        self.0
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }
}

impl SignedRawTransaction {
    pub fn new(bytes: Vec<u8>) -> Result<Self, SignedRawTransactionError> {
        if bytes.is_empty() {
            return Err(SignedRawTransactionError::Empty);
        }

        Ok(Self(bytes))
    }

    /// Parse a `0x`-prefixed or bare signed transaction hex payload.
    pub fn from_hex(data: impl AsRef<str>) -> Result<Self, SignedRawTransactionError> {
        Self::new(decode_hex_data(data.as_ref())?)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.0))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TxReceipt {
    #[must_use]
    pub fn new(transaction_hash: TxHash) -> Self {
        Self {
            transaction_hash,
            block_number: None,
            status: None,
            gas_used: None,
            effective_gas_price: None,
            logs: Vec::new(),
        }
    }

    /// Build a receipt from common JSON-RPC hex quantity fields.
    ///
    /// This does not fetch a receipt. It adapts provider-returned strings into
    /// the SDK's typed receipt shape.
    pub fn from_rpc_fields(
        transaction_hash: impl AsRef<str>,
        block_number: Option<&str>,
        status: Option<&str>,
        gas_used: Option<&str>,
        effective_gas_price: Option<&str>,
        logs: Vec<RawLog>,
    ) -> Result<Self, TxReceiptError> {
        Ok(Self {
            transaction_hash: TxHash::from_hex(transaction_hash)?,
            block_number: parse_optional_quantity_u64(block_number, "block_number")?,
            status: parse_optional_status(status)?,
            gas_used: parse_optional_quantity_u64(gas_used, "gas_used")?,
            effective_gas_price: parse_optional_quantity_u128(
                effective_gas_price,
                "effective_gas_price",
            )?,
            logs,
        })
    }

    #[must_use]
    pub fn with_block_number(mut self, block_number: u64) -> Self {
        self.block_number = Some(block_number);
        self
    }

    #[must_use]
    pub fn with_status(mut self, status: bool) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn with_gas_used(mut self, gas_used: u64) -> Self {
        self.gas_used = Some(gas_used);
        self
    }

    #[must_use]
    pub fn with_effective_gas_price(mut self, effective_gas_price: u128) -> Self {
        self.effective_gas_price = Some(effective_gas_price);
        self
    }

    #[must_use]
    pub fn with_logs(mut self, logs: Vec<RawLog>) -> Self {
        self.logs = logs;
        self
    }

    #[must_use]
    pub fn summary(&self) -> TxReceiptSummary {
        TxReceiptSummary {
            transaction_hash: self.transaction_hash,
            mined: self.is_mined(),
            success: self.is_success(),
            reverted: self.is_reverted(),
            block_number: self.block_number,
            status: self.status,
            gas_used: self.gas_used,
            effective_gas_price: self.effective_gas_price,
            execution_fee_paid: self.execution_fee_paid(),
            log_count: self.logs.len(),
            last_cursor: self.last_cursor(),
        }
    }

    #[must_use]
    pub const fn is_mined(&self) -> bool {
        self.block_number.is_some()
    }

    #[must_use]
    pub fn confirmations(&self, current_block: Option<u64>) -> Option<u64> {
        let block_number = self.block_number?;
        let current_block = current_block?;
        Some(current_block.saturating_sub(block_number).saturating_add(1))
    }

    #[must_use]
    pub fn confirmation_status(
        &self,
        policy: TxConfirmationPolicy,
        current_block: Option<u64>,
    ) -> TxConfirmationStatus {
        policy.classify(Some(self), current_block, None)
    }

    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status == Some(true)
    }

    #[must_use]
    pub fn is_reverted(&self) -> bool {
        self.status == Some(false)
    }

    #[must_use]
    pub fn last_cursor(&self) -> Option<RawLogCursor> {
        self.logs.iter().filter_map(RawLog::cursor).max()
    }

    /// Return the paid native-token fee in wei-like base units when available.
    #[must_use]
    pub fn execution_fee_paid(&self) -> Option<u128> {
        self.effective_gas_price?
            .checked_mul(u128::from(self.gas_used?))
    }

    pub fn decode_logs(
        &self,
        filters: &EventFilterSet,
    ) -> Result<DecodedTangentLogs, EventDecodeError> {
        filters.decode_logs(&self.logs)
    }

    pub fn decode_log_records(
        &self,
        filters: &EventFilterSet,
    ) -> Result<DecodedTangentLogRecords, EventDecodeError> {
        filters.decode_log_records(&self.logs)
    }
}

impl CallReturn {
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Parse a `0x`-prefixed or bare hex return payload.
    pub fn from_hex(data: impl AsRef<str>) -> Result<Self, CallReturnError> {
        Ok(Self {
            data: decode_hex_data(data.as_ref())?,
        })
    }

    /// Parse a batch of `0x`-prefixed or bare hex return payloads.
    pub fn from_hex_batch<I, S>(returns: I) -> Result<Vec<Self>, CallReturnError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        returns.into_iter().map(Self::from_hex).collect()
    }

    #[must_use]
    pub fn data_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }

    #[must_use]
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl AsRef<[u8]> for CallReturn {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl CallReturnBatch {
    #[must_use]
    pub fn new(returns: Vec<CallReturn>) -> Self {
        Self { returns }
    }

    /// Parse an ordered batch of `0x`-prefixed or bare hex return payloads.
    pub fn from_hex<I, S>(returns: I) -> Result<Self, CallReturnError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self::new(CallReturn::from_hex_batch(returns)?))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.returns.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.returns.is_empty()
    }

    #[must_use]
    pub fn as_returns(&self) -> &[CallReturn] {
        &self.returns
    }

    #[must_use]
    pub fn into_returns(self) -> Vec<CallReturn> {
        self.returns
    }

    #[must_use]
    pub fn data_hexes(&self) -> Vec<String> {
        self.returns.iter().map(CallReturn::data_hex).collect()
    }
}

mod call_return_data {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::CallReturn;

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
        CallReturn::from_hex(encoded)
            .map(|call_return| call_return.data)
            .map_err(serde::de::Error::custom)
    }
}

mod call_data {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::{decode_hex_data, CallDataError};

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
        decode_hex_data(&encoded)
            .map_err(CallDataError::InvalidHex)
            .map_err(serde::de::Error::custom)
    }
}

mod signed_raw_transaction_data {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::SignedRawTransaction;

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
        SignedRawTransaction::from_hex(encoded)
            .map(|signed| signed.0)
            .map_err(serde::de::Error::custom)
    }
}

mod tx_hash_hex {
    use alloy_primitives::B256;
    use serde::{Deserialize, Deserializer, Serializer};

    use super::TxHash;

    pub fn serialize<S>(hash: &B256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(hash)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<B256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        TxHash::from_hex(encoded)
            .map(TxHash::as_b256)
            .map_err(serde::de::Error::custom)
    }
}

fn decode_hex_data(data: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(strip_hex_prefix(data))
}

fn strip_hex_prefix(value: &str) -> &str {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
}

fn quantity_hex(value: u128) -> String {
    format!("0x{value:x}")
}

fn parse_rpc_quantity(value: &str) -> Result<u128, RpcQuantityError> {
    let value = strip_hex_prefix(value);
    if value.is_empty() {
        return Ok(0);
    }

    u128::from_str_radix(value, 16).map_err(|_| RpcQuantityError::InvalidHex(value.to_owned()))
}

fn parse_optional_quantity_u128(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<u128>, TxReceiptError> {
    value
        .map(parse_rpc_quantity)
        .transpose()
        .map_err(|source| TxReceiptError::Quantity { field, source })
}

fn parse_optional_quantity_u64(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<u64>, TxReceiptError> {
    parse_optional_quantity_u128(value, field)?
        .map(|value| u64::try_from(value).map_err(|_| RpcQuantityError::Overflow))
        .transpose()
        .map_err(|source| TxReceiptError::Quantity { field, source })
}

fn parse_optional_status(status: Option<&str>) -> Result<Option<bool>, TxReceiptError> {
    match parse_optional_quantity_u128(status, "status")? {
        Some(0) => Ok(Some(false)),
        Some(1) => Ok(Some(true)),
        Some(value) => Err(TxReceiptError::InvalidStatus(value)),
        None => Ok(None),
    }
}

fn decode_optional_u64_response(
    field: &'static str,
    response: Option<JsonRpcResponse<String>>,
) -> Result<Option<u64>, TxPreflightError> {
    response
        .map(JsonRpcResponse::into_quantity_u64)
        .transpose()
        .map_err(|source| TxPreflightError::Decode { field, source })
}

fn decode_optional_u128_response(
    field: &'static str,
    response: Option<JsonRpcResponse<String>>,
) -> Result<Option<u128>, TxPreflightError> {
    response
        .map(JsonRpcResponse::into_quantity_u128)
        .transpose()
        .map_err(|source| TxPreflightError::Decode { field, source })
}

fn to_json_value<T: Serialize>(value: T) -> serde_json::Value {
    serde_json::to_value(value).expect("SDK JSON-RPC request parameter serializes")
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256};

    use crate::events::{
        AccountRegisteredEvent, EventFilterSet, RawLog, RawLogCursor, RawLogMetadata,
        TangentEventKind,
    };
    use crate::AccountStatusPlan;

    use super::*;

    #[test]
    fn exposes_selector_helpers() {
        let call = UnsignedCall {
            to: Address::ZERO,
            data: vec![0x12, 0x34, 0x56, 0x78, 0xff],
        };

        assert_eq!(call.selector(), Some([0x12, 0x34, 0x56, 0x78]));
        assert_eq!(call.selector_hex(), Some("0x12345678".to_owned()));
        assert!(call.has_selector([0x12, 0x34, 0x56, 0x78]));
        assert!(!call.has_selector([0x12, 0x34, 0x56, 0x79]));
        assert_eq!(call.arguments(), Some([0xff].as_slice()));
        assert_eq!(call.data_hex(), "0x12345678ff");
        assert_eq!(call.data_len(), 5);
        assert_eq!(
            call.summary(),
            UnsignedCallSummary {
                to: Address::ZERO,
                selector: Some("0x12345678".to_owned()),
                calldata_bytes: 5,
            }
        );
        assert_eq!(
            call.to_request(),
            UnsignedCallRequest {
                to: Address::ZERO,
                data: "0x12345678ff".to_owned(),
            }
        );
        assert_eq!(
            serde_json::to_string(&call.to_request()).expect("serialize"),
            "{\"to\":\"0x0000000000000000000000000000000000000000\",\"data\":\"0x12345678ff\"}"
        );
        assert_eq!(RpcBlockTag::Latest.to_rpc_param(), "latest");
        assert_eq!(RpcBlockTag::Earliest.to_rpc_param(), "earliest");
        assert_eq!(RpcBlockTag::Pending.to_rpc_param(), "pending");
        assert_eq!(RpcBlockTag::Safe.to_rpc_param(), "safe");
        assert_eq!(RpcBlockTag::Finalized.to_rpc_param(), "finalized");
        assert_eq!(RpcBlockTag::Number(123).to_rpc_param(), "0x7b");
        assert_eq!(
            call.to_query_at(RpcBlockTag::Finalized),
            UnsignedCallQuery {
                call: call.to_request(),
                block: "finalized".to_owned(),
            }
        );
        assert_eq!(
            call.to_query_at(RpcBlockTag::Number(123)),
            UnsignedCallQuery {
                call: call.to_request(),
                block: "0x7b".to_owned(),
            }
        );
        assert_eq!(
            serde_json::to_string(&call.to_query_at(RpcBlockTag::Number(123)))
                .expect("serialize call query"),
            "{\"call\":{\"to\":\"0x0000000000000000000000000000000000000000\",\"data\":\"0x12345678ff\"},\"block\":\"0x7b\"}"
        );
        assert_eq!(
            serde_json::to_string(&call).expect("serialize call"),
            "{\"to\":\"0x0000000000000000000000000000000000000000\",\"data\":\"0x12345678ff\"}"
        );
        assert_eq!(
            serde_json::from_str::<UnsignedCall>(
                "{\"to\":\"0x0000000000000000000000000000000000000000\",\"data\":\"0x12345678ff\"}"
            )
            .expect("deserialize call"),
            call
        );
        assert_eq!(
            UnsignedCall::from_hex_data(Address::ZERO, "0X12345678ff").expect("hex calldata"),
            call
        );
        let other_call = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0xaa, 0xbb],
        };
        let batch = UnsignedCall::summarize_batch([&call, &other_call]);
        assert_eq!(batch.len, 2);
        assert!(!batch.is_empty);
        assert_eq!(batch.total_calldata_bytes, 7);
        assert_eq!(batch.unique_contracts, 2);
        assert_eq!(batch.contracts[0].calls, 1);
        assert_eq!(batch.contracts[1].calls, 1);
        assert_eq!(batch.calls[0].selector.as_deref(), Some("0x12345678"));
        assert_eq!(batch.calls[1].selector, None);
        let json = serde_json::to_string(&batch).expect("call batch summary serializes");
        let restored: UnsignedCallBatchSummary =
            serde_json::from_str(&json).expect("call batch summary deserializes");
        assert_eq!(restored, batch);
        assert!(matches!(
            UnsignedCall::from_hex_data(Address::ZERO, "0x123").expect_err("bad calldata"),
            CallDataError::InvalidHex(_)
        ));
        assert_eq!(
            call.to_tx_request(),
            tx_request(Address::ZERO, "0x12345678ff", "0x0")
        );
        assert_eq!(
            call.to_tx_request_with_value(16),
            tx_request(Address::ZERO, "0x12345678ff", "0x10")
        );
        let metadata = TxRequestMetadata::new()
            .with_from(Address::repeat_byte(0x44))
            .with_nonce(7)
            .with_gas(21_000)
            .with_eip1559_fees(2_000_000_000, 1_000_000_000)
            .with_chain_id(11111);
        let tx_with_metadata = call.to_tx_request_with_metadata(metadata);
        assert_eq!(tx_with_metadata.from, Some(Address::repeat_byte(0x44)));
        assert_eq!(tx_with_metadata.nonce.as_deref(), Some("0x7"));
        assert_eq!(tx_with_metadata.gas.as_deref(), Some("0x5208"));
        assert_eq!(
            tx_with_metadata.max_fee_per_gas.as_deref(),
            Some("0x77359400")
        );
        assert_eq!(
            tx_with_metadata.max_priority_fee_per_gas.as_deref(),
            Some("0x3b9aca00")
        );
        assert_eq!(tx_with_metadata.chain_id.as_deref(), Some("0x2b67"));
        assert_eq!(
            call.to_tx_request_with_value_and_metadata(
                16,
                TxRequestMetadata::new().with_gas_price(2_000_000_000),
            )
            .gas_price
            .as_deref(),
            Some("0x77359400")
        );
        assert_eq!(
            serde_json::to_string(&call.to_tx_request()).expect("serialize"),
            "{\"to\":\"0x0000000000000000000000000000000000000000\",\"data\":\"0x12345678ff\",\"value\":\"0x0\"}"
        );

        let other = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0xaa, 0xbb, 0xcc, 0xdd],
        };
        assert_eq!(
            UnsignedCall::to_requests([&call, &other]),
            vec![
                UnsignedCallRequest {
                    to: Address::ZERO,
                    data: "0x12345678ff".to_owned(),
                },
                UnsignedCallRequest {
                    to: Address::repeat_byte(0x11),
                    data: "0xaabbccdd".to_owned(),
                },
            ]
        );
        assert_eq!(
            UnsignedCall::to_queries_at([&call, &other], RpcBlockTag::Latest),
            vec![
                UnsignedCallQuery {
                    call: UnsignedCallRequest {
                        to: Address::ZERO,
                        data: "0x12345678ff".to_owned(),
                    },
                    block: "latest".to_owned(),
                },
                UnsignedCallQuery {
                    call: UnsignedCallRequest {
                        to: Address::repeat_byte(0x11),
                        data: "0xaabbccdd".to_owned(),
                    },
                    block: "latest".to_owned(),
                },
            ]
        );
        assert_eq!(
            UnsignedCall::to_tx_requests([&call, &other]),
            vec![
                tx_request(Address::ZERO, "0x12345678ff", "0x0"),
                tx_request(Address::repeat_byte(0x11), "0xaabbccdd", "0x0"),
            ]
        );
        let batch_metadata = TxBatchRequestMetadata::new()
            .with_from(Address::repeat_byte(0x44))
            .with_start_nonce(7)
            .with_gas(250_000)
            .with_eip1559_fees(2_000_000_000, 1_000_000_000)
            .with_chain_id(11111);
        let batch_requests =
            UnsignedCall::to_tx_requests_with_batch_metadata([&call, &other], batch_metadata)
                .expect("batch tx requests");
        assert_eq!(batch_requests.len(), 2);
        assert_eq!(batch_requests[0].from, Some(Address::repeat_byte(0x44)));
        assert_eq!(batch_requests[0].nonce.as_deref(), Some("0x7"));
        assert_eq!(batch_requests[1].nonce.as_deref(), Some("0x8"));
        assert_eq!(batch_requests[0].gas.as_deref(), Some("0x3d090"));
        assert_eq!(
            batch_requests[0].max_fee_per_gas.as_deref(),
            Some("0x77359400")
        );
        assert_eq!(
            batch_requests[0].max_priority_fee_per_gas.as_deref(),
            Some("0x3b9aca00")
        );
        assert_eq!(batch_requests[0].chain_id.as_deref(), Some("0x2b67"));
        assert_eq!(
            UnsignedCall::to_tx_requests_with_batch_metadata(
                [&call, &other],
                TxBatchRequestMetadata::new().with_start_nonce(u64::MAX),
            )
            .expect_err("second nonce overflows"),
            TxRequestMetadataError::NonceOverflow { index: 1 }
        );
    }

    #[test]
    fn selector_helpers_reject_short_data() {
        let call = UnsignedCall {
            to: Address::ZERO,
            data: vec![0x12, 0x34, 0x56],
        };

        assert_eq!(call.selector(), None);
        assert_eq!(call.selector_hex(), None);
        assert!(!call.has_selector([0x12, 0x34, 0x56, 0x78]));
        assert_eq!(call.arguments(), None);
    }

    #[test]
    fn builds_json_rpc_request_envelopes() {
        let call = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0x12, 0x34, 0x56, 0x78],
        };
        let query = call.to_query_at(RpcBlockTag::Number(123));

        let call_request = JsonRpcRequest::eth_call(1, &query);
        assert_eq!(call_request.jsonrpc, "2.0");
        assert_eq!(call_request.method, "eth_call");
        assert_eq!(
            serde_json::to_value(&call_request).expect("serialize eth_call"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "eth_call",
                "params": [
                    {
                        "to": "0x1111111111111111111111111111111111111111",
                        "data": "0x12345678",
                    },
                    "0x7b"
                ],
            })
        );
        assert_eq!(
            JsonRpcRequest::eth_call_at(1, &call, RpcBlockTag::Number(123)),
            call_request
        );

        let other = UnsignedCall {
            to: Address::repeat_byte(0x22),
            data: vec![0xaa, 0xbb, 0xcc, 0xdd],
        };
        let batch = JsonRpcRequest::eth_call_batch([&call, &other], RpcBlockTag::Latest, 7)
            .expect("call batch");
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].id, 7);
        assert_eq!(batch[1].id, 8);
        assert_eq!(batch[0].method, "eth_call");
        assert_eq!(batch[0].params[1], serde_json::json!("latest"));
        assert_eq!(
            JsonRpcRequest::eth_call_batch([&call, &other], RpcBlockTag::Latest, u64::MAX)
                .expect_err("second request id overflows"),
            JsonRpcRequestError::IdOverflow { index: 1 }
        );

        let log_query = EventLogRpcQuery {
            addresses: vec![Address::repeat_byte(0x33)],
            topics: vec![vec![B256::repeat_byte(0x44)]],
            from_block: Some("0x7b".to_owned()),
            to_block: Some("0xfa".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(JsonRpcRequest::eth_get_logs(9, &log_query))
                .expect("serialize eth_getLogs"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 9,
                "method": "eth_getLogs",
                "params": [{
                    "address": ["0x3333333333333333333333333333333333333333"],
                    "topics": [["0x4444444444444444444444444444444444444444444444444444444444444444"]],
                    "fromBlock": "0x7b",
                    "toBlock": "0xfa",
                }],
            })
        );

        let hash = TxHash::new(B256::repeat_byte(0x55));
        assert_eq!(
            JsonRpcRequest::eth_get_transaction_receipt(10, hash).params,
            vec![serde_json::json!(
                "0x5555555555555555555555555555555555555555555555555555555555555555"
            )]
        );

        let tx_request =
            call.to_tx_request_with_metadata(TxRequestMetadata::new().with_from(Address::ZERO));
        assert_eq!(
            JsonRpcRequest::eth_get_transaction_count(
                13,
                Address::repeat_byte(0x66),
                RpcBlockTag::Pending
            ),
            JsonRpcRequest::new(
                13,
                "eth_getTransactionCount",
                vec![
                    serde_json::json!("0x6666666666666666666666666666666666666666"),
                    serde_json::json!("pending"),
                ],
            )
        );
        assert_eq!(
            JsonRpcRequest::eth_estimate_gas(14, &tx_request).method,
            "eth_estimateGas"
        );
        assert_eq!(
            JsonRpcRequest::eth_gas_price(15).params,
            Vec::<serde_json::Value>::new()
        );
        assert_eq!(
            JsonRpcRequest::eth_max_priority_fee_per_gas(17).method,
            "eth_maxPriorityFeePerGas"
        );
        assert_eq!(JsonRpcRequest::eth_chain_id(16).method, "eth_chainId");
        assert_eq!(
            JsonRpcRequest::eth_block_number(18).method,
            "eth_blockNumber"
        );
        assert_eq!(
            JsonRpcRequest::eth_send_transaction(11, &tx_request).method,
            "eth_sendTransaction"
        );
        assert_eq!(
            serde_json::to_string(
                &JsonRpcRequest::eth_send_raw_transaction(
                    12,
                    &SignedRawTransaction::from_hex("0x02abcd").expect("signed tx"),
                )
            )
            .expect("serialize raw send"),
            "{\"jsonrpc\":\"2.0\",\"id\":12,\"method\":\"eth_sendRawTransaction\",\"params\":[\"0x02abcd\"]}"
        );
        assert_eq!(
            JsonRpcRequest::eth_send_raw_transaction_hex(12, "02abcd").expect("raw send request"),
            JsonRpcRequest::eth_send_raw_transaction(
                12,
                &SignedRawTransaction::from_hex("0x02abcd").expect("signed tx")
            )
        );
    }

    #[test]
    fn parses_json_rpc_response_envelopes() {
        let response: JsonRpcResponse<String> =
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"0x0007\"}")
                .expect("call response");
        assert!(response.is_success());
        assert!(!response.is_error());
        let call_return =
            CallReturn::from_hex(response.into_result().expect("result")).expect("return hex");
        assert_eq!(call_return.as_bytes(), &[0x00, 0x07]);
        let response: JsonRpcResponse<String> =
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"0x0007\"}")
                .expect("call response");
        assert_eq!(
            response.into_call_return().expect("call return").as_bytes(),
            &[0x00, 0x07]
        );

        let quantity_response: JsonRpcResponse<String> =
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":\"0x5208\"}")
                .expect("quantity response");
        assert_eq!(quantity_response.into_quantity_u64().expect("gas"), 21_000);
        let quantity_response: JsonRpcResponse<String> =
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":5,\"result\":\"0x2b67\"}")
                .expect("quantity response");
        assert_eq!(
            quantity_response.into_quantity_u128().expect("chain id"),
            11111
        );

        let hash_response: JsonRpcResponse<String> = serde_json::from_str(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":6,\"result\":\"{}\"}}",
            TxHash::new(B256::repeat_byte(0x77)).to_hex()
        ))
        .expect("hash response");
        assert_eq!(
            hash_response.into_tx_hash().expect("tx hash"),
            TxHash::new(B256::repeat_byte(0x77))
        );

        let batch_response: JsonRpcResponse<Vec<String>> =
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":[\"0x01\",\"0x0203\"]}")
                .expect("batch response");
        let batch = batch_response
            .into_call_return_batch()
            .expect("call return batch");
        assert_eq!(
            batch.data_hexes(),
            vec!["0x01".to_owned(), "0x0203".to_owned()]
        );

        let error_response: JsonRpcResponse<String> = serde_json::from_str(
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32000,\"message\":\"execution reverted\",\"data\":{\"reason\":\"paused\"}}}",
        )
        .expect("error response");
        assert!(error_response.is_error());
        assert_eq!(
            error_response.into_result().expect_err("rpc error"),
            JsonRpcResponseError::Rpc {
                code: -32000,
                message: "execution reverted".to_owned(),
                data: Some(serde_json::json!({ "reason": "paused" })),
            }
        );

        let missing_response: JsonRpcResponse<String> =
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":3}").expect("missing response");
        assert_eq!(
            missing_response.into_result().expect_err("missing result"),
            JsonRpcResponseError::MissingResult
        );
    }

    #[test]
    fn orders_json_rpc_call_batch_responses_by_request_id() {
        let first = UnsignedCall {
            to: Address::repeat_byte(0x11),
            data: vec![0x12, 0x34, 0x56, 0x78],
        };
        let second = UnsignedCall {
            to: Address::repeat_byte(0x22),
            data: vec![0xaa, 0xbb, 0xcc, 0xdd],
        };
        let requests = JsonRpcRequest::eth_call_batch([&first, &second], RpcBlockTag::Latest, 40)
            .expect("call batch requests");
        assert_eq!(JsonRpcRequest::ids(&requests), vec![40, 41]);

        let responses = vec![
            JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 41,
                result: Some("0x0203".to_owned()),
                error: None,
            },
            JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 40,
                result: Some("0x01".to_owned()),
                error: None,
            },
        ];
        let batch = JsonRpcResponse::into_call_return_batch_for_requests(responses, &requests)
            .expect("ordered returns");
        assert_eq!(
            batch.data_hexes(),
            vec!["0x01".to_owned(), "0x0203".to_owned()]
        );

        let duplicate = vec![
            JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 40,
                result: Some("0x01".to_owned()),
                error: None,
            },
            JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 40,
                result: Some("0x02".to_owned()),
                error: None,
            },
        ];
        assert_eq!(
            JsonRpcResponse::into_call_return_batch_for_requests(duplicate, &requests)
                .expect_err("duplicate response id"),
            JsonRpcBatchError::DuplicateResponse { id: 40 }
        );

        let missing = vec![JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id: 40,
            result: Some("0x01".to_owned()),
            error: None,
        }];
        assert_eq!(
            JsonRpcResponse::into_call_return_batch_for_requests(missing, &requests)
                .expect_err("missing response id"),
            JsonRpcBatchError::MissingResponse { id: 41 }
        );

        let reverted = vec![
            JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 40,
                result: Some("0x01".to_owned()),
                error: None,
            },
            JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 41,
                result: None,
                error: Some(JsonRpcErrorObject {
                    code: -32000,
                    message: "execution reverted".to_owned(),
                    data: None,
                }),
            },
        ];
        assert_eq!(
            JsonRpcResponse::into_call_return_batch_for_requests(reverted, &requests)
                .expect_err("rpc response error"),
            JsonRpcBatchError::Decode {
                id: 41,
                source: JsonRpcResultDecodeError::Response(JsonRpcResponseError::Rpc {
                    code: -32000,
                    message: "execution reverted".to_owned(),
                    data: None,
                }),
            }
        );
    }

    #[test]
    fn decodes_transaction_preflight_responses_into_metadata() {
        let preflight = TxPreflight::from_rpc_responses(
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 20,
                result: Some("0x2b67".to_owned()),
                error: None,
            }),
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 21,
                result: Some("0x7".to_owned()),
                error: None,
            }),
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 22,
                result: Some("0x3d090".to_owned()),
                error: None,
            }),
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 23,
                result: Some("0x77359400".to_owned()),
                error: None,
            }),
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 24,
                result: Some("0x77359400".to_owned()),
                error: None,
            }),
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 25,
                result: Some("0x3b9aca00".to_owned()),
                error: None,
            }),
        )
        .expect("preflight decodes");

        assert_eq!(
            preflight,
            TxPreflight::new()
                .with_chain_id(11111)
                .with_nonce(7)
                .with_gas(250_000)
                .with_gas_price(2_000_000_000)
                .with_eip1559_fees(2_000_000_000, 1_000_000_000)
        );
        let summary = preflight.summary();
        assert!(summary.has_chain_id);
        assert!(summary.has_nonce);
        assert!(summary.has_gas);
        assert!(summary.has_gas_price);
        assert!(summary.has_eip1559_fees);
        assert!(summary.has_complete_eip1559_fees);
        assert!(summary.has_any_fee);
        assert!(summary.ready_for_submission_request);
        assert_eq!(summary.chain_id, Some(11111));
        assert_eq!(summary.nonce, Some(7));
        assert_eq!(summary.gas, Some(250_000));
        assert_eq!(summary.gas_price, Some(2_000_000_000));
        assert_eq!(summary.max_fee_per_gas, Some(2_000_000_000));
        assert_eq!(summary.max_priority_fee_per_gas, Some(1_000_000_000));
        let json = serde_json::to_string(&summary).expect("preflight summary serializes");
        let restored: TxPreflightSummary =
            serde_json::from_str(&json).expect("preflight summary deserializes");
        assert_eq!(restored, summary);
        assert_eq!(
            preflight.to_metadata(Some(Address::repeat_byte(0x44))),
            TxRequestMetadata::new()
                .with_from(Address::repeat_byte(0x44))
                .with_nonce(7)
                .with_gas(250_000)
                .with_gas_price(2_000_000_000)
                .with_eip1559_fees(2_000_000_000, 1_000_000_000)
                .with_chain_id(11111)
        );

        let tx = UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x12, 0x34, 0x56, 0x78],
        };
        let request = preflight.apply_to(&tx, Some(Address::repeat_byte(0x44)));
        assert_eq!(request.from, Some(Address::repeat_byte(0x44)));
        assert_eq!(request.nonce.as_deref(), Some("0x7"));
        assert_eq!(request.gas.as_deref(), Some("0x3d090"));
        assert_eq!(request.gas_price.as_deref(), Some("0x77359400"));
        assert_eq!(request.max_fee_per_gas.as_deref(), Some("0x77359400"));
        assert_eq!(
            request.max_priority_fee_per_gas.as_deref(),
            Some("0x3b9aca00")
        );
        assert_eq!(request.chain_id.as_deref(), Some("0x2b67"));

        let failed = TxPreflight::from_rpc_responses(
            None,
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: 21,
                result: None,
                error: Some(JsonRpcErrorObject {
                    code: -32000,
                    message: "nonce unavailable".to_owned(),
                    data: None,
                }),
            }),
            None,
            None,
            None,
            None,
        )
        .expect_err("nonce preflight fails");
        assert_eq!(
            failed,
            TxPreflightError::Decode {
                field: "nonce",
                source: JsonRpcResultDecodeError::Response(JsonRpcResponseError::Rpc {
                    code: -32000,
                    message: "nonce unavailable".to_owned(),
                    data: None,
                }),
            }
        );
    }

    #[test]
    fn applies_transaction_fee_policies_to_preflight_values() {
        let preflight = TxPreflight::new()
            .with_chain_id(11111)
            .with_nonce(7)
            .with_gas(250_000)
            .with_gas_price(2_000_000_000)
            .with_eip1559_fees(5_000_000_000, 1_000_000_000);

        assert_eq!(preflight.with_fee_policy(TxFeePolicy::Preserve), preflight);
        assert_eq!(
            preflight.with_fee_policy(TxFeePolicy::LegacyGasPrice),
            TxPreflight::new()
                .with_chain_id(11111)
                .with_nonce(7)
                .with_gas(250_000)
                .with_gas_price(2_000_000_000)
        );

        let derived = TxPreflight::new()
            .with_chain_id(11111)
            .with_nonce(7)
            .with_gas(250_000)
            .with_gas_price(2_000_000_000)
            .with_fee_policy(TxFeePolicy::Eip1559FromGasPrice {
                max_fee_multiplier: 2,
                min_priority_fee_per_gas: Some(1_500_000_000),
            });
        assert_eq!(derived.max_fee_per_gas, Some(4_000_000_000));
        assert_eq!(derived.max_priority_fee_per_gas, Some(1_500_000_000));
        let derived_summary = derived.summary();
        assert!(derived_summary.has_complete_eip1559_fees);
        assert!(derived_summary.ready_for_submission_request);

        let floored = preflight.with_fee_policy(TxFeePolicy::Eip1559FromGasPrice {
            max_fee_multiplier: 0,
            min_priority_fee_per_gas: Some(2_000_000_000),
        });
        assert_eq!(floored.max_fee_per_gas, Some(5_000_000_000));
        assert_eq!(floored.max_priority_fee_per_gas, Some(2_000_000_000));

        let incomplete = TxPreflight::new()
            .with_chain_id(11111)
            .with_nonce(7)
            .with_gas(250_000);
        let incomplete_summary = incomplete.summary();
        assert!(!incomplete_summary.has_any_fee);
        assert!(!incomplete_summary.ready_for_submission_request);
    }

    #[test]
    fn submission_plan_can_apply_fee_policy() {
        let tx = UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x12, 0x34, 0x56, 0x78],
        };
        let preflight = TxPreflight::new()
            .with_chain_id(11111)
            .with_nonce(7)
            .with_gas(250_000)
            .with_gas_price(2_000_000_000)
            .with_eip1559_fees(2_000_000_000, 1_000_000_000);

        let plan = TxSubmissionPlan::from_unsigned_tx_with_fee_policy(
            &tx,
            Some(Address::repeat_byte(0x44)),
            preflight,
            TxFeePolicy::LegacyGasPrice,
            TxConfirmationPolicy::new(2),
        );

        assert_eq!(plan.request.gas_price.as_deref(), Some("0x77359400"));
        assert_eq!(plan.request.max_fee_per_gas, None);
        assert_eq!(plan.request.max_priority_fee_per_gas, None);
    }

    #[test]
    fn summarizes_submission_plans_for_review() {
        let first_tx = UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x12, 0x34, 0x56, 0x78, 0xaa],
        };
        let second_tx = UnsignedCall {
            to: Address::repeat_byte(0x66),
            data: vec![0xaa, 0xbb, 0xcc, 0xdd],
        };
        let preflight = TxPreflight::new()
            .with_chain_id(11111)
            .with_nonce(7)
            .with_gas(21_000)
            .with_gas_price(2_000_000_000)
            .with_eip1559_fees(4_000_000_000, 1_000_000_000);
        let first = TxSubmissionPlan::from_unsigned_tx(
            &first_tx,
            Some(Address::repeat_byte(0x44)),
            preflight,
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        )
        .with_submitted_at_block(123);
        let second = TxSubmissionPlan::from_unsigned_tx(
            &second_tx,
            Some(Address::repeat_byte(0x44)),
            TxPreflight {
                nonce: Some(8),
                gas: Some(30_000),
                ..preflight
            },
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );

        let summary = first.summary();

        assert_eq!(summary.from, Some(Address::repeat_byte(0x44)));
        assert_eq!(summary.to, Address::repeat_byte(0x55));
        assert_eq!(summary.selector.as_deref(), Some("0x12345678"));
        assert_eq!(summary.calldata_bytes, Some(5));
        assert_eq!(summary.value, "0x0");
        assert_eq!(summary.nonce.as_deref(), Some("0x7"));
        assert_eq!(summary.gas.as_deref(), Some("0x5208"));
        assert_eq!(summary.chain_id.as_deref(), Some("0x2b67"));
        assert!(summary.uses_eip1559_fees);
        assert!(summary.uses_legacy_gas_price);
        assert_eq!(summary.submitted_at_block, Some(123));

        let batch = TxSubmissionPlan::summarize_batch(&[first, second]);

        assert_eq!(batch.len, 2);
        assert!(!batch.is_empty);
        assert_eq!(batch.total_calldata_bytes, Some(9));
        assert_eq!(batch.total_gas, Some(51_000));
        assert_eq!(batch.first_nonce.as_deref(), Some("0x7"));
        assert_eq!(batch.last_nonce.as_deref(), Some("0x8"));
        assert_eq!(batch.chain_id.as_deref(), Some("0x2b67"));
        assert!(batch.all_same_chain_id);
        assert_eq!(batch.ready_plans, 2);
        assert_eq!(batch.not_ready_plans, 0);
        assert!(batch.has_ready_plans);
        assert!(batch.all_ready);
        assert_eq!(batch.eip1559_transactions, 2);
        assert_eq!(batch.legacy_gas_price_transactions, 2);
        assert_eq!(batch.plans.len(), 2);

        let json = serde_json::to_string(&batch).expect("summary serializes");
        let restored: TxSubmissionPlanBatchSummary =
            serde_json::from_str(&json).expect("summary deserializes");
        assert_eq!(restored, batch);
        let mut legacy_json = serde_json::to_value(&batch).expect("batch summary value");
        let legacy_object = legacy_json.as_object_mut().expect("batch summary object");
        legacy_object.remove("ready_plans");
        legacy_object.remove("not_ready_plans");
        legacy_object.remove("has_ready_plans");
        legacy_object.remove("all_ready");
        let legacy: TxSubmissionPlanBatchSummary =
            serde_json::from_value(legacy_json).expect("legacy summary deserializes");
        assert_eq!(legacy.ready_plans, 0);
        assert_eq!(legacy.not_ready_plans, 0);
        assert!(!legacy.has_ready_plans);
        assert!(!legacy.all_ready);

        let incomplete_plan = TxSubmissionPlan::from_unsigned_tx(
            &first_tx,
            Some(Address::repeat_byte(0x44)),
            TxPreflight::new()
                .with_chain_id(11111)
                .with_nonce(9)
                .with_gas(21_000),
            TxConfirmationPolicy::new(2),
        );
        let incomplete_batch = TxSubmissionPlan::summarize_batch(&[incomplete_plan]);
        assert_eq!(incomplete_batch.ready_plans, 0);
        assert_eq!(incomplete_batch.not_ready_plans, 1);
        assert!(!incomplete_batch.has_ready_plans);
        assert!(!incomplete_batch.all_ready);
    }

    #[test]
    fn parses_signed_raw_transactions() {
        let signed = SignedRawTransaction::from_hex("0X02abcd").expect("signed tx");
        assert_eq!(signed.as_bytes(), &[0x02, 0xab, 0xcd]);
        assert_eq!(signed.to_hex(), "0x02abcd");
        assert_eq!(signed.len(), 3);
        assert!(!signed.is_empty());
        assert_eq!(
            serde_json::to_string(&signed).expect("serialize signed tx"),
            "\"0x02abcd\""
        );
        assert_eq!(
            serde_json::from_str::<SignedRawTransaction>("\"0x02abcd\"")
                .expect("deserialize signed tx"),
            signed
        );
        assert!(serde_json::from_str::<SignedRawTransaction>("\"0x\"").is_err());
        assert_eq!(
            SignedRawTransaction::from_hex("0x").expect_err("empty signed tx"),
            SignedRawTransactionError::Empty
        );
        assert!(matches!(
            SignedRawTransaction::from_hex("0xabc").expect_err("bad signed tx hex"),
            SignedRawTransactionError::InvalidHex(_)
        ));
        assert!(matches!(
            JsonRpcRequest::eth_send_raw_transaction_hex(1, "0x")
                .expect_err("empty raw transaction"),
            SignedRawTransactionError::Empty
        ));
    }

    #[test]
    fn parses_transport_call_returns() {
        let empty = CallReturn::from_hex("0x").expect("empty return");
        assert!(empty.is_empty());
        assert_eq!(empty.data_len(), 0);
        assert_eq!(empty.data_hex(), "0x");
        assert_eq!(empty.as_bytes(), &[] as &[u8]);

        let value = CallReturn::from_hex("0x1234").expect("prefixed return");
        assert_eq!(value, CallReturn::new(vec![0x12, 0x34]));
        assert_eq!(value.as_bytes(), &[0x12, 0x34]);
        assert_eq!(CallReturn::from_hex("1234").expect("bare return"), value);
        assert_eq!(
            serde_json::to_string(&value).expect("serialize"),
            "{\"data\":\"0x1234\"}"
        );
        assert_eq!(
            serde_json::from_str::<CallReturn>("{\"data\":\"0x1234\"}").expect("deserialize"),
            value
        );

        assert_eq!(
            CallReturn::from_hex_batch(["0x", "0x1234"]).expect("batch"),
            vec![CallReturn::new(vec![]), CallReturn::new(vec![0x12, 0x34])]
        );
        assert!(matches!(
            CallReturn::from_hex("0x123").expect_err("odd hex"),
            CallReturnError::InvalidHex(_)
        ));

        let batch = CallReturnBatch::from_hex(["0x", "0x1234"]).expect("return batch");
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
        assert_eq!(
            batch.data_hexes(),
            vec!["0x".to_owned(), "0x1234".to_owned()]
        );
        assert_eq!(
            serde_json::to_string(&batch).expect("serialize batch"),
            "{\"returns\":[{\"data\":\"0x\"},{\"data\":\"0x1234\"}]}"
        );
        assert_eq!(
            serde_json::from_str::<CallReturnBatch>(
                "{\"returns\":[{\"data\":\"0x\"},{\"data\":\"0x1234\"}]}"
            )
            .expect("deserialize batch"),
            batch
        );
        assert_eq!(batch.clone().into_returns(), batch.returns);
    }

    #[test]
    fn call_return_batches_feed_plan_decoders() {
        let plan = AccountStatusPlan::new(addr(0x20), addr(0x30), 7);
        let owner = address_word(addr(0x30));
        let account_id = word(7);
        let total = word(9);
        let batch = CallReturnBatch::from_hex([
            format!("0x{}", hex::encode(owner)),
            format!("0x{}", hex::encode(account_id)),
            format!("0x{}", hex::encode(total)),
        ])
        .expect("return batch");

        let status = plan
            .decode_return_slices(batch.as_returns())
            .expect("status decodes from return batch");
        assert_eq!(status.owner_of_account, addr(0x30));
        assert_eq!(status.account_id_of_owner, 7);
        assert_eq!(status.total_accounts, 9);
    }

    #[test]
    fn parses_transaction_hashes_and_receipts() {
        let hash = TxHash::from_hex(format!("0x{}", "11".repeat(32))).expect("tx hash");
        assert_eq!(hash.as_b256(), B256::repeat_byte(0x11));
        assert_eq!(hash.to_hex(), format!("0x{}", "11".repeat(32)));
        assert_eq!(
            TxHash::from_hex("0x1234").expect_err("short hash"),
            TxHashError::InvalidLength { actual: 2 }
        );
        assert!(matches!(
            TxHash::from_hex("0xzz").expect_err("invalid hash"),
            TxHashError::InvalidHex(_)
        ));
        assert_eq!(
            serde_json::to_string(&hash).expect("serialize hash"),
            format!("\"0x{}\"", "11".repeat(32))
        );
        assert_eq!(
            serde_json::from_str::<TxHash>(&format!("\"0x{}\"", "11".repeat(32)))
                .expect("deserialize hash"),
            hash
        );

        let account_manager = Address::repeat_byte(0x22);
        let filter_set = EventFilterSet::new(vec![
            TangentEventKind::AccountRegistered.filter(account_manager)
        ]);
        let registered_log = RawLog::new(
            account_manager,
            vec![
                AccountRegisteredEvent::topic0(),
                topic_u128(7),
                topic_address(Address::repeat_byte(0x33)),
            ],
            word(123).to_vec(),
        )
        .with_metadata(RawLogMetadata::new(Some(10), Some(hash.as_b256()), Some(3)));
        let receipt = TxReceipt::new(hash)
            .with_block_number(10)
            .with_status(true)
            .with_gas_used(21_000)
            .with_effective_gas_price(2_000_000_000)
            .with_logs(vec![registered_log]);

        assert!(receipt.is_mined());
        assert!(receipt.is_success());
        assert!(!receipt.is_reverted());
        assert_eq!(receipt.execution_fee_paid(), Some(42_000_000_000_000));
        assert_eq!(receipt.last_cursor(), Some(RawLogCursor::new(10, 3)));
        assert_eq!(
            receipt.summary(),
            TxReceiptSummary {
                transaction_hash: hash,
                mined: true,
                success: true,
                reverted: false,
                block_number: Some(10),
                status: Some(true),
                gas_used: Some(21_000),
                effective_gas_price: Some(2_000_000_000),
                execution_fee_paid: Some(42_000_000_000_000),
                log_count: 1,
                last_cursor: Some(RawLogCursor::new(10, 3)),
            }
        );
        let serialized_summary = serde_json::to_string(&receipt.summary()).expect("summary json");
        assert!(serialized_summary.contains("\"execution_fee_paid\":42000000000000"));
        assert_eq!(
            serde_json::from_str::<TxReceiptSummary>(&serialized_summary)
                .expect("deserialize receipt summary"),
            receipt.summary()
        );
        assert_eq!(
            receipt
                .decode_logs(&filter_set)
                .expect("receipt event logs")
                .known_logs(),
            1
        );

        let serialized_receipt = serde_json::to_string(&receipt).expect("serialize receipt");
        assert!(serialized_receipt.contains("\"transaction_hash\":\"0x"));
        assert!(serialized_receipt.contains("\"logs\""));
        assert_eq!(
            serde_json::from_str::<TxReceipt>(&serialized_receipt).expect("deserialize receipt"),
            receipt
        );
        assert_eq!(
            TxReceipt::from_rpc_fields(
                hash.to_hex(),
                Some("0xa"),
                Some("0x1"),
                Some("0x5208"),
                Some("0x77359400"),
                receipt.logs.clone(),
            )
            .expect("receipt from RPC fields"),
            receipt
        );
        let pending_receipt =
            TxReceipt::from_rpc_fields(hash.to_hex(), None, None, None, None, vec![])
                .expect("pending receipt from RPC fields");
        assert_eq!(pending_receipt.transaction_hash, hash);
        assert!(!pending_receipt.is_mined());
        assert_eq!(pending_receipt.status, None);
        assert_eq!(pending_receipt.execution_fee_paid(), None);
        assert_eq!(
            pending_receipt.summary(),
            TxReceiptSummary {
                transaction_hash: hash,
                mined: false,
                success: false,
                reverted: false,
                block_number: None,
                status: None,
                gas_used: None,
                effective_gas_price: None,
                execution_fee_paid: None,
                log_count: 0,
                last_cursor: None,
            }
        );
        assert_eq!(
            TxReceipt::from_rpc_fields(
                hash.to_hex(),
                Some("0x10000000000000000"),
                Some("0x1"),
                None,
                None,
                vec![],
            )
            .expect_err("block overflow"),
            TxReceiptError::Quantity {
                field: "block_number",
                source: RpcQuantityError::Overflow,
            }
        );
        assert_eq!(
            TxReceipt::from_rpc_fields(hash.to_hex(), None, Some("0x2"), None, None, vec![])
                .expect_err("invalid status"),
            TxReceiptError::InvalidStatus(2)
        );
        assert_eq!(
            TxReceipt::from_rpc_fields(hash.to_hex(), None, Some("0xzz"), None, None, vec![])
                .expect_err("invalid status hex"),
            TxReceiptError::Quantity {
                field: "status",
                source: RpcQuantityError::InvalidHex("zz".to_owned()),
            }
        );

        let reverted_receipt = TxReceipt::new(hash).with_status(false);
        assert!(!reverted_receipt.is_mined());
        assert!(!reverted_receipt.is_success());
        assert!(reverted_receipt.is_reverted());
        assert_eq!(reverted_receipt.execution_fee_paid(), None);
        assert_eq!(reverted_receipt.last_cursor(), None);

        let overflow_receipt = TxReceipt::new(hash)
            .with_gas_used(u64::MAX)
            .with_effective_gas_price(u128::MAX);
        assert_eq!(overflow_receipt.execution_fee_paid(), None);
    }

    #[test]
    fn classifies_transaction_confirmation_status() {
        let hash = TxHash::new(B256::repeat_byte(0x33));
        let policy = TxConfirmationPolicy::new(3).with_timeout_blocks(20);

        let receipt = TxReceipt::new(hash).with_block_number(10).with_status(true);
        assert_eq!(receipt.confirmations(Some(10)), Some(1));
        assert_eq!(receipt.confirmations(Some(12)), Some(3));
        assert_eq!(receipt.confirmations(Some(9)), Some(1));
        assert_eq!(receipt.confirmations(None), None);
        assert_eq!(
            receipt.confirmation_status(policy, Some(11)),
            TxConfirmationStatus::Pending { confirmations: 2 }
        );
        assert!(TxConfirmationStatus::Pending { confirmations: 2 }.is_pending());
        assert!(TxConfirmationStatus::Pending { confirmations: 2 }.should_continue_polling());
        assert_eq!(
            TxConfirmationStatus::Pending { confirmations: 2 }.confirmations(),
            Some(2)
        );
        assert_eq!(
            receipt.confirmation_status(policy, Some(12)),
            TxConfirmationStatus::Confirmed { confirmations: 3 }
        );
        assert!(TxConfirmationStatus::Confirmed { confirmations: 3 }.is_confirmed());
        assert!(TxConfirmationStatus::Confirmed { confirmations: 3 }.is_terminal());

        let unknown_status_receipt = TxReceipt::new(hash).with_block_number(10);
        assert_eq!(
            unknown_status_receipt.confirmation_status(policy, Some(15)),
            TxConfirmationStatus::Pending { confirmations: 6 }
        );

        let reverted_receipt = TxReceipt::new(hash)
            .with_block_number(10)
            .with_status(false);
        assert_eq!(
            reverted_receipt.confirmation_status(policy, Some(12)),
            TxConfirmationStatus::Reverted { confirmations: 3 }
        );
        assert!(TxConfirmationStatus::Reverted { confirmations: 3 }.is_reverted());

        assert_eq!(
            policy.classify(None, Some(119), Some(100)),
            TxConfirmationStatus::Pending { confirmations: 0 }
        );
        assert_eq!(
            policy.classify(None, Some(120), Some(100)),
            TxConfirmationStatus::TimedOut
        );
        assert!(TxConfirmationStatus::TimedOut.is_timed_out());
        assert_eq!(TxConfirmationStatus::TimedOut.confirmations(), None);
        assert_eq!(
            policy.classify(None, None, Some(100)),
            TxConfirmationStatus::Pending { confirmations: 0 }
        );
    }

    #[test]
    fn builds_transaction_confirmation_plan_requests() {
        let hash = TxHash::new(B256::repeat_byte(0x44));
        let plan =
            TxConfirmationPlan::new(hash, TxConfirmationPolicy::new(2).with_timeout_blocks(10))
                .with_submitted_at_block(100);

        assert_eq!(
            plan.receipt_request(30),
            JsonRpcRequest::eth_get_transaction_receipt(30, hash)
        );
        assert_eq!(
            plan.block_number_request(31),
            JsonRpcRequest::eth_block_number(31)
        );
        let requests = plan.requests(30, 31);
        assert_eq!(requests[0].method, "eth_getTransactionReceipt");
        assert_eq!(requests[1].method, "eth_blockNumber");
        assert_eq!(
            plan.summary(),
            TxConfirmationPlanSummary {
                transaction_hash: hash,
                required_confirmations: 2,
                timeout_blocks: Some(10),
                submitted_at_block: Some(100),
                request_count: 2,
            }
        );
        let serialized_summary = serde_json::to_string(&plan.summary()).expect("summary json");
        assert!(serialized_summary.contains("\"required_confirmations\":2"));
        assert_eq!(
            serde_json::from_str::<TxConfirmationPlanSummary>(&serialized_summary)
                .expect("deserialize confirmation plan summary"),
            plan.summary()
        );

        let block_response: JsonRpcResponse<String> =
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":31,\"result\":\"0x7c\"}")
                .expect("block response");
        let current_block = block_response
            .into_quantity_u64()
            .expect("current block decodes");
        assert_eq!(current_block, 124);

        let receipt = TxReceipt::new(hash)
            .with_block_number(123)
            .with_status(true);
        assert_eq!(
            plan.classify(Some(&receipt), Some(current_block)),
            TxConfirmationStatus::Confirmed { confirmations: 2 }
        );
        assert_eq!(
            plan.classify(None, Some(109)),
            TxConfirmationStatus::Pending { confirmations: 0 }
        );
        assert_eq!(
            plan.classify(None, Some(110)),
            TxConfirmationStatus::TimedOut
        );
    }

    #[test]
    fn builds_transaction_submission_plan_requests() {
        let tx = UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x12, 0x34, 0x56, 0x78],
        };
        let preflight = TxPreflight::new()
            .with_chain_id(11111)
            .with_nonce(7)
            .with_gas(250_000)
            .with_eip1559_fees(2_000_000_000, 1_000_000_000);
        let policy = TxConfirmationPolicy::new(2).with_timeout_blocks(20);
        let plan = TxSubmissionPlan::from_unsigned_tx(
            &tx,
            Some(Address::repeat_byte(0x44)),
            preflight,
            policy,
        )
        .with_submitted_at_block(100);

        assert_eq!(plan.request.from, Some(Address::repeat_byte(0x44)));
        assert_eq!(plan.request.to, Address::repeat_byte(0x55));
        assert_eq!(plan.request.nonce.as_deref(), Some("0x7"));
        assert_eq!(plan.request.gas.as_deref(), Some("0x3d090"));
        assert_eq!(plan.request.chain_id.as_deref(), Some("0x2b67"));

        assert_eq!(plan.estimate_gas_request(1).method, "eth_estimateGas");
        assert_eq!(
            plan.send_transaction_request(2).method,
            "eth_sendTransaction"
        );
        let signed = SignedRawTransaction::from_hex("0x02abcd").expect("signed tx");
        assert_eq!(
            plan.send_raw_transaction_request(3, &signed),
            JsonRpcRequest::eth_send_raw_transaction(3, &signed)
        );

        let hash = TxHash::new(B256::repeat_byte(0x66));
        let confirmation_plan = plan.confirmation_plan(hash);
        assert_eq!(confirmation_plan.transaction_hash, hash);
        assert_eq!(confirmation_plan.policy, policy);
        assert_eq!(confirmation_plan.submitted_at_block, Some(100));
    }

    fn word(value: u128) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[16..].copy_from_slice(&value.to_be_bytes());
        out
    }

    fn address_word(value: Address) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[12..].copy_from_slice(value.as_slice());
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

    fn tx_request(to: Address, data: &str, value: &str) -> UnsignedTxRequest {
        UnsignedTxRequest {
            from: None,
            to,
            data: data.to_owned(),
            value: value.to_owned(),
            nonce: None,
            gas: None,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: None,
        }
    }

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }
}
