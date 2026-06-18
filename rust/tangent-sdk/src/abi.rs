//! Minimal ABI decoding helpers for fixed-shape return values.

use alloy_primitives::Address;

/// Errors that can occur while decoding simple ABI return values.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AbiDecodeError {
    #[error("invalid ABI return length: expected {expected} bytes, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("invalid ABI dynamic offset: {0}")]
    InvalidOffset(usize),
    #[error("ABI uint value exceeds supported SDK width")]
    UintOverflow,
    #[error("ABI int value exceeds supported SDK width")]
    IntOverflow,
    #[error("invalid ABI bool value: {0}")]
    InvalidBool(u8),
    #[error("invalid ABI address word: non-zero high bytes")]
    InvalidAddressPadding,
    #[error("invalid ABI string: not valid UTF-8")]
    InvalidStringUtf8,
    #[error("inconsistent ABI return data: {0}")]
    InconsistentData(&'static str),
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

pub(crate) fn decode_empty(data: &[u8]) -> Result<(), AbiDecodeError> {
    if data.is_empty() {
        Ok(())
    } else {
        Err(AbiDecodeError::InvalidLength {
            expected: 0,
            actual: data.len(),
        })
    }
}

pub(crate) fn expect_return_count<T: AsRef<[u8]>>(
    returns: &[T],
    expected: usize,
) -> Result<Vec<&[u8]>, AbiDecodeError> {
    if returns.len() != expected {
        return Err(AbiDecodeError::InvalidLength {
            expected,
            actual: returns.len(),
        });
    }

    Ok(returns.iter().map(AsRef::as_ref).collect())
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

pub(crate) fn decode_i128(data: &[u8]) -> Result<i128, AbiDecodeError> {
    let word = single_word(data)?;
    let mut low = [0u8; 16];
    low.copy_from_slice(&word[16..]);

    let sign_padding = if low[0] & 0x80 == 0 { 0x00 } else { 0xff };
    if word[..16].iter().any(|byte| *byte != sign_padding) {
        return Err(AbiDecodeError::IntOverflow);
    }

    Ok(i128::from_be_bytes(low))
}

pub(crate) fn decode_address(data: &[u8]) -> Result<Address, AbiDecodeError> {
    let word = single_word(data)?;
    if word[..12].iter().any(|byte| *byte != 0) {
        return Err(AbiDecodeError::InvalidAddressPadding);
    }
    Ok(Address::from_slice(&word[12..]))
}

pub(crate) fn decode_dynamic_string(
    data: &[u8],
    head_index: usize,
    head_words: usize,
) -> Result<String, AbiDecodeError> {
    let head_len = head_words
        .checked_mul(32)
        .ok_or(AbiDecodeError::UintOverflow)?;
    if data.len() < head_len {
        return Err(AbiDecodeError::InvalidLength {
            expected: head_len,
            actual: data.len(),
        });
    }

    let offset_start = head_index
        .checked_mul(32)
        .ok_or(AbiDecodeError::UintOverflow)?;
    let offset_end = offset_start
        .checked_add(32)
        .ok_or(AbiDecodeError::UintOverflow)?;
    if offset_end > head_len {
        return Err(AbiDecodeError::InvalidOffset(offset_start));
    }

    let offset = usize::try_from(decode_u128(&data[offset_start..offset_end])?)
        .map_err(|_| AbiDecodeError::UintOverflow)?;
    if offset < head_len || offset % 32 != 0 {
        return Err(AbiDecodeError::InvalidOffset(offset));
    }

    let len_end = offset.checked_add(32).ok_or(AbiDecodeError::UintOverflow)?;
    if data.len() < len_end {
        return Err(AbiDecodeError::InvalidLength {
            expected: len_end,
            actual: data.len(),
        });
    }

    let string_len = usize::try_from(decode_u128(&data[offset..len_end])?)
        .map_err(|_| AbiDecodeError::UintOverflow)?;
    let string_end = len_end
        .checked_add(string_len)
        .ok_or(AbiDecodeError::UintOverflow)?;
    let padded_len = string_len
        .checked_add(31)
        .ok_or(AbiDecodeError::UintOverflow)?
        / 32
        * 32;
    let padded_end = len_end
        .checked_add(padded_len)
        .ok_or(AbiDecodeError::UintOverflow)?;
    if data.len() != padded_end {
        return Err(AbiDecodeError::InvalidLength {
            expected: padded_end,
            actual: data.len(),
        });
    }

    std::str::from_utf8(&data[len_end..string_end])
        .map(str::to_owned)
        .map_err(|_| AbiDecodeError::InvalidStringUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word_with_last(value: u8) -> [u8; 32] {
        let mut word = [0u8; 32];
        word[31] = value;
        word
    }

    fn word_u128(value: u128) -> [u8; 32] {
        let mut word = [0u8; 32];
        word[16..].copy_from_slice(&value.to_be_bytes());
        word
    }

    #[test]
    fn decodes_u128_bool_and_address() {
        assert_eq!(decode_empty(&[]).expect("empty"), ());
        assert_eq!(decode_u128(&word_with_last(7)).expect("u128"), 7);
        assert_eq!(decode_i128(&word_with_last(7)).expect("i128"), 7);
        assert!(decode_bool(&word_with_last(1)).expect("bool"));

        let mut address_word = [0u8; 32];
        address_word[12..].fill(0x11);
        assert_eq!(
            decode_address(&address_word).expect("address"),
            Address::repeat_byte(0x11)
        );

        let mut string_data = Vec::new();
        string_data.extend_from_slice(&word_u128(64));
        string_data.extend_from_slice(&address_word);
        string_data.extend_from_slice(&word_u128(3));
        let mut symbol = [0u8; 32];
        symbol[..3].copy_from_slice(b"BTC");
        string_data.extend_from_slice(&symbol);
        assert_eq!(
            decode_dynamic_string(&string_data, 0, 2).expect("string"),
            "BTC"
        );
    }

    #[test]
    fn decodes_negative_i128() {
        let minus_seven = (-7i128).to_be_bytes();
        let mut word = [0xffu8; 32];
        word[16..].copy_from_slice(&minus_seven);

        assert_eq!(decode_i128(&word).expect("i128"), -7);
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
        assert_eq!(
            decode_empty(&[0u8; 1]).expect_err("bad empty"),
            AbiDecodeError::InvalidLength {
                expected: 0,
                actual: 1,
            }
        );
        assert_eq!(
            expect_return_count(&[[0u8; 32]], 2).expect_err("bad return count"),
            AbiDecodeError::InvalidLength {
                expected: 2,
                actual: 1,
            }
        );
        assert_eq!(
            expect_return_count(&[[0u8; 32], [1u8; 32]], 2).expect("count matches"),
            vec![[0u8; 32].as_slice(), [1u8; 32].as_slice()]
        );

        let overflow = [1u8; 32];
        assert_eq!(
            decode_u128(&overflow).expect_err("overflow"),
            AbiDecodeError::UintOverflow
        );
        assert_eq!(
            decode_i128(&overflow).expect_err("int overflow"),
            AbiDecodeError::IntOverflow
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

        let mut bad_offset = Vec::new();
        bad_offset.extend_from_slice(&word_with_last(32));
        bad_offset.extend_from_slice(&[0u8; 32]);
        assert_eq!(
            decode_dynamic_string(&bad_offset, 0, 2).expect_err("bad offset"),
            AbiDecodeError::InvalidOffset(32)
        );
    }
}
