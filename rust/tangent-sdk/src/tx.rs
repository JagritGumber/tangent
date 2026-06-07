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
    pub fn data_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }
}

/// Backwards-compatible alias for transaction-oriented workflow callers.
pub type UnsignedTx = UnsignedCall;
