//! External-signer boundary types.
//!
//! Tangent intentionally keeps signing backends out of the minimal typed-data
//! core. These types let callers prepare the exact digest to sign, attach the
//! 65-byte EVM signature returned by a wallet service, and pass a single typed
//! payload to future RPC submission helpers.

use alloy_primitives::{keccak256, Address, B256};
use serde::{Deserialize, Serialize};

use crate::tx::{SignedRawTransaction, TxSubmissionPlan, UnsignedTxRequest};
use crate::{DomainSeparatorInput, Order};

/// Caller-provided order signer.
///
/// Implement this for a local wallet, KMS wrapper, Circle Dev Wallet adapter,
/// relayer, or test double. The SDK owns the EIP-712 digest and signature
/// validation shape; signer implementations own credentials and transport.
pub trait OrderSigner {
    type Error;

    fn sign_order(&mut self, request: &OrderSigningRequest) -> Result<OrderSignature, Self::Error>;
}

/// Caller-provided raw transaction signer.
///
/// The SDK does not RLP/EIP-2718 encode transactions. Implementations can map
/// the prepared JSON-RPC transaction envelope into their own transaction type,
/// sign it, and return the final raw bytes.
pub trait RawTransactionSigner {
    type Error;

    fn sign_transaction(
        &mut self,
        request: &RawTransactionSigningRequest,
    ) -> Result<SignedRawTransaction, Self::Error>;
}

/// Caller-provided external signing client.
///
/// Implement this for a Circle Dev Wallet, KMS, relayer, or test transport that
/// accepts the SDK's serializable signing envelope and returns a typed response.
pub trait ExternalSigningClient {
    type Error;

    fn sign_external(
        &mut self,
        request: &ExternalSigningRequest,
    ) -> Result<ExternalSigningResponse, Self::Error>;
}

/// Supported signer backend categories for transport-neutral configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignerBackendKind {
    Local,
    Kms,
    CircleDevWallet,
    Relayer,
    Test,
}

/// One signer backend metadata entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignerBackendMetadata {
    pub key: String,
    pub value: String,
    pub secret: bool,
}

/// Transport-neutral signer backend configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignerBackendConfig {
    pub kind: SignerBackendKind,
    pub key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,
    #[serde(default)]
    pub metadata: Vec<SignerBackendMetadata>,
}

/// Secret-free signer backend summary for logs, queues, and support bundles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignerBackendReport {
    pub kind: SignerBackendKind,
    pub key_id: String,
    pub address: Option<Address>,
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

/// Signer-friendly order payload with hex digests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderSigningRequest {
    pub order: Order,
    pub domain: DomainSeparatorInput,
    pub digest: String,
    pub domain_separator: String,
    pub order_hash: String,
}

/// Signer-friendly raw transaction payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTransactionSigningRequest {
    pub transaction: UnsignedTxRequest,
}

/// Transport-neutral signing payload for a wallet, KMS, relayer, or test adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ExternalSigningPayload {
    Order(OrderSigningRequest),
    RawTransaction(RawTransactionSigningRequest),
}

/// Correlated signing request for an external signer service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalSigningRequest {
    pub request_id: String,
    pub backend: SignerBackendConfig,
    pub payload: ExternalSigningPayload,
}

/// Signing payload category for compact request reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalSigningPayloadKind {
    Order,
    RawTransaction,
}

/// Compact signing request report for logs, queues, and operator UIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalSigningRequestReport {
    pub request_id: String,
    pub backend_kind: SignerBackendKind,
    pub backend_key_id: String,
    pub backend_address: Option<Address>,
    #[serde(default)]
    pub backend_has_address: bool,
    pub payload_kind: ExternalSigningPayloadKind,
    #[serde(default)]
    pub is_order_request: bool,
    #[serde(default)]
    pub is_raw_transaction_request: bool,
    pub order_hash: Option<String>,
    pub order_digest: Option<String>,
    pub order_account_id: Option<u128>,
    pub order_market_id: Option<u128>,
    pub order_is_buy: Option<bool>,
    pub order_limit_price: Option<u128>,
    pub order_size: Option<u128>,
    pub order_nonce: Option<u128>,
    pub order_expiry: Option<u64>,
    pub order_reduce_only: Option<bool>,
    pub transaction_from: Option<Address>,
    pub transaction_to: Option<Address>,
    pub transaction_value: Option<String>,
    pub transaction_nonce: Option<String>,
    pub transaction_gas: Option<String>,
    pub transaction_gas_price: Option<String>,
    pub transaction_max_fee_per_gas: Option<String>,
    pub transaction_max_priority_fee_per_gas: Option<String>,
    pub transaction_chain_id: Option<String>,
    #[serde(default)]
    pub transaction_has_sender: bool,
    #[serde(default)]
    pub transaction_has_nonce: bool,
    #[serde(default)]
    pub transaction_has_gas: bool,
    #[serde(default)]
    pub transaction_has_any_fee: bool,
    #[serde(default)]
    pub transaction_has_chain_id: bool,
    #[serde(default)]
    pub transaction_has_selector: bool,
    pub transaction_uses_legacy_gas_price: bool,
    pub transaction_uses_eip1559_fees: bool,
    pub transaction_selector: Option<String>,
    pub transaction_calldata_bytes: Option<usize>,
}

/// Adapter that implements SDK signer traits through an external signing client.
#[derive(Debug, Clone)]
pub struct ExternalSignerAdapter<C> {
    client: C,
    backend: SignerBackendConfig,
    request_id_prefix: String,
    next_sequence: u64,
}

/// Typed external signer response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ExternalSigningResponse {
    Order(OrderSignature),
    RawTransaction(SignedRawTransaction),
}

/// Compact signing response report for logs, queues, and operator UIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalSigningResponseReport {
    pub payload_kind: ExternalSigningPayloadKind,
    #[serde(default)]
    pub is_order_signature: bool,
    #[serde(default)]
    pub is_raw_transaction: bool,
    pub order_signature_hex: Option<String>,
    pub order_signature_bytes: Option<usize>,
    #[serde(default)]
    pub has_order_signature_hex: bool,
    #[serde(default)]
    pub has_order_signature_bytes: bool,
    pub raw_transaction_bytes: Option<usize>,
    pub raw_transaction_hash: Option<String>,
    #[serde(default)]
    pub has_raw_transaction_bytes: bool,
    #[serde(default)]
    pub has_raw_transaction_hash: bool,
}

/// An order plus its EIP-712 domain and final signing digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedOrder {
    pub order: Order,
    pub domain: DomainSeparatorInput,
    pub digest: B256,
}

/// Errors that can occur while accepting signer backend configuration.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SignerBackendConfigError {
    #[error("signer backend key id must not be empty")]
    EmptyKeyId,
    #[error("signer metadata key must not be empty")]
    EmptyMetadataKey,
    #[error("external signing request id must not be empty")]
    EmptyRequestId,
}

