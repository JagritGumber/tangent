//! Raw calldata helpers for the current `IOrderBook` interface.
//!
//! These helpers intentionally stop at calldata construction. They do not
//! submit transactions or perform RPC reads; full client behavior remains a
//! later layer once a full-stack deployment is published.

use alloy_primitives::{keccak256, B256};

use crate::{AbiDecodeError, Order};

fn selector(signature: &str) -> [u8; 4] {
    let hash = keccak256(signature.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

fn bytes32_call(signature: &str, value: B256) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(&selector(signature));
    crate::eip712::encode_bytes32(&mut out, value);
    out
}

fn no_arg_call(signature: &str) -> Vec<u8> {
    selector(signature).to_vec()
}

/// ABI helpers for `IOrderBook`.
pub struct OrderBookCalls;

impl OrderBookCalls {
    pub const CANCEL_ORDER_SIGNATURE: &'static str = "cancelOrder(bytes32)";
    pub const IS_LIVE_SIGNATURE: &'static str = "isLive(bytes32)";
    pub const ORDER_OF_SIGNATURE: &'static str = "orderOf(bytes32)";
    pub const TICK_SIGNATURE: &'static str = "tick()";

    /// Compute the 4-byte selector for `cancelOrder(bytes32)`.
    #[must_use]
    pub fn cancel_order_selector() -> [u8; 4] {
        selector(Self::CANCEL_ORDER_SIGNATURE)
    }

    /// ABI-encode `OrderBook.cancelOrder(orderHash)` calldata.
    #[must_use]
    pub fn cancel_order_calldata(order_hash: B256) -> Vec<u8> {
        bytes32_call(Self::CANCEL_ORDER_SIGNATURE, order_hash)
    }

    /// ABI-encode `OrderBook.cancelOrder(orderHash)` as `0x` hex.
    #[must_use]
    pub fn cancel_order_calldata_hex(order_hash: B256) -> String {
        format!("0x{}", hex::encode(Self::cancel_order_calldata(order_hash)))
    }

    /// Compute the 4-byte selector for `isLive(bytes32)`.
    #[must_use]
    pub fn is_live_selector() -> [u8; 4] {
        selector(Self::IS_LIVE_SIGNATURE)
    }

    /// ABI-encode `OrderBook.isLive(orderHash)` calldata.
    #[must_use]
    pub fn is_live_calldata(order_hash: B256) -> Vec<u8> {
        bytes32_call(Self::IS_LIVE_SIGNATURE, order_hash)
    }

    /// Compute the 4-byte selector for `orderOf(bytes32)`.
    #[must_use]
    pub fn order_of_selector() -> [u8; 4] {
        selector(Self::ORDER_OF_SIGNATURE)
    }

    /// ABI-encode `OrderBook.orderOf(orderHash)` calldata.
    #[must_use]
    pub fn order_of_calldata(order_hash: B256) -> Vec<u8> {
        bytes32_call(Self::ORDER_OF_SIGNATURE, order_hash)
    }

    /// Compute the 4-byte selector for `tick()`.
    #[must_use]
    pub fn tick_selector() -> [u8; 4] {
        selector(Self::TICK_SIGNATURE)
    }

    /// ABI-encode `OrderBook.tick()` calldata.
    #[must_use]
    pub fn tick_calldata() -> Vec<u8> {
        no_arg_call(Self::TICK_SIGNATURE)
    }

    pub fn decode_is_live_return(data: &[u8]) -> Result<bool, AbiDecodeError> {
        crate::abi::decode_bool(data)
    }

    /// Decode `OrderBook.orderOf(orderHash)` return data.
    ///
    /// The Solidity return shape is the eight fixed `Order` fields followed by
    /// the `exists` flag. This decoder only parses those ABI words; it does
    /// not perform the `eth_call`.
    pub fn decode_order_of_return(data: &[u8]) -> Result<(Order, bool), AbiDecodeError> {
        if data.len() != 288 {
            return Err(AbiDecodeError::InvalidLength {
                expected: 288,
                actual: data.len(),
            });
        }

        let expiry = crate::abi::decode_u128(&data[192..224])?;
        if expiry > u64::MAX as u128 {
            return Err(AbiDecodeError::UintOverflow);
        }

        Ok((
            Order::new(
                crate::abi::decode_u128(&data[0..32])?,
                crate::abi::decode_u128(&data[32..64])?,
                crate::abi::decode_bool(&data[64..96])?,
                crate::abi::decode_u128(&data[96..128])?,
                crate::abi::decode_u128(&data[128..160])?,
                crate::abi::decode_u128(&data[160..192])?,
                expiry as u64,
                crate::abi::decode_bool(&data[224..256])?,
            ),
            crate::abi::decode_bool(&data[256..288])?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> B256 {
        B256::repeat_byte(0x11)
    }

    #[test]
    fn selectors_match_foundry_fixtures() {
        assert_eq!(
            hex::encode(OrderBookCalls::cancel_order_selector()),
            "7489ec23"
        );
        assert_eq!(hex::encode(OrderBookCalls::is_live_selector()), "cedd3a0e");
        assert_eq!(hex::encode(OrderBookCalls::order_of_selector()), "d5a014e5");
        assert_eq!(hex::encode(OrderBookCalls::tick_selector()), "3eaf5d9f");
    }

    #[test]
    fn bytes32_calls_encode_selector_plus_hash() {
        let cancel = OrderBookCalls::cancel_order_calldata(hash());
        assert_eq!(cancel.len(), 36);
        assert_eq!(&cancel[..4], &OrderBookCalls::cancel_order_selector());
        assert_eq!(&cancel[4..], hash().as_slice());

        let is_live = OrderBookCalls::is_live_calldata(hash());
        assert_eq!(is_live.len(), 36);
        assert_eq!(&is_live[..4], &OrderBookCalls::is_live_selector());
        assert_eq!(&is_live[4..], hash().as_slice());

        let order_of = OrderBookCalls::order_of_calldata(hash());
        assert_eq!(order_of.len(), 36);
        assert_eq!(&order_of[..4], &OrderBookCalls::order_of_selector());
        assert_eq!(&order_of[4..], hash().as_slice());
    }

    #[test]
    fn tick_call_is_selector_only() {
        let tick = OrderBookCalls::tick_calldata();
        assert_eq!(tick, OrderBookCalls::tick_selector());
    }

    #[test]
    fn decodes_is_live_return() {
        let mut word = [0u8; 32];
        word[31] = 1;
        assert!(OrderBookCalls::decode_is_live_return(&word).expect("is live"));
    }

    #[test]
    fn decodes_order_of_return() {
        fn word(value: u8) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[31] = value;
            out
        }

        let mut data = Vec::new();
        data.extend_from_slice(&word(7));
        data.extend_from_slice(&word(1));
        data.extend_from_slice(&word(1));
        data.extend_from_slice(&word(9));
        data.extend_from_slice(&word(8));
        data.extend_from_slice(&word(6));
        data.extend_from_slice(&word(5));
        data.extend_from_slice(&word(0));
        data.extend_from_slice(&word(1));

        let (order, exists) = OrderBookCalls::decode_order_of_return(&data).expect("order decodes");

        assert!(exists);
        assert_eq!(order, Order::new(7, 1, true, 9, 8, 6, 5, false));
    }

    #[test]
    fn decodes_missing_order_of_return() {
        let data = [0u8; 288];

        let (order, exists) = OrderBookCalls::decode_order_of_return(&data).expect("order decodes");

        assert!(!exists);
        assert_eq!(order, Order::new(0, 0, false, 0, 0, 0, 0, false));
    }

    #[test]
    fn rejects_bad_order_of_return_shape() {
        assert_eq!(
            OrderBookCalls::decode_order_of_return(&[0u8; 287]).expect_err("bad length"),
            AbiDecodeError::InvalidLength {
                expected: 288,
                actual: 287,
            }
        );
    }

    #[test]
    fn rejects_expiry_overflow_in_order_of_return() {
        let mut data = [0u8; 288];
        data[208..224].copy_from_slice(&u128::from(u64::MAX).saturating_add(1).to_be_bytes());

        assert_eq!(
            OrderBookCalls::decode_order_of_return(&data).expect_err("expiry overflow"),
            AbiDecodeError::UintOverflow,
        );
    }
}
