//! Transport-agnostic call inputs produced by SDK workflow helpers.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

/// Unsigned contract call input produced by the SDK.
///
/// The SDK deliberately does not choose a transport, signer, nonce, gas limit,
/// fee policy, or `eth_call` execution policy. Callers can pass these fields
/// into Alloy, Circle Dev Wallets, a relayer, or their own transaction/call
/// builder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedCall {
    pub to: Address,
    pub data: Vec<u8>,
}

impl UnsignedCall {
    #[must_use]
    pub fn selector(&self) -> Option<[u8; 4]> {
        self.data.get(..4).map(|bytes| {
            let mut selector = [0u8; 4];
            selector.copy_from_slice(bytes);
            selector
        })
    }

    #[must_use]
    pub fn selector_hex(&self) -> Option<String> {
        self.selector()
            .map(|selector| format!("0x{}", hex::encode(selector)))
    }

    #[must_use]
    pub fn data_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }
}

/// Backwards-compatible alias for transaction-oriented workflow callers.
pub type UnsignedTx = UnsignedCall;

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use super::*;

    #[test]
    fn exposes_selector_helpers() {
        let call = UnsignedCall {
            to: Address::ZERO,
            data: vec![0x12, 0x34, 0x56, 0x78, 0xff],
        };

        assert_eq!(call.selector(), Some([0x12, 0x34, 0x56, 0x78]));
        assert_eq!(call.selector_hex(), Some("0x12345678".to_owned()));
        assert_eq!(call.data_hex(), "0x12345678ff");
    }

    #[test]
    fn selector_helpers_reject_short_data() {
        let call = UnsignedCall {
            to: Address::ZERO,
            data: vec![0x12, 0x34, 0x56],
        };

        assert_eq!(call.selector(), None);
        assert_eq!(call.selector_hex(), None);
    }
}