/// Errors surfaced by [`ExternalSignerAdapter`].
#[derive(Debug, thiserror::Error)]
pub enum ExternalSignerAdapterError<ClientError> {
    #[error(transparent)]
    Config(#[from] SignerBackendConfigError),
    #[error("external signer request id sequence overflowed")]
    RequestIdOverflow,
    #[error("external signer returned an order signature for a raw transaction request")]
    UnexpectedOrderResponse,
    #[error("external signer returned a raw transaction for an order request")]
    UnexpectedRawTransactionResponse,
    #[error("external signing client failed")]
    Client(ClientError),
}

impl SignerBackendConfig {
    pub fn new(
        kind: SignerBackendKind,
        key_id: impl Into<String>,
    ) -> Result<Self, SignerBackendConfigError> {
        let config = Self {
            kind,
            key_id: key_id.into(),
            address: None,
            metadata: Vec::new(),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn with_address(mut self, address: Address) -> Self {
        self.address = Some(address);
        self
    }

    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
        secret: bool,
    ) -> Result<Self, SignerBackendConfigError> {
        let metadata = SignerBackendMetadata {
            key: key.into(),
            value: value.into(),
            secret,
        };
        metadata.validate()?;
        self.metadata.push(metadata);
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), SignerBackendConfigError> {
        if self.key_id.trim().is_empty() {
            return Err(SignerBackendConfigError::EmptyKeyId);
        }
        for metadata in &self.metadata {
            metadata.validate()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn redacted(&self) -> Self {
        Self {
            kind: self.kind,
            key_id: self.key_id.clone(),
            address: self.address,
            metadata: self
                .metadata
                .iter()
                .map(SignerBackendMetadata::redacted)
                .collect(),
        }
    }

    #[must_use]
    pub fn report(&self) -> SignerBackendReport {
        let metadata_keys = self
            .metadata
            .iter()
            .map(|metadata| metadata.key.clone())
            .collect::<Vec<_>>();
        let secret_metadata_keys = self
            .metadata
            .iter()
            .filter(|metadata| metadata.secret)
            .map(|metadata| metadata.key.clone())
            .collect::<Vec<_>>();

        SignerBackendReport {
            kind: self.kind,
            key_id: self.key_id.clone(),
            address: self.address,
            metadata_count: metadata_keys.len(),
            secret_metadata_count: secret_metadata_keys.len(),
            has_address: self.address.is_some(),
            has_metadata: !metadata_keys.is_empty(),
            has_secret_metadata: !secret_metadata_keys.is_empty(),
            metadata_keys,
            secret_metadata_keys,
        }
    }
}

impl<C> ExternalSignerAdapter<C> {
    pub fn new(backend: SignerBackendConfig, client: C) -> Result<Self, SignerBackendConfigError> {
        backend.validate()?;
        Ok(Self {
            client,
            backend,
            request_id_prefix: "sign".to_owned(),
            next_sequence: 1,
        })
    }

    pub fn with_request_id_prefix(
        mut self,
        request_id_prefix: impl Into<String>,
    ) -> Result<Self, SignerBackendConfigError> {
        let request_id_prefix = request_id_prefix.into();
        if request_id_prefix.trim().is_empty() {
            return Err(SignerBackendConfigError::EmptyRequestId);
        }
        self.request_id_prefix = request_id_prefix;
        Ok(self)
    }

    #[must_use]
    pub const fn client(&self) -> &C {
        &self.client
    }

    #[must_use]
    pub fn client_mut(&mut self) -> &mut C {
        &mut self.client
    }

    #[must_use]
    pub const fn backend(&self) -> &SignerBackendConfig {
        &self.backend
    }

    #[must_use]
    pub fn into_parts(self) -> (SignerBackendConfig, C) {
        (self.backend, self.client)
    }

    fn next_request_id(
        &mut self,
        payload_kind: &str,
    ) -> Result<String, ExternalSignerAdapterError<C::Error>>
    where
        C: ExternalSigningClient,
    {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(ExternalSignerAdapterError::RequestIdOverflow)?;
        Ok(format!(
            "{}-{}-{}",
            self.request_id_prefix, payload_kind, sequence
        ))
    }
}

impl<C: ExternalSigningClient> OrderSigner for ExternalSignerAdapter<C> {
    type Error = ExternalSignerAdapterError<C::Error>;

    fn sign_order(&mut self, request: &OrderSigningRequest) -> Result<OrderSignature, Self::Error> {
        let external = ExternalSigningRequest::order(
            self.next_request_id("order")?,
            self.backend.clone(),
            request.clone(),
        )?;
        match self
            .client
            .sign_external(&external)
            .map_err(ExternalSignerAdapterError::Client)?
        {
            ExternalSigningResponse::Order(signature) => Ok(signature),
            ExternalSigningResponse::RawTransaction(_) => {
                Err(ExternalSignerAdapterError::UnexpectedRawTransactionResponse)
            }
        }
    }
}

impl<C: ExternalSigningClient> RawTransactionSigner for ExternalSignerAdapter<C> {
    type Error = ExternalSignerAdapterError<C::Error>;

    fn sign_transaction(
        &mut self,
        request: &RawTransactionSigningRequest,
    ) -> Result<SignedRawTransaction, Self::Error> {
        let external = ExternalSigningRequest::raw_transaction(
            self.next_request_id("tx")?,
            self.backend.clone(),
            request.clone(),
        )?;
        match self
            .client
            .sign_external(&external)
            .map_err(ExternalSignerAdapterError::Client)?
        {
            ExternalSigningResponse::RawTransaction(signed) => Ok(signed),
            ExternalSigningResponse::Order(_) => {
                Err(ExternalSignerAdapterError::UnexpectedOrderResponse)
            }
        }
    }
}

impl SignerBackendMetadata {
    pub fn validate(&self) -> Result<(), SignerBackendConfigError> {
        if self.key.trim().is_empty() {
            return Err(SignerBackendConfigError::EmptyMetadataKey);
        }
        Ok(())
    }

    #[must_use]
    pub fn redacted(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: if self.secret {
                "<redacted>".to_owned()
            } else {
                self.value.clone()
            },
            secret: self.secret,
        }
    }
}

impl ExternalSigningRequest {
    pub fn order(
        request_id: impl Into<String>,
        backend: SignerBackendConfig,
        request: OrderSigningRequest,
    ) -> Result<Self, SignerBackendConfigError> {
        Self::new(request_id, backend, ExternalSigningPayload::Order(request))
    }

    pub fn raw_transaction(
        request_id: impl Into<String>,
        backend: SignerBackendConfig,
        request: RawTransactionSigningRequest,
    ) -> Result<Self, SignerBackendConfigError> {
        Self::new(
            request_id,
            backend,
            ExternalSigningPayload::RawTransaction(request),
        )
    }

    pub fn new(
        request_id: impl Into<String>,
        backend: SignerBackendConfig,
        payload: ExternalSigningPayload,
    ) -> Result<Self, SignerBackendConfigError> {
        let request = Self {
            request_id: request_id.into(),
            backend,
            payload,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), SignerBackendConfigError> {
        if self.request_id.trim().is_empty() {
            return Err(SignerBackendConfigError::EmptyRequestId);
        }
        self.backend.validate()
    }

    #[must_use]
    pub fn redacted(&self) -> Self {
        Self {
            request_id: self.request_id.clone(),
            backend: self.backend.redacted(),
            payload: self.payload.clone(),
        }
    }

    #[must_use]
    pub fn report(&self) -> ExternalSigningRequestReport {
        let mut report = ExternalSigningRequestReport {
            request_id: self.request_id.clone(),
            backend_kind: self.backend.kind,
            backend_key_id: self.backend.key_id.clone(),
            backend_address: self.backend.address,
            backend_has_address: self.backend.address.is_some(),
            payload_kind: self.payload.kind(),
            is_order_request: matches!(self.payload, ExternalSigningPayload::Order(_)),
            is_raw_transaction_request: matches!(
                self.payload,
                ExternalSigningPayload::RawTransaction(_)
            ),
            order_hash: None,
            order_digest: None,
            order_account_id: None,
            order_market_id: None,
            order_is_buy: None,
            order_limit_price: None,
            order_size: None,
            order_nonce: None,
            order_expiry: None,
            order_reduce_only: None,
            transaction_from: None,
            transaction_to: None,
            transaction_value: None,
            transaction_nonce: None,
            transaction_gas: None,
            transaction_gas_price: None,
            transaction_max_fee_per_gas: None,
            transaction_max_priority_fee_per_gas: None,
            transaction_chain_id: None,
            transaction_has_sender: false,
            transaction_has_nonce: false,
            transaction_has_gas: false,
            transaction_has_any_fee: false,
            transaction_has_chain_id: false,
            transaction_has_selector: false,
            transaction_uses_legacy_gas_price: false,
            transaction_uses_eip1559_fees: false,
            transaction_selector: None,
            transaction_calldata_bytes: None,
        };

        match &self.payload {
            ExternalSigningPayload::Order(request) => {
                report.order_hash = Some(request.order_hash.clone());
                report.order_digest = Some(request.digest.clone());
                report.order_account_id = Some(request.order.account_id);
                report.order_market_id = Some(request.order.market_id);
                report.order_is_buy = Some(request.order.is_buy);
                report.order_limit_price = Some(request.order.limit_price);
                report.order_size = Some(request.order.size);
                report.order_nonce = Some(request.order.nonce);
                report.order_expiry = Some(request.order.expiry);
                report.order_reduce_only = Some(request.order.reduce_only);
            }
            ExternalSigningPayload::RawTransaction(request) => {
                report.transaction_from = request.transaction.from;
                report.transaction_to = Some(request.transaction.to);
                report.transaction_value = Some(request.transaction.value.clone());
                report.transaction_nonce = request.transaction.nonce.clone();
                report.transaction_gas = request.transaction.gas.clone();
                report.transaction_gas_price = request.transaction.gas_price.clone();
                report.transaction_max_fee_per_gas = request.transaction.max_fee_per_gas.clone();
                report.transaction_max_priority_fee_per_gas =
                    request.transaction.max_priority_fee_per_gas.clone();
                report.transaction_chain_id = request.transaction.chain_id.clone();
                report.transaction_has_sender = request.transaction.from.is_some();
                report.transaction_has_nonce = request.transaction.nonce.is_some();
                report.transaction_has_gas = request.transaction.gas.is_some();
                report.transaction_uses_legacy_gas_price = request.transaction.gas_price.is_some();
                report.transaction_uses_eip1559_fees =
                    request.transaction.max_fee_per_gas.is_some()
                        || request.transaction.max_priority_fee_per_gas.is_some();
                report.transaction_has_any_fee = report.transaction_uses_legacy_gas_price
                    || report.transaction_uses_eip1559_fees;
                report.transaction_has_chain_id = request.transaction.chain_id.is_some();
                let calldata = decode_tx_data(&request.transaction.data);
                report.transaction_selector = calldata
                    .as_ref()
                    .and_then(|data| data.get(..4))
                    .map(|selector| format!("0x{}", hex::encode(selector)));
                report.transaction_calldata_bytes = calldata.as_ref().map(Vec::len);
                report.transaction_has_selector = report.transaction_selector.is_some();
            }
        }

        report
    }
}

impl ExternalSigningPayload {
    #[must_use]
    pub const fn kind(&self) -> ExternalSigningPayloadKind {
        match self {
            Self::Order(_) => ExternalSigningPayloadKind::Order,
            Self::RawTransaction(_) => ExternalSigningPayloadKind::RawTransaction,
        }
    }
}

impl ExternalSigningResponse {
    #[must_use]
    pub fn report(&self) -> ExternalSigningResponseReport {
        match self {
            Self::Order(signature) => ExternalSigningResponseReport {
                payload_kind: ExternalSigningPayloadKind::Order,
                is_order_signature: true,
                is_raw_transaction: false,
                order_signature_hex: Some(signature.to_hex()),
                order_signature_bytes: Some(OrderSignature::LEN),
                has_order_signature_hex: true,
                has_order_signature_bytes: true,
                raw_transaction_bytes: None,
                raw_transaction_hash: None,
                has_raw_transaction_bytes: false,
                has_raw_transaction_hash: false,
            },
            Self::RawTransaction(signed_transaction) => {
                let hash = keccak256(signed_transaction.as_bytes());
                ExternalSigningResponseReport {
                    payload_kind: ExternalSigningPayloadKind::RawTransaction,
                    is_order_signature: false,
                    is_raw_transaction: true,
                    order_signature_hex: None,
                    order_signature_bytes: None,
                    has_order_signature_hex: false,
                    has_order_signature_bytes: false,
                    raw_transaction_bytes: Some(signed_transaction.len()),
                    raw_transaction_hash: Some(format!("0x{}", hex::encode(hash))),
                    has_raw_transaction_bytes: true,
                    has_raw_transaction_hash: true,
                }
            }
        }
    }
}

impl PreparedOrder {
    /// Prepare an order for an external signing backend.
    #[must_use]
    pub fn new(order: Order, domain: DomainSeparatorInput) -> Self {
        let digest = order.digest(&domain);
        Self {
            order,
            domain,
            digest,
        }
    }

    /// Attach a 65-byte EVM signature to this order.
    #[must_use]
    pub fn attach_signature(self, signature: OrderSignature) -> SignedOrder {
        SignedOrder {
            order: self.order,
            signature,
        }
    }

    /// Build a serializable payload for wallet/KMS/relayer signing backends.
    #[must_use]
    pub fn signing_request(&self) -> OrderSigningRequest {
        OrderSigningRequest {
            order: self.order.clone(),
            domain: self.domain.clone(),
            digest: self.digest_hex(),
            domain_separator: self.domain_separator_hex(),
            order_hash: format!("0x{}", hex::encode(self.order.order_hash())),
        }
    }

    /// Build a correlated external signing request for this order.
    pub fn external_signing_request(
        &self,
        request_id: impl Into<String>,
        backend: SignerBackendConfig,
    ) -> Result<ExternalSigningRequest, SignerBackendConfigError> {
        ExternalSigningRequest::order(request_id, backend, self.signing_request())
    }

    /// Sign this prepared order with a caller-provided signer.
    pub fn sign_with<S: OrderSigner>(&self, signer: &mut S) -> Result<SignedOrder, S::Error> {
        let signature = signer.sign_order(&self.signing_request())?;
        Ok(self.clone().attach_signature(signature))
    }

    /// Hex-encode the final EIP-712 digest with a `0x` prefix.
    #[must_use]
    pub fn digest_hex(&self) -> String {
        format!("0x{}", hex::encode(self.digest))
    }

    /// Hex-encode the EIP-712 domain separator with a `0x` prefix.
    #[must_use]
    pub fn domain_separator_hex(&self) -> String {
        format!("0x{}", hex::encode(self.domain.separator()))
    }
}

impl RawTransactionSigningRequest {
    #[must_use]
    pub fn new(transaction: UnsignedTxRequest) -> Self {
        Self { transaction }
    }
}

impl TxSubmissionPlan {
    /// Build a serializable payload for a raw transaction signing backend.
    #[must_use]
    pub fn raw_transaction_signing_request(&self) -> RawTransactionSigningRequest {
        RawTransactionSigningRequest::new(self.request.clone())
    }

    /// Build a correlated external raw-transaction signing request.
    pub fn external_raw_transaction_signing_request(
        &self,
        request_id: impl Into<String>,
        backend: SignerBackendConfig,
    ) -> Result<ExternalSigningRequest, SignerBackendConfigError> {
        ExternalSigningRequest::raw_transaction(
            request_id,
            backend,
            self.raw_transaction_signing_request(),
        )
    }

    /// Sign this submission plan with a caller-provided raw transaction signer.
    pub fn sign_with<S: RawTransactionSigner>(
        &self,
        signer: &mut S,
    ) -> Result<SignedRawTransaction, S::Error> {
        signer.sign_transaction(&self.raw_transaction_signing_request())
    }
}

/// A signed Tangent order ready for `OrderBook.submitOrder(order, signature)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedOrder {
    pub order: Order,
    pub signature: OrderSignature,
}

impl SignedOrder {
    /// Solidity function signature for `IOrderBook.submitOrder`.
    pub const SUBMIT_ORDER_SIGNATURE: &'static str =
        "submitOrder((uint256,uint256,bool,uint256,uint256,uint256,uint256,bool),bytes)";

    /// Return the on-chain order hash for this signed payload.
    #[must_use]
    pub fn order_hash(&self) -> B256 {
        self.order.order_hash()
    }

    /// Hex-encode the on-chain order hash with a `0x` prefix.
    #[must_use]
    pub fn order_hash_hex(&self) -> String {
        format!("0x{}", hex::encode(self.order_hash()))
    }

    /// Compute the 4-byte selector for `submitOrder(Order,bytes)`.
    #[must_use]
    pub fn submit_order_selector() -> [u8; 4] {
        let hash = keccak256(Self::SUBMIT_ORDER_SIGNATURE.as_bytes());
        [hash[0], hash[1], hash[2], hash[3]]
    }

    /// ABI-encode `OrderBook.submitOrder(order, signature)` calldata.
    ///
    /// This does not submit a transaction. It produces the exact calldata a
    /// caller can pass into their own transaction builder once an OrderBook
    /// deployment is known.
    #[must_use]
    pub fn submit_order_calldata(&self) -> Vec<u8> {
        const ORDER_WORDS: usize = 8;
        const SIGNATURE_OFFSET_WORDS: usize = ORDER_WORDS + 1;

        let mut out = Vec::with_capacity(4 + 32 * (SIGNATURE_OFFSET_WORDS + 1 + 3));
        out.extend_from_slice(&Self::submit_order_selector());

        crate::eip712::encode_u128(&mut out, self.order.account_id);
        crate::eip712::encode_u128(&mut out, self.order.market_id);
        crate::eip712::encode_bool(&mut out, self.order.is_buy);
        crate::eip712::encode_u128(&mut out, self.order.limit_price);
        crate::eip712::encode_u128(&mut out, self.order.size);
        crate::eip712::encode_u128(&mut out, self.order.nonce);
        crate::eip712::encode_u64(&mut out, self.order.expiry);
        crate::eip712::encode_bool(&mut out, self.order.reduce_only);
        crate::eip712::encode_u128(&mut out, (SIGNATURE_OFFSET_WORDS * 32) as u128);
        crate::eip712::encode_dynamic_bytes(&mut out, self.signature.as_bytes());
        out
    }

    /// Hex-encode `submit_order_calldata()` with a `0x` prefix.
    #[must_use]
    pub fn submit_order_calldata_hex(&self) -> String {
        format!("0x{}", hex::encode(self.submit_order_calldata()))
    }
}

fn decode_tx_data(data: &str) -> Option<Vec<u8>> {
    hex::decode(
        data.strip_prefix("0x")
            .or_else(|| data.strip_prefix("0X"))
            .unwrap_or(data),
    )
    .ok()
}

/// A canonical EVM order signature: `r || s || v`, exactly 65 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderSignature(#[serde(with = "signature_bytes")] pub [u8; Self::LEN]);

impl OrderSignature {
    pub const LEN: usize = 65;

    /// Construct from raw signature bytes.
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, SignatureError> {
        let bytes = bytes.as_ref();
        if bytes.len() != Self::LEN {
            return Err(SignatureError::InvalidLength {
                actual: bytes.len(),
            });
        }

        let mut signature = [0u8; Self::LEN];
        signature.copy_from_slice(bytes);
        Ok(Self(signature))
    }

    /// Parse a hex signature with or without a `0x` prefix.
    pub fn from_hex(input: &str) -> Result<Self, SignatureError> {
        let trimmed = input
            .strip_prefix("0x")
            .or_else(|| input.strip_prefix("0X"))
            .unwrap_or(input);
        let bytes = hex::decode(trimmed).map_err(SignatureError::Hex)?;
        Self::from_bytes(bytes)
    }

    /// Borrow the raw `r || s || v` bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Self::LEN] {
        &self.0
    }

    /// Hex-encode with a `0x` prefix.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("0x{}", hex::encode(self.0))
    }
}

/// Errors that can occur while accepting external signatures.
#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("invalid signature length: expected 65 bytes, got {actual}")]
    InvalidLength { actual: usize },
    #[error("invalid hex signature: {0}")]
    Hex(hex::FromHexError),
}

