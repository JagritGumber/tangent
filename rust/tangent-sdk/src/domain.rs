//! EIP-712 domain-separator helper inputs.

use alloy_primitives::{keccak256, Address, B256};
use serde::{Deserialize, Serialize};

/// On the Solidity side this is computed in
/// `OrderTypes.sol::domainSeparator(chainId, verifyingContract)` with
/// name `"Tangent"` and version `"v1"`. Any change to those constants
/// is a wire-breaking change and the [`DomainSeparatorInput::NAME`] /
/// [`DomainSeparatorInput::VERSION`] constants here must rev in lockstep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainSeparatorInput {
    /// The chain id the OrderBook is deployed on. Bound at deploy time
    /// in `Deploy.s.sol`; readers pin against the deployment manifest at
    /// `docs/deployments/arc-testnet.json` (v0.7 target).
    pub chain_id: u64,
    /// The OrderBook contract address. Bound at deploy time.
    pub verifying_contract: Address,
}

impl DomainSeparatorInput {
    /// EIP-712 domain type string. MUST match Solidity-side domain encoding.
    pub const EIP712_TYPE_STRING: &'static str =
        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

    /// EIP-712 domain name. MUST match the Solidity-side
    /// `keccak256(bytes("Tangent"))` argument.
    pub const NAME: &'static str = "Tangent";

    /// EIP-712 domain version. MUST match the Solidity-side
    /// `keccak256(bytes("v1"))` argument.
    pub const VERSION: &'static str = "v1";

    /// Construct from typed inputs.
    #[must_use]
    pub fn new(chain_id: u64, verifying_contract: Address) -> Self {
        Self {
            chain_id,
            verifying_contract,
        }
    }

    /// The canonical EIP-712 domain type hash.
    #[must_use]
    pub fn type_hash() -> B256 {
        keccak256(Self::EIP712_TYPE_STRING.as_bytes())
    }

    /// The canonical Tangent domain name hash.
    #[must_use]
    pub fn name_hash() -> B256 {
        keccak256(Self::NAME.as_bytes())
    }

    /// The canonical Tangent domain version hash.
    #[must_use]
    pub fn version_hash() -> B256 {
        keccak256(Self::VERSION.as_bytes())
    }

    /// Compute the Solidity-compatible EIP-712 domain separator.
    #[must_use]
    pub fn separator(&self) -> B256 {
        let mut encoded = Vec::with_capacity(160);
        crate::eip712::encode_bytes32(&mut encoded, Self::type_hash());
        crate::eip712::encode_bytes32(&mut encoded, Self::name_hash());
        crate::eip712::encode_bytes32(&mut encoded, Self::version_hash());
        crate::eip712::encode_u64(&mut encoded, self.chain_id);
        crate::eip712::encode_address(&mut encoded, self.verifying_contract);
        crate::eip712::hash_words(encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_name_and_version_match_solidity() {
        assert_eq!(DomainSeparatorInput::NAME, "Tangent");
        assert_eq!(DomainSeparatorInput::VERSION, "v1");
    }

    #[test]
    fn domain_separator_input_serde_roundtrips() {
        let input = DomainSeparatorInput::new(11111, Address::ZERO);
        let json = serde_json::to_string(&input).expect("serialize");
        let back: DomainSeparatorInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(input, back);
    }

    #[test]
    fn domain_hash_fixtures_match_solidity() {
        assert_eq!(
            hex::encode(DomainSeparatorInput::type_hash()),
            "8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f"
        );
        assert_eq!(
            hex::encode(DomainSeparatorInput::name_hash()),
            "14df5c3d6a0a828bb9b1f54dbef5ca1732b87823e796469a1de96a4ad5ccb767"
        );
        assert_eq!(
            hex::encode(DomainSeparatorInput::version_hash()),
            "0984d5efd47d99151ae1be065a709e56c602102f24c1abc4008eb3f815a8d217"
        );
    }
}
