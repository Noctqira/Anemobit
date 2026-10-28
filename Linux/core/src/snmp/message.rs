use crate::SnmpValue;
use crate::ber::{BerReader, encoding::encode_tlv, tag};

use super::SnmpError;
use super::pdu::Pdu;

/// SNMPv2c message (version = 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnmpMessage {
    pub(crate) version: i64,
    pub(crate) community: Vec<u8>,
    pub(crate) pdu: Pdu,
}

impl SnmpMessage {
    pub(crate) const VERSION_V2C: i64 = 1;

    pub(crate) fn new(community: impl Into<Vec<u8>>, pdu: Pdu) -> Self {
        SnmpMessage {
            version: Self::VERSION_V2C,
            community: community.into(),
            pdu,
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut content = SnmpValue::Integer(self.version).encode();
        content.extend(SnmpValue::OctetString(self.community.clone()).encode());
        content.extend(self.pdu.encode());
        encode_tlv(tag::SEQUENCE, &content)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, SnmpError> {
        let mut outer = BerReader::new(bytes);
        let mut message = outer.read_sequence()?;
        let version = message.read_integer()?;
        if version != Self::VERSION_V2C {
            return Err(SnmpError::UnsupportedVersion(version));
        }
        let community = message.read_octet_string()?;
        let pdu = Pdu::decode(&mut message)?;
        Ok(SnmpMessage {
            version,
            community,
            pdu,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ber::BerError;
    use crate::snmp::oids::{IF_HC_IN_OCTETS, IF_HC_OUT_OCTETS, IF_NAME, SYS_UP_TIME};
    use crate::snmp::pdu::PduType;
    use crate::snmp::{ErrorStatus, VarBind};

    fn get_sys_up_time() -> SnmpMessage {
        SnmpMessage::new("public", Pdu::get(1, std::slice::from_ref(&*SYS_UP_TIME)))
    }

    #[test]
    fn get_request_bytes() {
        #[rustfmt::skip]
        let expected = [
            0x30, 0x26,
            0x02, 0x01, 0x01,                                   // version = 1 (v2c)
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,     // "public"
            0xA0, 0x19,                                         // GetRequest
            0x02, 0x01, 0x01,                                   // request-id
            0x02, 0x01, 0x00,                                   // error-status
            0x02, 0x01, 0x00,                                   // error-index
            0x30, 0x0E,
            0x30, 0x0C,
            0x06, 0x08, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x03, 0x00,
            0x05, 0x00,
        ];
        assert_eq!(get_sys_up_time().encode(), expected);
    }

    #[test]
    fn get_next_request_uses_tag_a1() {
        let message = SnmpMessage::new(
            "public",
            Pdu::get_next(1, std::slice::from_ref(&*SYS_UP_TIME)),
        );
        assert_eq!(message.encode()[13], 0xA1);
    }

    #[test]
    fn get_bulk_request_bytes() {
        let message = SnmpMessage::new(
            "public",
            Pdu::get_bulk(0x1234, 0, 10, std::slice::from_ref(&*IF_NAME)),
        );
        #[rustfmt::skip]
        let expected = [
            0x30, 0x29,
            0x02, 0x01, 0x01,
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,
            0xA5, 0x1C,                                         // GetBulkRequest
            0x02, 0x02, 0x12, 0x34,                             // request-id
            0x02, 0x01, 0x00,                                   // non-repeaters
            0x02, 0x01, 0x0A,                                   // max-repetitions
            0x30, 0x10,
            0x30, 0x0E,
            0x06, 0x0A, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x1F, 0x01, 0x01, 0x01, 0x01,
            0x05, 0x00,
        ];
        assert_eq!(message.encode(), expected);
    }

    #[test]
    fn multiple_var_binds_keep_their_order() {
        let oids = [
            IF_HC_IN_OCTETS.child(3),
            IF_HC_OUT_OCTETS.child(3),
            SYS_UP_TIME.clone(),
        ];
        let message = SnmpMessage::new("c", Pdu::get(7, &oids));
        let decoded = SnmpMessage::decode(&message.encode()).unwrap();
        let decoded_oids: Vec<_> = decoded
            .pdu
            .var_binds
            .iter()
            .map(|b| b.oid.clone())
            .collect();
        assert_eq!(decoded_oids, oids);
        assert!(
            decoded
                .pdu
                .var_binds
                .iter()
                .all(|b| b.value == SnmpValue::Null)
        );
    }

    #[test]
    fn decodes_response_with_no_such_instance() {
        #[rustfmt::skip]
        let bytes = [
            0x30, 0x29,
            0x02, 0x01, 0x01,
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,
            0xA2, 0x1C,                                         // Response
            0x02, 0x01, 0x01,
            0x02, 0x01, 0x00,
            0x02, 0x01, 0x00,
            0x30, 0x11,
            0x30, 0x0F,
            0x06, 0x0B, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x1F, 0x01, 0x01, 0x01, 0x06, 0x63,
            0x81, 0x00,                                         // noSuchInstance
        ];
        let message = SnmpMessage::decode(&bytes).unwrap();
        assert_eq!(message.version, 1);
        assert_eq!(message.community, b"public");
        assert_eq!(message.pdu.pdu_type, PduType::Response);
        assert_eq!(message.pdu.request_id, 1);
        assert_eq!(message.pdu.error_status, 0);
        assert_eq!(
            message.pdu.var_binds,
            [VarBind {
                oid: IF_HC_IN_OCTETS.child(99),
                value: SnmpValue::NoSuchInstance
            }]
        );
    }

    #[test]
    fn decodes_error_status() {
        #[rustfmt::skip]
        let bytes = [
            0x30, 0x26,
            0x02, 0x01, 0x01,
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,
            0xA2, 0x19,
            0x02, 0x01, 0x02,                                   // request-id 2
            0x02, 0x01, 0x02,                                   // error-status noSuchName
            0x02, 0x01, 0x01,                                   // error-index 1
            0x30, 0x0E,
            0x30, 0x0C,
            0x06, 0x08, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x03, 0x00,
            0x05, 0x00,
        ];
        let message = SnmpMessage::decode(&bytes).unwrap();
        assert_eq!(message.pdu.request_id, 2);
        assert_eq!(message.pdu.error_status, 2);
        assert_eq!(message.pdu.error_index, 1);
        assert_eq!(
            ErrorStatus::from_code(message.pdu.error_status),
            Some(ErrorStatus::NoSuchName)
        );
    }

    #[test]
    fn counter64_time_ticks_and_strings_round_trip() {
        let pdu = Pdu {
            pdu_type: PduType::Response,
            request_id: -5,
            error_status: 0,
            error_index: 0,
            var_binds: vec![
                VarBind {
                    oid: IF_HC_IN_OCTETS.child(3),
                    value: SnmpValue::Counter64(u64::MAX),
                },
                VarBind {
                    oid: SYS_UP_TIME.clone(),
                    value: SnmpValue::TimeTicks(4_294_967_295),
                },
                VarBind {
                    oid: IF_NAME.child(3),
                    value: SnmpValue::OctetString(b"pppoe-out1".to_vec()),
                },
            ],
        };
        let original = SnmpMessage::new("public", pdu);
        assert_eq!(SnmpMessage::decode(&original.encode()), Ok(original));
    }

    #[test]
    fn rejects_snmpv1_message() {
        let mut bytes = get_sys_up_time().encode();
        bytes[4] = 0x00; // version = 0
        assert_eq!(
            SnmpMessage::decode(&bytes),
            Err(SnmpError::UnsupportedVersion(0))
        );
    }

    #[test]
    fn rejects_unknown_pdu_type() {
        let mut bytes = get_sys_up_time().encode();
        bytes[13] = 0xA7;
        assert_eq!(
            SnmpMessage::decode(&bytes),
            Err(SnmpError::UnexpectedPduType(0xA7))
        );
    }

    #[test]
    fn rejects_request_id_outside_i32() {
        let mut message = get_sys_up_time().encode();
        // Change request-id to the 5-byte 0x01_0000_0000 (length fields adjusted too).
        message.splice(15..18, [0x02, 0x05, 0x01, 0x00, 0x00, 0x00, 0x00]);
        message[1] += 4;
        message[14] += 4;
        assert_eq!(
            SnmpMessage::decode(&message),
            Err(SnmpError::Malformed(BerError::IntegerOverflow {
                tag: tag::INTEGER
            }))
        );
    }

    #[test]
    fn rejects_var_bind_without_oid() {
        let mut bytes = get_sys_up_time().encode();
        bytes[28] = 0x04; // Change the varbind name from OID to OCTET STRING
        assert_eq!(
            SnmpMessage::decode(&bytes),
            Err(SnmpError::Malformed(BerError::InvalidContent {
                tag: tag::OID
            }))
        );
    }

    #[test]
    fn rejects_truncated_message() {
        let bytes = get_sys_up_time().encode();
        assert_eq!(
            SnmpMessage::decode(&bytes[..bytes.len() - 3]),
            Err(SnmpError::Malformed(BerError::Truncated))
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            SnmpMessage::decode(&[0xDE, 0xAD, 0xBE, 0xEF]),
            Err(SnmpError::Malformed(_))
        ));
    }
}