mod signature_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::OrderSignature;

    pub fn serialize<S>(bytes: &[u8; OrderSignature::LEN], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(bytes)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; OrderSignature::LEN], D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let signature = OrderSignature::from_hex(&encoded).map_err(serde::de::Error::custom)?;
        Ok(signature.0)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;
    use std::collections::VecDeque;

    use super::*;
    use crate::{TxConfirmationPolicy, BASE_SCALE, PRICE_SCALE};

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

    #[test]
    fn prepared_order_carries_frozen_digest() {
        let prepared = PreparedOrder::new(order(), DomainSeparatorInput::new(11111, Address::ZERO));

        assert_eq!(
            hex::encode(prepared.digest),
            "28e8b0b1104d7872301ab044c7b2106a4df3759a110949d6658cf7a704a79447"
        );
        assert_eq!(
            prepared.digest_hex(),
            "0x28e8b0b1104d7872301ab044c7b2106a4df3759a110949d6658cf7a704a79447"
        );
        assert_eq!(
            prepared.domain_separator_hex(),
            "0x7a56aaa9c62a007bd4ad2bb83215db0d7bbebadab42d61484a18d062e9f99a72"
        );
    }

    #[test]
    fn signature_hex_roundtrips_with_prefix() {
        let signature = OrderSignature::from_bytes([1u8; OrderSignature::LEN]).expect("valid");
        let encoded = signature.to_hex();
        let decoded = OrderSignature::from_hex(&encoded).expect("valid hex");
        let decoded_upper_prefix =
            OrderSignature::from_hex(&encoded.replacen("0x", "0X", 1)).expect("valid hex");

        assert_eq!(signature, decoded);
        assert_eq!(signature, decoded_upper_prefix);
    }

    #[test]
    fn signature_rejects_bad_length() {
        let err = OrderSignature::from_bytes([1u8; 64]).expect_err("bad length");
        assert!(matches!(err, SignatureError::InvalidLength { actual: 64 }));
    }

    #[test]
    fn signer_backend_config_validates_and_redacts_metadata() {
        assert_eq!(
            SignerBackendConfig::new(SignerBackendKind::CircleDevWallet, ""),
            Err(SignerBackendConfigError::EmptyKeyId)
        );

        let backend = SignerBackendConfig::new(SignerBackendKind::CircleDevWallet, "wallet-1")
            .expect("backend")
            .with_address(Address::repeat_byte(0x44))
            .with_metadata("entity", "entity-id", false)
            .expect("metadata")
            .with_metadata("api-key", "secret", true)
            .expect("secret metadata");

        assert_eq!(backend.address, Some(Address::repeat_byte(0x44)));
        assert_eq!(backend.metadata.len(), 2);
        assert_eq!(backend.redacted().metadata[0].value, "entity-id");
        assert_eq!(backend.redacted().metadata[1].value, "<redacted>");
        let report = backend.report();
        assert_eq!(report.kind, SignerBackendKind::CircleDevWallet);
        assert_eq!(report.key_id, "wallet-1");
        assert_eq!(report.address, Some(Address::repeat_byte(0x44)));
        assert_eq!(report.metadata_keys, vec!["entity", "api-key"]);
        assert_eq!(report.secret_metadata_keys, vec!["api-key"]);
        assert_eq!(report.metadata_count, 2);
        assert_eq!(report.secret_metadata_count, 1);
        assert!(report.has_address);
        assert!(report.has_metadata);
        assert!(report.has_secret_metadata);
        let report_json = serde_json::to_string(&report).expect("report serializes");
        assert!(!report_json.contains("entity-id"));
        assert!(!report_json.contains("\":\"secret\""));
        let restored_report: SignerBackendReport =
            serde_json::from_str(&report_json).expect("report deserializes");
        assert_eq!(restored_report, report);
        let mut legacy_report_json =
            serde_json::to_value(&report).expect("report value serializes");
        let legacy_report_object = legacy_report_json
            .as_object_mut()
            .expect("signer backend report object");
        legacy_report_object.remove("has_address");
        legacy_report_object.remove("has_metadata");
        legacy_report_object.remove("has_secret_metadata");
        let legacy_report: SignerBackendReport =
            serde_json::from_value(legacy_report_json).expect("legacy report deserializes");
        assert!(!legacy_report.has_address);
        assert!(!legacy_report.has_metadata);
        assert!(!legacy_report.has_secret_metadata);
        assert_eq!(
            backend.clone().with_metadata("", "value", false),
            Err(SignerBackendConfigError::EmptyMetadataKey)
        );
    }

    #[test]
    fn signed_order_serde_uses_hex_signature() {
        let signature = OrderSignature::from_bytes([1u8; OrderSignature::LEN]).expect("valid");
        let signed = PreparedOrder::new(order(), DomainSeparatorInput::new(11111, Address::ZERO))
            .attach_signature(signature);

        let json = serde_json::to_string(&signed).expect("serialize");
        assert!(json.contains("\"signature\":\"0x010101"));
        let decoded: SignedOrder = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, signed);
    }

    #[test]
    fn external_order_signing_request_wraps_signer_payload() {
        let prepared = PreparedOrder::new(order(), DomainSeparatorInput::new(11111, Address::ZERO));
        let backend = SignerBackendConfig::new(SignerBackendKind::Kms, "key-1").expect("backend");

        let external = prepared
            .external_signing_request("order-1", backend.clone())
            .expect("external signing request");

        assert_eq!(external.request_id, "order-1");
        assert_eq!(external.backend, backend);
        assert!(matches!(
            &external.payload,
            ExternalSigningPayload::Order(OrderSigningRequest { .. })
        ));
        let report = external.report();
        assert_eq!(report.request_id, "order-1");
        assert_eq!(report.backend_kind, SignerBackendKind::Kms);
        assert_eq!(report.backend_key_id, "key-1");
        assert_eq!(report.backend_address, None);
        assert!(!report.backend_has_address);
        assert_eq!(report.payload_kind, ExternalSigningPayloadKind::Order);
        assert!(report.is_order_request);
        assert!(!report.is_raw_transaction_request);
        assert_eq!(
            report.order_hash,
            Some(prepared.signing_request().order_hash)
        );
        assert_eq!(report.order_digest, Some(prepared.digest_hex()));
        assert_eq!(report.order_account_id, Some(prepared.order.account_id));
        assert_eq!(report.order_market_id, Some(prepared.order.market_id));
        assert_eq!(report.order_is_buy, Some(prepared.order.is_buy));
        assert_eq!(report.order_limit_price, Some(prepared.order.limit_price));
        assert_eq!(report.order_size, Some(prepared.order.size));
        assert_eq!(report.order_nonce, Some(prepared.order.nonce));
        assert_eq!(report.order_expiry, Some(prepared.order.expiry));
        assert_eq!(report.order_reduce_only, Some(prepared.order.reduce_only));
        assert_eq!(report.transaction_to, None);
        assert!(!report.transaction_has_sender);
        assert!(!report.transaction_has_nonce);
        assert!(!report.transaction_has_gas);
        assert!(!report.transaction_has_any_fee);
        assert!(!report.transaction_has_chain_id);
        assert!(!report.transaction_has_selector);
        let report_json = serde_json::to_string(&report).expect("report serializes");
        assert!(report_json.contains("\"order_account_id\":7"));
        assert!(report_json.contains("\"order_is_buy\":true"));
        assert!(report_json.contains("\"is_order_request\":true"));
        assert!(report_json.contains("\"is_raw_transaction_request\":false"));
        let restored_report: ExternalSigningRequestReport =
            serde_json::from_str(&report_json).expect("report deserializes");
        assert_eq!(restored_report, report);
        let legacy_report_json = report_json
            .replace("\"backend_has_address\":false,", "")
            .replace("\"is_order_request\":true,", "")
            .replace("\"is_raw_transaction_request\":false,", "")
            .replace("\"transaction_has_sender\":false,", "")
            .replace("\"transaction_has_nonce\":false,", "")
            .replace("\"transaction_has_gas\":false,", "")
            .replace("\"transaction_has_any_fee\":false,", "")
            .replace("\"transaction_has_chain_id\":false,", "")
            .replace("\"transaction_has_selector\":false,", "");
        let restored_legacy_report: ExternalSigningRequestReport =
            serde_json::from_str(&legacy_report_json).expect("legacy report deserializes");
        assert!(!restored_legacy_report.backend_has_address);
        assert!(!restored_legacy_report.is_order_request);
        assert!(!restored_legacy_report.is_raw_transaction_request);
        assert!(!restored_legacy_report.transaction_has_sender);
        assert!(!restored_legacy_report.transaction_has_nonce);
        assert!(!restored_legacy_report.transaction_has_gas);
        assert!(!restored_legacy_report.transaction_has_any_fee);
        assert!(!restored_legacy_report.transaction_has_chain_id);
        assert!(!restored_legacy_report.transaction_has_selector);
        assert_eq!(
            ExternalSigningRequest::order("", backend, prepared.signing_request()),
            Err(SignerBackendConfigError::EmptyRequestId)
        );

        let json = serde_json::to_string(&external).expect("serialize external request");
        let decoded: ExternalSigningRequest =
            serde_json::from_str(&json).expect("decode external request");
        assert_eq!(decoded, external);
    }

    #[derive(Debug, Default)]
    struct MockExternalSigningClient {
        responses: VecDeque<ExternalSigningResponse>,
        seen: Vec<ExternalSigningRequest>,
    }

    impl MockExternalSigningClient {
        fn new(responses: impl IntoIterator<Item = ExternalSigningResponse>) -> Self {
            Self {
                responses: responses.into_iter().collect(),
                seen: Vec::new(),
            }
        }
    }

    impl ExternalSigningClient for MockExternalSigningClient {
        type Error = &'static str;

        fn sign_external(
            &mut self,
            request: &ExternalSigningRequest,
        ) -> Result<ExternalSigningResponse, Self::Error> {
            self.seen.push(request.clone());
            self.responses.pop_front().ok_or("missing response")
        }
    }

    #[test]
    fn external_signer_adapter_signs_orders_with_correlated_requests() {
        let prepared = PreparedOrder::new(order(), DomainSeparatorInput::new(11111, Address::ZERO));
        let signature = OrderSignature::from_bytes([3u8; OrderSignature::LEN]).expect("signature");
        let response_report = ExternalSigningResponse::Order(signature).report();
        assert_eq!(
            response_report.payload_kind,
            ExternalSigningPayloadKind::Order
        );
        assert!(response_report.is_order_signature);
        assert!(!response_report.is_raw_transaction);
        assert_eq!(
            response_report.order_signature_hex.as_deref(),
            Some(signature.to_hex().as_str())
        );
        assert_eq!(
            response_report.order_signature_bytes,
            Some(OrderSignature::LEN)
        );
        assert!(response_report.has_order_signature_hex);
        assert!(response_report.has_order_signature_bytes);
        assert_eq!(response_report.raw_transaction_bytes, None);
        assert!(!response_report.has_raw_transaction_bytes);
        assert!(!response_report.has_raw_transaction_hash);
        let response_report_json =
            serde_json::to_string(&response_report).expect("response report serializes");
        assert!(response_report_json.contains("\"is_order_signature\":true"));
        assert!(response_report_json.contains("\"has_order_signature_hex\":true"));
        assert!(response_report_json.contains("\"is_raw_transaction\":false"));
        let restored_response_report: ExternalSigningResponseReport =
            serde_json::from_str(&response_report_json).expect("response report deserializes");
        assert_eq!(restored_response_report, response_report);
        let mut legacy_response_report_json =
            serde_json::to_value(&response_report).expect("response report value");
        let legacy_response_report_object = legacy_response_report_json
            .as_object_mut()
            .expect("response report object");
        legacy_response_report_object.remove("is_order_signature");
        legacy_response_report_object.remove("is_raw_transaction");
        legacy_response_report_object.remove("has_order_signature_hex");
        legacy_response_report_object.remove("has_order_signature_bytes");
        legacy_response_report_object.remove("has_raw_transaction_bytes");
        legacy_response_report_object.remove("has_raw_transaction_hash");
        let restored_legacy_response_report: ExternalSigningResponseReport =
            serde_json::from_value(legacy_response_report_json)
                .expect("legacy response report deserializes");
        assert!(!restored_legacy_response_report.is_order_signature);
        assert!(!restored_legacy_response_report.is_raw_transaction);
        assert!(!restored_legacy_response_report.has_order_signature_hex);
        assert!(!restored_legacy_response_report.has_order_signature_bytes);
        assert!(!restored_legacy_response_report.has_raw_transaction_bytes);
        assert!(!restored_legacy_response_report.has_raw_transaction_hash);
        let backend = SignerBackendConfig::new(SignerBackendKind::CircleDevWallet, "wallet-1")
            .expect("backend");
        let client = MockExternalSigningClient::new([ExternalSigningResponse::Order(signature)]);
        let mut adapter = ExternalSignerAdapter::new(backend.clone(), client)
            .expect("adapter")
            .with_request_id_prefix("orderbook")
            .expect("prefix");

        let signed = prepared
            .sign_with(&mut adapter)
            .expect("external signer signs order");

        assert_eq!(signed.signature, signature);
        let (adapter_backend, client) = adapter.into_parts();
        assert_eq!(adapter_backend, backend);
        assert_eq!(client.seen.len(), 1);
        assert_eq!(client.seen[0].request_id, "orderbook-order-1");
        assert_eq!(client.seen[0].backend, backend);
        assert!(matches!(
            client.seen[0].payload,
            ExternalSigningPayload::Order(OrderSigningRequest { .. })
        ));
    }

    #[test]
    fn signed_order_calldata_has_submit_order_shape() {
        let signature = OrderSignature::from_bytes([1u8; OrderSignature::LEN]).expect("valid");
        let signed = PreparedOrder::new(order(), DomainSeparatorInput::new(11111, Address::ZERO))
            .attach_signature(signature);
        let calldata = signed.submit_order_calldata();

        assert_eq!(
            hex::encode(SignedOrder::submit_order_selector()),
            "e8357b2d"
        );
        assert_eq!(&calldata[0..4], &SignedOrder::submit_order_selector());
        assert_eq!(signed.order_hash(), signed.order.order_hash());
        assert_eq!(
            signed.order_hash_hex(),
            "0xb0b9bd99f3734201d225297621c4a3a15cbdb0c6381dc7789dc0b85d94a08cc0"
        );
        assert_eq!(calldata.len(), 420);
        assert_eq!(
            hex::encode(&calldata[4 + 8 * 32..4 + 9 * 32]),
            format!("{:064x}", 288)
        );
        assert_eq!(
            hex::encode(&calldata[4 + 9 * 32..4 + 10 * 32]),
            format!("{:064x}", 65)
        );
        assert_eq!(
            &calldata[4 + 10 * 32..4 + 10 * 32 + OrderSignature::LEN],
            signature.as_bytes()
        );
        assert!(calldata[4 + 10 * 32 + OrderSignature::LEN..]
            .iter()
            .all(|byte| *byte == 0));
    }

    #[derive(Debug, Default)]
    struct MockOrderSigner {
        seen: Vec<OrderSigningRequest>,
    }

    impl OrderSigner for MockOrderSigner {
        type Error = &'static str;

        fn sign_order(
            &mut self,
            request: &OrderSigningRequest,
        ) -> Result<OrderSignature, Self::Error> {
            self.seen.push(request.clone());
            OrderSignature::from_bytes([2u8; OrderSignature::LEN]).map_err(|_| "bad signature")
        }
    }

    #[test]
    fn prepared_order_builds_signing_request_and_signs_with_backend() {
        let prepared = PreparedOrder::new(order(), DomainSeparatorInput::new(11111, Address::ZERO));
        let request = prepared.signing_request();

        assert_eq!(request.order, prepared.order);
        assert_eq!(request.domain, prepared.domain);
        assert_eq!(request.digest, prepared.digest_hex());
        assert_eq!(request.domain_separator, prepared.domain_separator_hex());
        assert_eq!(
            request.order_hash,
            "0xb0b9bd99f3734201d225297621c4a3a15cbdb0c6381dc7789dc0b85d94a08cc0"
        );

        let mut signer = MockOrderSigner::default();
        let signed = prepared
            .sign_with(&mut signer)
            .expect("mock signer returns signature");

        assert_eq!(signer.seen, vec![request]);
        assert_eq!(signed.order, order());
        assert_eq!(
            signed.signature,
            OrderSignature::from_bytes([2u8; OrderSignature::LEN]).unwrap()
        );
    }

    #[derive(Debug, Default)]
    struct MockRawTransactionSigner {
        seen: Vec<RawTransactionSigningRequest>,
    }

    impl RawTransactionSigner for MockRawTransactionSigner {
        type Error = &'static str;

        fn sign_transaction(
            &mut self,
            request: &RawTransactionSigningRequest,
        ) -> Result<SignedRawTransaction, Self::Error> {
            self.seen.push(request.clone());
            SignedRawTransaction::from_hex("0x02abcd").map_err(|_| "bad raw transaction")
        }
    }

    #[test]
    fn transaction_submission_plan_builds_raw_signing_request() {
        let transaction = crate::UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x12, 0x34, 0x56, 0x78],
        }
        .to_tx_request_with_value_and_metadata(
            123,
            crate::TxRequestMetadata::new()
                .with_from(Address::repeat_byte(0x44))
                .with_nonce(7)
                .with_gas(250_000)
                .with_gas_price(1_000_000_000)
                .with_eip1559_fees(2_000_000_000, 1_000_000_000)
                .with_chain_id(11111),
        );
        let plan = TxSubmissionPlan::new(
            transaction.clone(),
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );
        let request = plan.raw_transaction_signing_request();

        assert_eq!(request.transaction, transaction);

        let mut signer = MockRawTransactionSigner::default();
        let signed = plan
            .sign_with(&mut signer)
            .expect("mock raw transaction signer");

        assert_eq!(signer.seen, vec![request]);
        assert_eq!(signed.to_hex(), "0x02abcd");
    }

    #[test]
    fn external_signer_adapter_signs_raw_transactions_and_rejects_wrong_payload_kind() {
        let transaction = crate::UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x12, 0x34, 0x56, 0x78],
        }
        .to_tx_request_with_value_and_metadata(
            123,
            crate::TxRequestMetadata::new()
                .with_from(Address::repeat_byte(0x44))
                .with_nonce(7)
                .with_gas(250_000)
                .with_gas_price(1_000_000_000)
                .with_eip1559_fees(2_000_000_000, 1_000_000_000)
                .with_chain_id(11111),
        );
        let plan = TxSubmissionPlan::new(
            transaction,
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );
        let signed_raw = SignedRawTransaction::from_hex("0x02abcd").expect("raw tx");
        let wrong_signature =
            OrderSignature::from_bytes([4u8; OrderSignature::LEN]).expect("signature");
        let backend =
            SignerBackendConfig::new(SignerBackendKind::Relayer, "relayer-1").expect("backend");
        let client = MockExternalSigningClient::new([
            ExternalSigningResponse::RawTransaction(signed_raw.clone()),
            ExternalSigningResponse::Order(wrong_signature),
        ]);
        let mut adapter = ExternalSignerAdapter::new(backend, client)
            .expect("adapter")
            .with_request_id_prefix("keeper")
            .expect("prefix");

        let signed = plan.sign_with(&mut adapter).expect("raw tx signed");
        assert_eq!(signed, signed_raw);
        let error = plan
            .sign_with(&mut adapter)
            .expect_err("wrong response kind is rejected");

        assert!(matches!(
            error,
            ExternalSignerAdapterError::UnexpectedOrderResponse
        ));
        let (_, client) = adapter.into_parts();
        assert_eq!(client.seen[0].request_id, "keeper-tx-1");
        assert_eq!(client.seen[1].request_id, "keeper-tx-2");
        assert!(matches!(
            client.seen[0].payload,
            ExternalSigningPayload::RawTransaction(RawTransactionSigningRequest { .. })
        ));
    }

    #[test]
    fn external_raw_transaction_signing_request_wraps_submission_plan() {
        let transaction = crate::UnsignedCall {
            to: Address::repeat_byte(0x55),
            data: vec![0x12, 0x34, 0x56, 0x78],
        }
        .to_tx_request_with_value_and_metadata(
            123,
            crate::TxRequestMetadata::new()
                .with_from(Address::repeat_byte(0x44))
                .with_nonce(7)
                .with_gas(250_000)
                .with_gas_price(1_000_000_000)
                .with_eip1559_fees(2_000_000_000, 1_000_000_000)
                .with_chain_id(11111),
        );
        let plan = TxSubmissionPlan::new(
            transaction.clone(),
            TxConfirmationPolicy::new(2).with_timeout_blocks(20),
        );
        let backend =
            SignerBackendConfig::new(SignerBackendKind::Relayer, "relayer-1").expect("backend");

        let external = plan
            .external_raw_transaction_signing_request("tx-1", backend)
            .expect("external signing request");

        let report = external.report();
        assert_eq!(report.request_id, "tx-1");
        assert_eq!(report.backend_kind, SignerBackendKind::Relayer);
        assert_eq!(report.backend_key_id, "relayer-1");
        assert_eq!(report.backend_address, None);
        assert!(!report.backend_has_address);
        assert_eq!(
            report.payload_kind,
            ExternalSigningPayloadKind::RawTransaction
        );
        assert!(!report.is_order_request);
        assert!(report.is_raw_transaction_request);
        assert_eq!(report.transaction_from, Some(Address::repeat_byte(0x44)));
        assert_eq!(report.transaction_to, Some(Address::repeat_byte(0x55)));
        assert_eq!(report.transaction_value.as_deref(), Some("0x7b"));
        assert_eq!(report.transaction_nonce.as_deref(), Some("0x7"));
        assert_eq!(report.transaction_gas.as_deref(), Some("0x3d090"));
        assert_eq!(report.transaction_gas_price.as_deref(), Some("0x3b9aca00"));
        assert_eq!(
            report.transaction_max_fee_per_gas.as_deref(),
            Some("0x77359400")
        );
        assert_eq!(
            report.transaction_max_priority_fee_per_gas.as_deref(),
            Some("0x3b9aca00")
        );
        assert_eq!(report.transaction_chain_id.as_deref(), Some("0x2b67"));
        assert!(report.transaction_has_sender);
        assert!(report.transaction_has_nonce);
        assert!(report.transaction_has_gas);
        assert!(report.transaction_has_any_fee);
        assert!(report.transaction_has_chain_id);
        assert!(report.transaction_uses_legacy_gas_price);
        assert!(report.transaction_uses_eip1559_fees);
        assert_eq!(report.transaction_selector.as_deref(), Some("0x12345678"));
        assert!(report.transaction_has_selector);
        assert_eq!(report.transaction_calldata_bytes, Some(4));
        assert_eq!(report.order_hash, None);
        let report_json = serde_json::to_string(&report).expect("serialize raw signing report");
        assert!(report_json.contains("\"transaction_value\":\"0x7b\""));
        assert!(report_json.contains("\"transaction_has_any_fee\":true"));
        assert!(report_json.contains("\"transaction_uses_eip1559_fees\":true"));
        assert!(report_json.contains("\"is_order_request\":false"));
        assert!(report_json.contains("\"is_raw_transaction_request\":true"));
        let restored_report: ExternalSigningRequestReport =
            serde_json::from_str(&report_json).expect("deserialize raw signing report");
        assert_eq!(restored_report, report);
        let legacy_report_json = report_json
            .replace("\"backend_has_address\":false,", "")
            .replace("\"is_order_request\":false,", "")
            .replace("\"is_raw_transaction_request\":true,", "")
            .replace("\"transaction_has_sender\":true,", "")
            .replace("\"transaction_has_nonce\":true,", "")
            .replace("\"transaction_has_gas\":true,", "")
            .replace("\"transaction_has_any_fee\":true,", "")
            .replace("\"transaction_has_chain_id\":true,", "")
            .replace("\"transaction_has_selector\":true,", "");
        let restored_legacy_report: ExternalSigningRequestReport =
            serde_json::from_str(&legacy_report_json).expect("deserialize legacy raw report");
        assert!(!restored_legacy_report.backend_has_address);
        assert!(!restored_legacy_report.is_order_request);
        assert!(!restored_legacy_report.is_raw_transaction_request);
        assert!(!restored_legacy_report.transaction_has_sender);
        assert!(!restored_legacy_report.transaction_has_nonce);
        assert!(!restored_legacy_report.transaction_has_gas);
        assert!(!restored_legacy_report.transaction_has_any_fee);
        assert!(!restored_legacy_report.transaction_has_chain_id);
        assert!(!restored_legacy_report.transaction_has_selector);
        assert!(matches!(
            external.payload,
            ExternalSigningPayload::RawTransaction(RawTransactionSigningRequest { .. })
        ));
        if let ExternalSigningPayload::RawTransaction(request) = external.payload {
            assert_eq!(request.transaction, transaction);
        }

        let response = ExternalSigningResponse::RawTransaction(
            SignedRawTransaction::from_hex("0x02abcd").expect("raw tx"),
        );
        let response_report = response.report();
        assert_eq!(
            response_report.payload_kind,
            ExternalSigningPayloadKind::RawTransaction
        );
        assert!(!response_report.is_order_signature);
        assert!(response_report.is_raw_transaction);
        assert_eq!(response_report.order_signature_hex, None);
        assert!(!response_report.has_order_signature_hex);
        assert!(!response_report.has_order_signature_bytes);
        assert_eq!(response_report.raw_transaction_bytes, Some(3));
        assert!(response_report.has_raw_transaction_bytes);
        assert_eq!(
            response_report.raw_transaction_hash.as_deref(),
            Some("0xe3607eedbe2ea88ad1994e3ef901f3c7ed167a59ebb5ffe5e40321e468f49eb1")
        );
        assert!(response_report.has_raw_transaction_hash);
        let response_report_json =
            serde_json::to_string(&response_report).expect("response report serializes");
        assert!(response_report_json.contains("\"raw_transaction_bytes\":3"));
        assert!(response_report_json.contains("\"has_raw_transaction_hash\":true"));
        assert!(response_report_json.contains("\"is_order_signature\":false"));
        assert!(response_report_json.contains("\"is_raw_transaction\":true"));
        let restored_response_report: ExternalSigningResponseReport =
            serde_json::from_str(&response_report_json).expect("response report deserializes");
        assert_eq!(restored_response_report, response_report);
        let mut legacy_response_report_json =
            serde_json::to_value(&response_report).expect("response report value");
        let legacy_response_report_object = legacy_response_report_json
            .as_object_mut()
            .expect("response report object");
        legacy_response_report_object.remove("is_order_signature");
        legacy_response_report_object.remove("is_raw_transaction");
        legacy_response_report_object.remove("has_order_signature_hex");
        legacy_response_report_object.remove("has_order_signature_bytes");
        legacy_response_report_object.remove("has_raw_transaction_bytes");
        legacy_response_report_object.remove("has_raw_transaction_hash");
        let restored_legacy_response_report: ExternalSigningResponseReport =
            serde_json::from_value(legacy_response_report_json)
                .expect("deserialize legacy raw response report");
        assert!(!restored_legacy_response_report.is_order_signature);
        assert!(!restored_legacy_response_report.is_raw_transaction);
        assert!(!restored_legacy_response_report.has_order_signature_hex);
        assert!(!restored_legacy_response_report.has_order_signature_bytes);
        assert!(!restored_legacy_response_report.has_raw_transaction_bytes);
        assert!(!restored_legacy_response_report.has_raw_transaction_hash);
        let json = serde_json::to_string(&response).expect("serialize response");
        let decoded: ExternalSigningResponse =
            serde_json::from_str(&json).expect("decode response");
        assert_eq!(decoded, response);
    }
}
