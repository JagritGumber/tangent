//! Transport-agnostic transaction inputs produced by SDK workflow helpers.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

/// Unsigned transaction input produced by the SDK.
///
/// The SDK deliberately does not choose a transport, signer, nonce, gas limit,
/// or fee policy. Callers can pass these fields into Alloy, Circle Dev Wallets,
/// a relayer, or their own transaction builder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedTx {
    pub to: Address,
    pub data: Vec<u8>,
}

impl UnsignedTx {
    #[must_use]
    pub fn data_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }
}
