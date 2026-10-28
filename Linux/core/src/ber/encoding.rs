//! BER encoding and encoding / decoding of primitive contents.

use super::{BerError, Oid};

/// Length field: short form below 128, long form otherwise.
pub(crate) fn encode_length(length: usize) -> Vec<u8> {
    if length < 0x80 {
        return vec![length as u8];
    }
    let bytes = length.to_be_bytes();
    let first = bytes
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(bytes.len() - 1);
    let mut encoded = vec![0x80 | (bytes.len() - first) as u8];
    encoded.extend_from_slice(&bytes[first..]);
    encoded
}

pub(crate) fn encode_tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut encoded = vec![tag];
    encoded.extend(encode_length(content.len()));
    encoded.extend_from_slice(content);
    encoded
}

/// Shortest two's-complement representation.
pub(super) fn encode_signed(value: i64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    // A leading byte can be dropped when it only extends the sign bit of the next byte.
    let redundant = |i: usize| {
        (bytes[i] == 0x00 && bytes[i + 1] & 0x80 == 0)
            || (bytes[i] == 0xFF && bytes[i + 1] & 0x80 != 0)
    };
    let first = (0..bytes.len() - 1)
        .find(|&i| !redundant(i))
        .unwrap_or(bytes.len() - 1);
    bytes[first..].to_vec()
}

/// Unsigned integer: strip leading zeros, and prepend 0x00 when the high bit is 1 so it isn't read as negative.
pub(super) fn encode_unsigned(value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let first = bytes
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(bytes.len() - 1);
    let mut encoded = Vec::with_capacity(bytes.len() - first + 1);
    if bytes[first] & 0x80 != 0 {
        encoded.push(0x00);
    }
    encoded.extend_from_slice(&bytes[first..]);
    encoded
}

pub(super) fn encode_oid_content(oid: &Oid) -> Vec<u8> {
    let arcs = oid.arcs();
    let first = u64::from(arcs.first().copied().unwrap_or(0));
    let second = u64::from(arcs.get(1).copied().unwrap_or(0));
    let mut encoded = Vec::new();
    encode_base128(first * 40 + second, &mut encoded);
    for &arc in arcs.iter().skip(2) {
        encode_base128(arc.into(), &mut encoded);
    }
    encoded
}

fn encode_base128(value: u64, out: &mut Vec<u8>) {
    let groups = (u64::BITS - value.leading_zeros()).div_ceil(7).max(1);
    for i in (0..groups).rev() {
        let digit = ((value >> (7 * i)) & 0x7F) as u8;
        out.push(if i == 0 { digit } else { digit | 0x80 });
    }
}

pub(super) fn decode_signed(content: &[u8], tag: u8) -> Result<i64, BerError> {
    let Some(&first) = content.first() else {
        return Err(BerError::InvalidContent { tag });
    };
    if content.len() > 8 {
        return Err(BerError::IntegerOverflow { tag });
    }
    let initial: i64 = if first & 0x80 != 0 { -1 } else { 0 };
    Ok(content
        .iter()
        .fold(initial, |value, &byte| (value << 8) | i64::from(byte)))
}

/// `bits` is the type width (32 or 64); leading zeros don't count toward the width.
pub(super) fn decode_unsigned(content: &[u8], tag: u8, bits: u32) -> Result<u64, BerError> {
    if content.is_empty() {
        return Err(BerError::InvalidContent { tag });
    }
    let first = content
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(content.len() - 1);
    let digits = &content[first..];
    if digits.len() > (bits / 8) as usize {
        return Err(BerError::IntegerOverflow { tag });
    }
    Ok(digits
        .iter()
        .fold(0, |value, &byte| (value << 8) | u64::from(byte)))
}

pub(super) fn decode_oid_content(content: &[u8]) -> Result<Oid, BerError> {
    if content.is_empty() {
        return Err(BerError::InvalidOid);
    }
    let mut subidentifiers = Vec::new();
    let mut current: u64 = 0;
    let mut in_progress = false;
    for &byte in content {
        // Guard against u64 overflow first; whether an arc exceeds u32 is checked after splitting.
        if current >> 57 != 0 {
            return Err(BerError::InvalidOid);
        }
        current = (current << 7) | u64::from(byte & 0x7F);
        in_progress = byte & 0x80 != 0;
        if !in_progress {
            subidentifiers.push(current);
            current = 0;
        }
    }
    if in_progress {
        return Err(BerError::InvalidOid);
    }

    // The first subidentifier is 40 × the first arc + the second arc; the first arc can only be 0, 1 or 2.
    let combined = subidentifiers[0];
    let first = (combined / 40).min(2);
    let second = combined - first * 40;
    std::iter::once(first)
        .chain(std::iter::once(second))
        .chain(subidentifiers[1..].iter().copied())
        .map(|arc| u32::try_from(arc).map_err(|_| BerError::InvalidOid))
        .collect::<Result<Vec<_>, _>>()
        .map(Oid::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_length_short_form() {
        assert_eq!(encode_length(0), [0x00]);
        assert_eq!(encode_length(127), [0x7F]);
    }

    #[test]
    fn encode_length_long_form() {
        assert_eq!(encode_length(128), [0x81, 0x80]);
        assert_eq!(encode_length(200), [0x81, 0xC8]);
        assert_eq!(encode_length(300), [0x82, 0x01, 0x2C]);
        assert_eq!(encode_length(70_000), [0x83, 0x01, 0x11, 0x70]);
    }

    #[test]
    fn encode_tlv_wraps_content() {
        assert_eq!(encode_tlv(0x30, &[0x05, 0x00]), [0x30, 0x02, 0x05, 0x00]);
    }
}
