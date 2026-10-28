//! SNMPv3 messages (RFC 3412) and USM security parameters (RFC 3414 §2.4).

use std::ops::{BitOr, Range};

use crate::SnmpValue;
use crate::ber::{BerError, BerReader, encoding::encode_tlv, tag};
use crate::snmp::SnmpError;
use crate::snmp::pdu::Pdu;

/// msgFlags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct MsgFlags(pub(crate) u8);

impl MsgFlags {
    pub(crate) const AUTH: MsgFlags = MsgFlags(0x01);
    pub(crate) const PRIV: MsgFlags = MsgFlags(0x02);
    pub(crate) const REPORTABLE: MsgFlags = MsgFlags(0x04);

    pub(crate) fn contains(self, other: MsgFlags) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for MsgFlags {
    type Output = MsgFlags;

    fn bitor(self, other: MsgFlags) -> MsgFlags {
        MsgFlags(self.0 | other.0)
    }
}

/// UsmSecurityParameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct UsmSecurityParameters {
    pub(crate) engine_id: Vec<u8>,
    pub(crate) engine_boots: u32,
    pub(crate) engine_time: u32,
    pub(crate) user_name: Vec<u8>,
    pub(crate) auth_params: Vec<u8>,
    pub(crate) priv_params: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopedPdu {
    pub(crate) context_engine_id: Vec<u8>,
    pub(crate) context_name: Vec<u8>,
    pub(crate) pdu: Pdu,
}

impl ScopedPdu {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut content = SnmpValue::OctetString(self.context_engine_id.clone()).encode();
        content.extend(SnmpValue::OctetString(self.context_name.clone()).encode());
        content.extend(self.pdu.encode());
        encode_tlv(tag::SEQUENCE, &content)
    }

    /// Reads a ScopedPDU (trailing bytes are ignored, e.g. zero padding after DES decryption).
    pub(crate) fn decode(reader: &mut BerReader<'_>) -> Result<Self, SnmpError> {
        let mut scoped = reader.read_sequence()?;
        let context_engine_id = scoped.read_octet_string()?;
        let context_name = scoped.read_octet_string()?;
        let pdu = Pdu::decode(&mut scoped)?;
        Ok(ScopedPdu {
            context_engine_id,
            context_name,
            pdu,
        })
    }
}

/// msgData: a plaintext ScopedPDU, or encrypted bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScopedData {
    Plaintext(ScopedPdu),
    Encrypted(Vec<u8>),
}

/// SNMPv3 message (USM).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct V3Message {
    pub(crate) msg_id: i32,
    pub(crate) flags: MsgFlags,
    pub(crate) security: UsmSecurityParameters,
    pub(crate) data: ScopedData,
}

impl V3Message {
    pub(crate) const VERSION: i64 = 3;
    pub(crate) const USM_SECURITY_MODEL: i64 = 3;
    pub(crate) const MAX_SIZE: i64 = 65_507;

