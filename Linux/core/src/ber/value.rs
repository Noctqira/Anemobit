use std::borrow::Cow;
use std::net::Ipv4Addr;

use super::encoding::{
    decode_oid_content, decode_signed, decode_unsigned, encode_oid_content, encode_signed,
    encode_tlv, encode_unsigned,
};
use super::{BerError, BerReader, Oid, tag};

/// Value of an SNMP varbind.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SnmpValue {
    Integer(i64),
    OctetString(Vec<u8>),
    Null,
    Oid(Oid),
    IpAddress(Ipv4Addr),
    Counter32(u32),
    Gauge32(u32),
    TimeTicks(u32),
    Counter64(u64),
    NoSuchObject,
    NoSuchInstance,
    EndOfMibView,
}

impl SnmpValue {
    /// `NoSuchObject` / `NoSuchInstance` / `EndOfMibView`.
    pub fn is_exception(&self) -> bool {
        matches!(
            self,
            Self::NoSuchObject | Self::NoSuchInstance | Self::EndOfMibView
        )
    }

    /// Numeric types as `u64` (counter, gauge, timeTicks, non-negative integer).
    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Self::Counter64(v) => Some(v),
            Self::Counter32(v) | Self::Gauge32(v) | Self::TimeTicks(v) => Some(v.into()),
            Self::Integer(v) => u64::try_from(v).ok(),
            _ => None,
        }
    }

    /// OCTET STRING decoded as UTF-8 (for ifName / ifDescr); invalid bytes are replaced with U+FFFD.
    pub fn text(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::OctetString(bytes) => Some(String::from_utf8_lossy(bytes)),
            _ => None,
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        match self {
            Self::Integer(v) => encode_tlv(tag::INTEGER, &encode_signed(*v)),
            Self::OctetString(bytes) => encode_tlv(tag::OCTET_STRING, bytes),
            Self::Null => encode_tlv(tag::NULL, &[]),
            Self::Oid(oid) => encode_tlv(tag::OID, &encode_oid_content(oid)),
            Self::IpAddress(address) => encode_tlv(tag::IP_ADDRESS, &address.octets()),
            Self::Counter32(v) => encode_tlv(tag::COUNTER32, &encode_unsigned((*v).into())),
            Self::Gauge32(v) => encode_tlv(tag::GAUGE32, &encode_unsigned((*v).into())),
            Self::TimeTicks(v) => encode_tlv(tag::TIME_TICKS, &encode_unsigned((*v).into())),
            Self::Counter64(v) => encode_tlv(tag::COUNTER64, &encode_unsigned(*v)),
            Self::NoSuchObject => encode_tlv(tag::NO_SUCH_OBJECT, &[]),
            Self::NoSuchInstance => encode_tlv(tag::NO_SUCH_INSTANCE, &[]),
            Self::EndOfMibView => encode_tlv(tag::END_OF_MIB_VIEW, &[]),
        }
    }

    pub(crate) fn decode(reader: &mut BerReader<'_>) -> Result<Self, BerError> {
        let (tag, content) = reader.read_tlv()?;
        // decode_unsigned has already checked the width, so converting to u32 can't fail.
        let unsigned32 = || decode_unsigned(content, tag, 32).map(|v| v as u32);
        Ok(match tag {
            tag::INTEGER => Self::Integer(decode_signed(content, tag)?),
            tag::OCTET_STRING => Self::OctetString(content.to_vec()),
            tag::NULL if content.is_empty() => Self::Null,
            tag::OID => Self::Oid(decode_oid_content(content)?),
            tag::IP_ADDRESS => {
                let octets: [u8; 4] = content
                    .try_into()
                    .map_err(|_| BerError::InvalidContent { tag })?;
                Self::IpAddress(Ipv4Addr::from(octets))
            }
            tag::COUNTER32 => Self::Counter32(unsigned32()?),
            tag::GAUGE32 => Self::Gauge32(unsigned32()?),
            tag::TIME_TICKS => Self::TimeTicks(unsigned32()?),
            tag::COUNTER64 => Self::Counter64(decode_unsigned(content, tag, 64)?),
            tag::NO_SUCH_OBJECT => Self::NoSuchObject,
            tag::NO_SUCH_INSTANCE => Self::NoSuchInstance,
            tag::END_OF_MIB_VIEW => Self::EndOfMibView,
            tag::NULL => return Err(BerError::InvalidContent { tag }),
            _ => return Err(BerError::UnknownTag(tag)),
        })
    }
}

