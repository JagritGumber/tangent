//! Minimal ABI decoding helpers for single-word return values.

use alloy_primitives::Address;

/// Errors that can occur while decoding simple ABI return values.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AbiDecodeError {
    #[error("invalid ABI return length: expected {expected} bytes, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("ABI uint value exceeds supported SDK width")]
    UintOverflow,
    #[error("invalid ABI bool value: {0}")]
    InvalidBool(u8),
    #[error("invalid ABI address word: non-zero high bytes")]
    InvalidAddressPadding,
}

fn single_word(data: &[u8]) -> Result<&[u8], AbiDecodeError> {
    if data.len() != 32 {
        return Err(AbiDecodeError::InvalidLength {
            expected: 32,
            actual: data.len(),
        });
    }
    Ok(data)
}

pub(crate) fn decode_u128(data: &[u8]) -> Result<u128, AbiDecodeError> {
    let word = single_word(data)?;
    if word[..16].iter().any(|byte| *byte != 0) {
        return Err(AbiDecodeError::UintOverflow);
    }

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&word[16..]);
    Ok(u128::from_be_bytes(bytes))
}

pub(crate) fn decode_bool(data: &[u8]) -> Result<bool, AbiDecodeError> {
    let word = single_word(data)?;
    if word[..31].iter().any(|byte| *byte != 0) {
        return Err(AbiDecodeError::InvalidBool(word[31]));
    }

    match word[31] {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(AbiDecodeError::InvalidBool(value)),
    }
}

pub(crate) fn decode_address(data: &[u8]) -> Result<Address, AbiDecodeError> {
    let word = single_word(data)?;
    if word[..12].iter().any(|byte| *byte != 0) {
        return Err(AbiDecodeError::InvalidAddressPadding);
    }
    Ok(Address::from_slice(&word[12..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word_with_last(value: u8) -> [u8; 32] {
        let mut word = [0u8; 32];
        word[31] = value;
        word
    }

    #[test]
    fn decodes_u128_bool_and_address() {
        assert_eq!(decode_u128(&word_with_last(7)).expect("u128"), 7);
        assert!(decode_bool(&word_with_last(1)).expect("bool"));

        let mut address_word = [0u8; 32];
        address_word[12..].fill(0x11);
        assert_eq!(
            decode_address(&address_word).expect("address"),
            Address::repeat_byte(0x11)
        );
    }

    #[test]
    fn rejects_bad_return_shapes() {
        assert_eq!(
            decode_u128(&[0u8; 31]).expect_err("bad len"),
            AbiDecodeError::InvalidLength {
                expected: 32,
                actual: 31,
            }
        );

        let overflow = [1u8; 32];
        assert_eq!(
            decode_u128(&overflow).expect_err("overflow"),
            AbiDecodeError::UintOverflow
        );

        assert_eq!(
            decode_bool(&word_with_last(2)).expect_err("bad bool"),
            AbiDecodeError::InvalidBool(2)
        );

        let bad_address = [1u8; 32];
        assert_eq!(
            decode_address(&bad_address).expect_err("bad address"),
            AbiDecodeError::InvalidAddressPadding
        );
    }
}