    /// Encoded bytes and the position of the authParams contents within them (`None` when authParams is empty), for filling in the HMAC.
    pub(crate) fn encode(&self) -> (Vec<u8>, Option<Range<usize>>) {
        let mut global_content = SnmpValue::Integer(self.msg_id.into()).encode();
        global_content.extend(SnmpValue::Integer(Self::MAX_SIZE).encode());
        global_content.extend(SnmpValue::OctetString(vec![self.flags.0]).encode());
        global_content.extend(SnmpValue::Integer(Self::USM_SECURITY_MODEL).encode());
        let global_data = encode_tlv(tag::SEQUENCE, &global_content);

        let security = &self.security;
        let mut before_auth = SnmpValue::OctetString(security.engine_id.clone()).encode();
        before_auth.extend(SnmpValue::Integer(security.engine_boots.into()).encode());
        before_auth.extend(SnmpValue::Integer(security.engine_time.into()).encode());
        before_auth.extend(SnmpValue::OctetString(security.user_name.clone()).encode());
        let auth_tlv = SnmpValue::OctetString(security.auth_params.clone()).encode();
        let mut usm_content = before_auth.clone();
        usm_content.extend(&auth_tlv);
        usm_content.extend(SnmpValue::OctetString(security.priv_params.clone()).encode());
        let usm_sequence = encode_tlv(tag::SEQUENCE, &usm_content);
        let security_parameters = SnmpValue::OctetString(usm_sequence.clone()).encode();

        let data = match &self.data {
            ScopedData::Plaintext(scoped) => scoped.encode(),
            ScopedData::Encrypted(bytes) => SnmpValue::OctetString(bytes.clone()).encode(),
        };

        let version = SnmpValue::Integer(Self::VERSION).encode();
        let mut content = version.clone();
        content.extend(&global_data);
        content.extend(&security_parameters);
        content.extend(&data);
        let message = encode_tlv(tag::SEQUENCE, &content);

        if security.auth_params.is_empty() {
            return (message, None);
        }
        // Add up the TLV header lengths from the outside in to find where the authParams contents are.
        let outer_header = message.len() - content.len();
        let security_header = security_parameters.len() - usm_sequence.len();
        let usm_header = usm_sequence.len() - usm_content.len();
        let auth_header = auth_tlv.len() - security.auth_params.len();
        let start = outer_header
            + version.len()
            + global_data.len()
            + security_header
            + usm_header
            + before_auth.len()
            + auth_header;
        (message, Some(start..start + security.auth_params.len()))
    }

    /// Decodes, and returns where the authParams contents are in `bytes` (they must be zeroed to verify the HMAC).
    pub(crate) fn decode(bytes: &[u8]) -> Result<(Self, Range<usize>), SnmpError> {
        let mut outer = BerReader::new(bytes);
        let mut message = outer.read_sequence()?;

        let version = message.read_integer()?;
        if version != Self::VERSION {
            return Err(SnmpError::UnsupportedVersion(version));
        }

        let mut global = message.read_sequence()?;
        let msg_id = i32::try_from(global.read_integer()?)
            .map_err(|_| BerError::IntegerOverflow { tag: tag::INTEGER })?;
        global.read_integer()?; // msgMaxSize
        let &[flags] = global.read_octet_string()?.as_slice() else {
            return Err(BerError::InvalidContent {
                tag: tag::OCTET_STRING,
            }
            .into());
        };
        if global.read_integer()? != Self::USM_SECURITY_MODEL {
            return Err(SnmpError::UnexpectedResponse(
                "unsupported security model".into(),
            ));
        }

        // msgSecurityParameters is an OCTET STRING wrapping BER; the child reader's positions stay relative to the whole packet.
        let (security_tag, security_range) = message.read_tlv_range()?;
        if security_tag != tag::OCTET_STRING {
            return Err(BerError::UnexpectedTag {
                expected: tag::OCTET_STRING,
                actual: security_tag,
            }
            .into());
        }
        let mut usm = message.sub_reader(security_range).read_sequence()?;
        let engine_id = usm.read_octet_string()?;
        let engine_boots = read_u32(&mut usm)?;
        let engine_time = read_u32(&mut usm)?;
        let user_name = usm.read_octet_string()?;
        let (auth_tag, auth_range) = usm.read_tlv_range()?;
        if auth_tag != tag::OCTET_STRING {
            return Err(BerError::UnexpectedTag {
                expected: tag::OCTET_STRING,
                actual: auth_tag,
            }
            .into());
        }
        let auth_params = usm.bytes(auth_range.clone()).to_vec();
        let priv_params = usm.read_octet_string()?;
        let security = UsmSecurityParameters {
            engine_id,
            engine_boots,
            engine_time,
            user_name,
            auth_params,
            priv_params,
        };

        let data = if message.peek_tag()? == tag::OCTET_STRING {
            ScopedData::Encrypted(message.read_octet_string()?)
        } else {
            ScopedData::Plaintext(ScopedPdu::decode(&mut message)?)
        };
        Ok((
            V3Message {
                msg_id,
                flags: MsgFlags(flags),
                security,
                data,
            },
            auth_range,
        ))
    }
}

