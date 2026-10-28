use crate::ber::{BerError, BerReader, encoding::encode_tlv, tag};
use crate::{Oid, SnmpValue};

use super::SnmpError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PduType {
    Get,
    GetNext,
    Response,
    Set,
    GetBulk,
    Report,
}

impl PduType {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Get => 0xA0,
            Self::GetNext => 0xA1,
            Self::Response => 0xA2,
            Self::Set => 0xA3,
            Self::GetBulk => 0xA5,
            Self::Report => 0xA8,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        [
            Self::Get,
            Self::GetNext,
            Self::Response,
            Self::Set,
            Self::GetBulk,
            Self::Report,
        ]
        .into_iter()
        .find(|t| t.tag() == tag)
    }
}

/// An OID and value pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VarBind {
    pub oid: Oid,
    pub value: SnmpValue,
}

/// SNMP PDU. For GetBulk, `error_status` / `error_index` carry non-repeaters / max-repetitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pdu {
    pub(crate) pdu_type: PduType,
    pub(crate) request_id: i32,
    pub(crate) error_status: i64,
    pub(crate) error_index: i64,
    pub(crate) var_binds: Vec<VarBind>,
}

impl Pdu {
    pub(crate) fn get(request_id: i32, oids: &[Oid]) -> Self {
        Self::request(PduType::Get, request_id, 0, 0, oids)
    }

    pub(crate) fn get_next(request_id: i32, oids: &[Oid]) -> Self {
        Self::request(PduType::GetNext, request_id, 0, 0, oids)
    }

    pub(crate) fn get_bulk(
        request_id: i32,
        non_repeaters: u32,
        max_repetitions: u32,
        oids: &[Oid],
    ) -> Self {
        Self::request(
            PduType::GetBulk,
            request_id,
            non_repeaters.into(),
            max_repetitions.into(),
            oids,
        )
    }

    /// Request values are always NULL.
    fn request(
        pdu_type: PduType,
        request_id: i32,
        error_status: i64,
        error_index: i64,
        oids: &[Oid],
    ) -> Self {
        let var_binds = oids
            .iter()
            .map(|oid| VarBind {
                oid: oid.clone(),
                value: SnmpValue::Null,
            })
            .collect();
        Pdu {
            pdu_type,
            request_id,
            error_status,
            error_index,
            var_binds,
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut var_bind_list = Vec::new();
        for bind in &self.var_binds {
            let mut content = SnmpValue::Oid(bind.oid.clone()).encode();
            content.extend(bind.value.encode());
            var_bind_list.extend(encode_tlv(tag::SEQUENCE, &content));
        }
        let mut content = SnmpValue::Integer(self.request_id.into()).encode();
        content.extend(SnmpValue::Integer(self.error_status).encode());
        content.extend(SnmpValue::Integer(self.error_index).encode());
        content.extend(encode_tlv(tag::SEQUENCE, &var_bind_list));
        encode_tlv(self.pdu_type.tag(), &content)
    }

    pub(crate) fn decode(reader: &mut BerReader<'_>) -> Result<Self, SnmpError> {
        let raw_type = reader.peek_tag()?;
        let pdu_type = PduType::from_tag(raw_type).ok_or(SnmpError::UnexpectedPduType(raw_type))?;
        let mut pdu = reader.read_constructed(raw_type)?;

        let request_id = i32::try_from(pdu.read_integer()?)
            .map_err(|_| BerError::IntegerOverflow { tag: tag::INTEGER })?;
        let error_status = pdu.read_integer()?;
        let error_index = pdu.read_integer()?;

        let mut list = pdu.read_sequence()?;
        let mut var_binds = Vec::new();
        while !list.is_at_end() {
            let mut bind = list.read_sequence()?;
            let SnmpValue::Oid(oid) = SnmpValue::decode(&mut bind)? else {
                return Err(BerError::InvalidContent { tag: tag::OID }.into());
            };
            let value = SnmpValue::decode(&mut bind)?;
            var_binds.push(VarBind { oid, value });
        }
        Ok(Pdu {
            pdu_type,
            request_id,
            error_status,
            error_index,
            var_binds,
        })
    }
}
