use alloy_primitives::{keccak256, Address, B256};

pub(crate) fn encode_bytes32(out: &mut Vec<u8>, value: B256) {
    out.extend_from_slice(value.as_slice());
}

pub(crate) fn encode_u64(out: &mut Vec<u8>, value: u64) {
    let mut word = [0u8; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    out.extend_from_slice(&word);
}

pub(crate) fn encode_u128(out: &mut Vec<u8>, value: u128) {
    let mut word = [0u8; 32];
    word[16..].copy_from_slice(&value.to_be_bytes());
    out.extend_from_slice(&word);
}

pub(crate) fn encode_bool(out: &mut Vec<u8>, value: bool) {
    let mut word = [0u8; 32];
    word[31] = u8::from(value);
    out.extend_from_slice(&word);
}

pub(crate) fn encode_address(out: &mut Vec<u8>, value: Address) {
    let mut word = [0u8; 32];
    word[12..].copy_from_slice(value.as_slice());
    out.extend_from_slice(&word);
}

pub(crate) fn hash_words(words: Vec<u8>) -> B256 {
    keccak256(words)
}

pub(crate) fn digest(domain_separator: B256, struct_hash: B256) -> B256 {
    let mut payload = Vec::with_capacity(66);
    payload.extend_from_slice(b"\x19\x01");
    payload.extend_from_slice(domain_separator.as_slice());
    payload.extend_from_slice(struct_hash.as_slice());
    keccak256(payload)
}