/// engineBoots / engineTime: INTEGER, range 0…2³¹−1.
fn read_u32(reader: &mut BerReader<'_>) -> Result<u32, BerError> {
    u32::try_from(reader.read_integer()?)
        .map_err(|_| BerError::IntegerOverflow { tag: tag::INTEGER })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oids::SYS_UP_TIME;
    use crate::snmp::message::SnmpMessage;

    fn discovery(msg_id: i32) -> V3Message {
        V3Message {
            msg_id,
            flags: MsgFlags::REPORTABLE,
            security: UsmSecurityParameters::default(),
            data: ScopedData::Plaintext(ScopedPdu {
                context_engine_id: vec![],
                context_name: vec![],
                pdu: Pdu::get(msg_id, &[]),
            }),
        }
    }

    #[test]
    fn discovery_request_bytes() {
        #[rustfmt::skip]
        let expected = [
            0x30, 0x38,
            0x02, 0x01, 0x03,                               // msgVersion 3
            0x30, 0x0E,                                     // msgGlobalData
            0x02, 0x01, 0x01,                               //   msgID
            0x02, 0x03, 0x00, 0xFF, 0xE3,                   //   msgMaxSize 65507
            0x04, 0x01, 0x04,                               //   msgFlags reportable
            0x02, 0x01, 0x03,                               //   msgSecurityModel USM
            0x04, 0x10,                                     // msgSecurityParameters
            0x30, 0x0E,
            0x04, 0x00,                                     //   engineID
            0x02, 0x01, 0x00,                               //   engineBoots
            0x02, 0x01, 0x00,                               //   engineTime
            0x04, 0x00,                                     //   userName
            0x04, 0x00,                                     //   authParams
            0x04, 0x00,                                     //   privParams
            0x30, 0x11,                                     // ScopedPDU
            0x04, 0x00,                                     //   contextEngineID
            0x04, 0x00,                                     //   contextName
            0xA0, 0x0B, 0x02, 0x01, 0x01, 0x02, 0x01, 0x00, 0x02, 0x01, 0x00, 0x30, 0x00,
        ];
        assert_eq!(discovery(1).encode(), (expected.to_vec(), None));
    }

    #[test]
    fn plaintext_round_trip_tracks_auth_params() {
        let security = UsmSecurityParameters {
            engine_id: vec![0x80, 0x00, 0x3A, 0x8C, 0x04, 0x01],
            engine_boots: 12,
            engine_time: 345_678,
            user_name: b"anemobit".to_vec(),
            auth_params: vec![0; 12],
            priv_params: vec![],
        };
        let message = V3Message {
            msg_id: 42,
            flags: MsgFlags::REPORTABLE | MsgFlags::AUTH,
            data: ScopedData::Plaintext(ScopedPdu {
                context_engine_id: security.engine_id.clone(),
                context_name: vec![],
                pdu: Pdu::get(42, std::slice::from_ref(&*SYS_UP_TIME)),
            }),
            security,
        };
        let (bytes, range) = message.encode();
        let range = range.unwrap();
        assert_eq!(range.len(), 12);
        assert!(bytes[range.clone()].iter().all(|&b| b == 0));
        // After replacing authParams, the decoded value is the new one: the position really maps to the authParams contents.
        let mut marked = bytes.clone();
        marked[range.clone()].copy_from_slice(&[0xAB; 12]);
        let (decoded, decoded_range) = V3Message::decode(&marked).unwrap();
        assert_eq!(decoded.security.auth_params, [0xAB; 12]);
        assert_eq!(decoded_range, range);

        assert_eq!(V3Message::decode(&bytes), Ok((message, range)));
    }

    #[test]
    fn encrypted_data_round_trips() {
        let message = V3Message {
            msg_id: 7,
            flags: MsgFlags::AUTH | MsgFlags::PRIV,
            security: UsmSecurityParameters {
                engine_id: vec![1, 2, 3],
                engine_boots: 1,
                engine_time: 2,
                user_name: vec![0x61],
                auth_params: vec![0; 12],
                priv_params: vec![1, 2, 3, 4, 5, 6, 7, 8],
            },
            data: ScopedData::Encrypted(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };
        let (bytes, range) = message.encode();
        assert_eq!(V3Message::decode(&bytes), Ok((message, range.unwrap())));
    }

    #[test]
    fn large_engine_boots_and_time_round_trip() {
        // engineBoots / engineTime range is 0…2147483647, encoded as INTEGER.
        let message = V3Message {
            msg_id: 1,
            flags: MsgFlags::default(),
            security: UsmSecurityParameters {
                engine_boots: 2_147_483_647,
                engine_time: 2_147_483_647,
                ..Default::default()
            },
            data: ScopedData::Encrypted(vec![]),
        };
        let (bytes, _) = message.encode();
        assert_eq!(V3Message::decode(&bytes).map(|(m, _)| m), Ok(message));
    }

    #[test]
    fn rejects_negative_engine_time() {
        let message = V3Message {
            msg_id: 1,
            flags: MsgFlags::default(),
            security: UsmSecurityParameters::default(),
            data: ScopedData::Encrypted(vec![]),
        };
        let (mut bytes, _) = message.encode();
        // Change engineTime (0x02 0x01 0x00) to -1.
        let at = bytes
            .windows(6)
            .position(|w| w == [0x02, 0x01, 0x00, 0x02, 0x01, 0x00])
            .unwrap()
            + 5;
        bytes[at] = 0xFF;
        assert_eq!(
            V3Message::decode(&bytes),
            Err(SnmpError::Malformed(BerError::IntegerOverflow {
                tag: tag::INTEGER
            }))
        );
    }

    #[test]
    fn rejects_v2c_message() {
        let v2c =
            SnmpMessage::new("public", Pdu::get(1, std::slice::from_ref(&*SYS_UP_TIME))).encode();
        assert_eq!(
            V3Message::decode(&v2c),
            Err(SnmpError::UnsupportedVersion(1))
        );
    }

    #[test]
    fn rejects_non_usm_security_model() {
        let (mut bytes, _) = discovery(1).encode();
        bytes[20] = 0x02; // msgSecurityModel 3 → 2
        assert!(matches!(
            V3Message::decode(&bytes),
            Err(SnmpError::UnexpectedResponse(_))
        ));
    }

    #[test]
    fn rejects_flags_longer_than_one_byte() {
        let (bytes, _) = discovery(1).encode();
        // msgFlags 04 01 04 → 04 02 04 00 (length fields adjusted too).
        let mut changed = bytes[..15].to_vec();
        changed.extend([0x04, 0x02, 0x04, 0x00]);
        changed.extend(&bytes[18..]);
        changed[1] += 1;
        changed[6] += 1;
        assert_eq!(
            V3Message::decode(&changed),
            Err(SnmpError::Malformed(BerError::InvalidContent {
                tag: tag::OCTET_STRING
            }))
        );
    }

    #[test]
    fn scoped_pdu_ignores_trailing_padding() {
        let scoped = ScopedPdu {
            context_engine_id: vec![1, 2],
            context_name: vec![],
            pdu: Pdu::get(3, std::slice::from_ref(&*SYS_UP_TIME)),
        };
        let mut bytes = scoped.encode();
        bytes.extend([0, 0, 0]);
        assert_eq!(ScopedPdu::decode(&mut BerReader::new(&bytes)), Ok(scoped));
    }
}
