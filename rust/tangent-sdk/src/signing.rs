//! External-signer boundary types.
//!
//! Tangent intentionally keeps signing backends out of the minimal typed-data
//! core. These types let callers prepare the exact digest to sign, attach the
//! 65-byte EVM signature returned by a wallet service, and pass a single typed
//! payload to future RPC submission helpers.

use alloy_primitives::{keccak256, B256};
use serde::{Deserialize, Serialize};

use crate::{DomainSeparatorInput, Order};

/// An order plus its EIP-712 domain and final signing digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedOrder {
    pub order: Order,
    pub domain: DomainSeparatorInput,
    pub digest: B256,
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
        let trimmed = input.strip_prefix("0x").unwrap_or(input);
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

    use super::*;
    use crate::{BASE_SCALE, PRICE_SCALE};

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
    }

    #[test]
    fn signature_hex_roundtrips_with_prefix() {
        let signature = OrderSignature::from_bytes([1u8; OrderSignature::LEN]).expect("valid");
        let encoded = signature.to_hex();
        let decoded = OrderSignature::from_hex(&encoded).expect("valid hex");

        assert_eq!(signature, decoded);
    }

    #[test]
    fn signature_rejects_bad_length() {
        let err = OrderSignature::from_bytes([1u8; 64]).expect_err("bad length");
        assert!(matches!(err, SignatureError::InvalidLength { actual: 64 }));
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
}