impl BerReader<'_> {
    /// Reads an INTEGER; `InvalidContent` if it isn't one.
    pub(crate) fn read_integer(&mut self) -> Result<i64, BerError> {
        match SnmpValue::decode(self)? {
            SnmpValue::Integer(value) => Ok(value),
            _ => Err(BerError::InvalidContent { tag: tag::INTEGER }),
        }
    }

    /// Reads an OCTET STRING; `InvalidContent` if it isn't one.
    pub(crate) fn read_octet_string(&mut self) -> Result<Vec<u8>, BerError> {
        match SnmpValue::decode(self)? {
            SnmpValue::OctetString(bytes) => Ok(bytes),
            _ => Err(BerError::InvalidContent {
                tag: tag::OCTET_STRING,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodes a single TLV and checks there are no extra bytes.
    fn decode_single(bytes: &[u8]) -> Result<SnmpValue, BerError> {
        let mut reader = BerReader::new(bytes);
        let value = SnmpValue::decode(&mut reader)?;
        assert!(reader.is_at_end());
        Ok(value)
    }

    /// Encodes to `bytes`, and decodes back to the same value.
    fn assert_round_trip(value: SnmpValue, bytes: &[u8]) {
        assert_eq!(value.encode(), bytes, "{value:?}");
        assert_eq!(decode_single(bytes), Ok(value));
    }

    #[test]
    fn octet_string_longer_than_127_bytes_round_trips() {
        let payload = vec![0x41; 200];
        let encoded = SnmpValue::OctetString(payload.clone()).encode();
        assert_eq!(encoded[..3], [0x04, 0x81, 0xC8]);
        assert_eq!(encoded.len(), 203);
        assert_eq!(decode_single(&encoded), Ok(SnmpValue::OctetString(payload)));
    }

    #[test]
    fn octet_string_of_300_bytes_uses_two_length_bytes() {
        let payload: Vec<u8> = (0..300).map(|i| (i & 0xFF) as u8).collect();
        let encoded = SnmpValue::OctetString(payload.clone()).encode();
        assert_eq!(encoded[..4], [0x04, 0x82, 0x01, 0x2C]);
        assert_eq!(decode_single(&encoded), Ok(SnmpValue::OctetString(payload)));
    }

    #[test]
    fn integer_encoding() {
        let cases: [(i64, &[u8]); 9] = [
            (0, &[0x02, 0x01, 0x00]),
            (127, &[0x02, 0x01, 0x7F]),
            (128, &[0x02, 0x02, 0x00, 0x80]),
            (256, &[0x02, 0x02, 0x01, 0x00]),
            (-1, &[0x02, 0x01, 0xFF]),
            (-128, &[0x02, 0x01, 0x80]),
            (-129, &[0x02, 0x02, 0xFF, 0x7F]),
            (i32::MAX.into(), &[0x02, 0x04, 0x7F, 0xFF, 0xFF, 0xFF]),
            (i32::MIN.into(), &[0x02, 0x04, 0x80, 0x00, 0x00, 0x00]),
        ];
        for (value, bytes) in cases {
            assert_round_trip(SnmpValue::Integer(value), bytes);
        }
    }

    #[test]
    fn rejects_empty_integer() {
        assert_eq!(
            decode_single(&[0x02, 0x00]),
            Err(BerError::InvalidContent { tag: 0x02 })
        );
    }

    #[test]
    fn rejects_integer_wider_than_64_bits() {
        assert_eq!(
            decode_single(&[0x02, 0x09, 0x01, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(BerError::IntegerOverflow { tag: 0x02 })
        );
    }

    #[test]
    fn octet_string() {
        assert_round_trip(
            SnmpValue::OctetString(b"public".to_vec()),
            &[0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63],
        );
        assert_round_trip(SnmpValue::OctetString(vec![]), &[0x04, 0x00]);
    }

    #[test]
    fn null() {
        assert_round_trip(SnmpValue::Null, &[0x05, 0x00]);
        assert_eq!(
            decode_single(&[0x05, 0x01, 0x00]),
            Err(BerError::InvalidContent { tag: 0x05 })
        );
    }

    #[test]
    fn ip_address() {
        assert_round_trip(
            SnmpValue::IpAddress(Ipv4Addr::new(192, 168, 88, 1)),
            &[0x40, 0x04, 192, 168, 88, 1],
        );
        assert_eq!(
            decode_single(&[0x40, 0x03, 1, 2, 3]),
            Err(BerError::InvalidContent { tag: 0x40 })
        );
    }

    #[test]
    fn sequence_wraps_values() {
        let mut inner = SnmpValue::Integer(5).encode();
        inner.extend(SnmpValue::Null.encode());
        let encoded = encode_tlv(tag::SEQUENCE, &inner);
        assert_eq!(encoded, [0x30, 0x05, 0x02, 0x01, 0x05, 0x05, 0x00]);

        let mut reader = BerReader::new(&encoded);
        let mut sequence = reader.read_sequence().unwrap();
        assert_eq!(SnmpValue::decode(&mut sequence), Ok(SnmpValue::Integer(5)));
        assert_eq!(SnmpValue::decode(&mut sequence), Ok(SnmpValue::Null));
        assert!(sequence.is_at_end());
        assert!(reader.is_at_end());
    }

    #[test]
    fn unknown_tag_is_rejected() {
        assert_eq!(
            decode_single(&[0x1F, 0x00]),
            Err(BerError::UnknownTag(0x1F))
        );
    }

    #[test]
    fn counter32() {
        let cases: [(u32, &[u8]); 4] = [
            (0, &[0x41, 0x01, 0x00]),
            (127, &[0x41, 0x01, 0x7F]),
            (128, &[0x41, 0x02, 0x00, 0x80]),
            (u32::MAX, &[0x41, 0x05, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]),
        ];
        for (value, bytes) in cases {
            assert_round_trip(SnmpValue::Counter32(value), bytes);
        }
    }

    #[test]
    fn gauge32_and_time_ticks() {
        assert_round_trip(
            SnmpValue::Gauge32(1_000_000_000),
            &[0x42, 0x04, 0x3B, 0x9A, 0xCA, 0x00],
        );
        assert_round_trip(
            SnmpValue::TimeTicks(123_456),
            &[0x43, 0x03, 0x01, 0xE2, 0x40],
        );
    }

    #[test]
    fn counter64_boundaries() {
        let cases: [(u64, &[u8]); 3] = [
            (0, &[0x46, 0x01, 0x00]),
            (1 << 32, &[0x46, 0x05, 0x01, 0x00, 0x00, 0x00, 0x00]),
            (
                u64::MAX,
                &[
                    0x46, 0x09, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                ],
            ),
        ];
        for (value, bytes) in cases {
            assert_round_trip(SnmpValue::Counter64(value), bytes);
        }
    }

    #[test]
    fn counter64_without_leading_zero_still_decodes() {
        // Some agents omit the 0x00 prefix; still treat a set high bit as unsigned.
        assert_eq!(
            decode_single(&[0x46, 0x08, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
            Ok(SnmpValue::Counter64(u64::MAX))
        );
    }

    #[test]
    fn unsigned_overflow_is_rejected() {
        assert_eq!(
            decode_single(&[0x41, 0x05, 0x01, 0x00, 0x00, 0x00, 0x00]),
            Err(BerError::IntegerOverflow { tag: 0x41 })
        );
        assert_eq!(
            decode_single(&[0x46, 0x09, 0x01, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(BerError::IntegerOverflow { tag: 0x46 })
        );
        assert_eq!(
            decode_single(&[0x41, 0x00]),
            Err(BerError::InvalidContent { tag: 0x41 })
        );
    }

    #[test]
    fn oid_sys_up_time() {
        assert_round_trip(
            SnmpValue::Oid(Oid::from([1, 3, 6, 1, 2, 1, 1, 3, 0])),
            &[0x06, 0x08, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x03, 0x00],
        );
    }

    #[test]
    fn oid_first_two_arcs_are_combined() {
        // 1.3 → 40×1 + 3 = 43 = 0x2B; 0.0 → 0x00; 2.5 → 85 = 0x55
        assert_round_trip(SnmpValue::Oid(Oid::from([1, 3])), &[0x06, 0x01, 0x2B]);
        assert_round_trip(SnmpValue::Oid(Oid::from([0, 0])), &[0x06, 0x01, 0x00]);
        assert_round_trip(SnmpValue::Oid(Oid::from([2, 5])), &[0x06, 0x01, 0x55]);
    }

    #[test]
    fn oid_combined_first_arc_above_127() {
        // X.690 example: 2.999.3 → 88 37 03
        assert_round_trip(
            SnmpValue::Oid(Oid::from([2, 999, 3])),
            &[0x06, 0x03, 0x88, 0x37, 0x03],
        );
    }

    #[test]
    fn oid_multi_byte_arcs() {
        // 2636 (Juniper enterprise) → 0x94 0x4C; 14988 (MikroTik) → 0xF5 0x0C
        assert_round_trip(
            SnmpValue::Oid(Oid::from([1, 3, 6, 1, 4, 1, 2636, 14988])),
            &[
                0x06, 0x09, 0x2B, 0x06, 0x01, 0x04, 0x01, 0x94, 0x4C, 0xF5, 0x0C,
            ],
        );
    }

    #[test]
    fn oid_max_u32_arc() {
        assert_round_trip(
            SnmpValue::Oid(Oid::from([1, 3, u32::MAX])),
            &[0x06, 0x06, 0x2B, 0x8F, 0xFF, 0xFF, 0xFF, 0x7F],
        );
    }

    #[test]
    fn invalid_oids_are_rejected() {
        let cases: [&[u8]; 4] = [
            // Arc of 2^32 → 90 80 80 80 00
            &[0x06, 0x06, 0x2B, 0x90, 0x80, 0x80, 0x80, 0x00],
            // The last arc doesn't terminate
            &[0x06, 0x02, 0x2B, 0x86],
            // Empty OID
            &[0x06, 0x00],
            // Oversized arc (beyond u64)
            &[
                0x06, 0x0C, 0x2B, 0x81, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00,
            ],
        ];
        for bytes in cases {
            assert_eq!(
                decode_single(bytes),
                Err(BerError::InvalidOid),
                "{bytes:02X?}"
            );
        }
    }

    #[test]
    fn exception_values() {
        let cases: [(SnmpValue, &[u8]); 3] = [
            (SnmpValue::NoSuchObject, &[0x80, 0x00]),
            (SnmpValue::NoSuchInstance, &[0x81, 0x00]),
            (SnmpValue::EndOfMibView, &[0x82, 0x00]),
        ];
        for (value, bytes) in cases {
            assert!(value.is_exception());
            assert_round_trip(value, bytes);
        }
        assert!(!SnmpValue::Null.is_exception());
        assert!(!SnmpValue::Counter64(1).is_exception());
    }

    #[test]
    fn as_u64_converts_numeric_values() {
        assert_eq!(SnmpValue::Counter64(u64::MAX).as_u64(), Some(u64::MAX));
        assert_eq!(SnmpValue::Counter32(7).as_u64(), Some(7));
        assert_eq!(SnmpValue::Gauge32(8).as_u64(), Some(8));
        assert_eq!(SnmpValue::TimeTicks(9).as_u64(), Some(9));
        assert_eq!(SnmpValue::Integer(10).as_u64(), Some(10));
        assert_eq!(SnmpValue::Integer(-1).as_u64(), None);
        assert_eq!(SnmpValue::Null.as_u64(), None);
        assert_eq!(SnmpValue::OctetString(b"1".to_vec()).as_u64(), None);
    }

    #[test]
    fn text_decodes_utf8_octet_strings() {
        assert_eq!(
            SnmpValue::OctetString(b"pppoe-out1".to_vec())
                .text()
                .as_deref(),
            Some("pppoe-out1")
        );
        assert_eq!(
            SnmpValue::OctetString(vec![0x61, 0xFF]).text().as_deref(),
            Some("a\u{FFFD}")
        );
        assert_eq!(SnmpValue::Null.text(), None);
    }
}
