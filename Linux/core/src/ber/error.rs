use std::fmt;

/// BER decoding error (wrapped in [`SnmpError::Malformed`](crate::SnmpError::Malformed)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BerError {
    /// The data ended before the TLV did.
    Truncated,
    /// Indefinite length (0x80), which SNMP doesn't allow.
    IndefiniteLength,
    /// Length field longer than 4 bytes.
    LengthTooLarge,
    UnexpectedTag {
        expected: u8,
        actual: u8,
    },
    UnknownTag(u8),
    InvalidContent {
        tag: u8,
    },
    IntegerOverflow {
        tag: u8,
    },
    InvalidOid,
}

impl fmt::Display for BerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "BER data is truncated"),
            Self::IndefiniteLength => write!(f, "BER indefinite length is not allowed"),
            Self::LengthTooLarge => write!(f, "BER length field is too large"),
            Self::UnexpectedTag { expected, actual } => {
                write!(
                    f,
                    "unexpected BER tag 0x{actual:02X} (expected 0x{expected:02X})"
                )
            }
            Self::UnknownTag(tag) => write!(f, "unknown BER tag 0x{tag:02X}"),
            Self::InvalidContent { tag } => write!(f, "invalid content for BER tag 0x{tag:02X}"),
            Self::IntegerOverflow { tag } => write!(f, "integer overflow for BER tag 0x{tag:02X}"),
            Self::InvalidOid => write!(f, "invalid OBJECT IDENTIFIER"),
        }
    }
}

impl std::error::Error for BerError {}
