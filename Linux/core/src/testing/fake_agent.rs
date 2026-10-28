use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::snmp::message::SnmpMessage;
use crate::snmp::pdu::{Pdu, PduType};
use crate::{Oid, SnmpValue, VarBind, oids};

/// Simulates an SNMP agent's GET / GETNEXT / GETBULK with a sorted MIB table. Clones share the same table.
#[derive(Clone, Default)]
pub(crate) struct FakeAgent {
    mib: Rc<RefCell<BTreeMap<Oid, SnmpValue>>>,
}

impl FakeAgent {
    pub(crate) fn new(mib: BTreeMap<Oid, SnmpValue>) -> Self {
        FakeAgent {
            mib: Rc::new(RefCell::new(mib)),
        }
    }

    /// RouterOS-style interface table: index 1, 2, 3, 7, 12.
    pub(crate) fn router_os_interface_mib() -> BTreeMap<Oid, SnmpValue> {
        let names = [
            (1, "ether1"),
            (2, "ether2"),
            (3, "bridge"),
            (7, "pppoe-out1"),
            (12, "wlan1"),
        ];
        let mut mib = BTreeMap::from([(oids::SYS_UP_TIME.clone(), SnmpValue::TimeTicks(100))]);
        for (index, name) in names {
            let text = SnmpValue::OctetString(name.as_bytes().to_vec());
            mib.insert(oids::IF_DESCR.child(index), text.clone());
            mib.insert(oids::IF_NAME.child(index), text);
            mib.insert(oids::IF_HC_IN_OCTETS.child(index), SnmpValue::Counter64(0));
            mib.insert(oids::IF_HC_OUT_OCTETS.child(index), SnmpValue::Counter64(0));
        }
        mib
    }

    /// Updates (or adds) a value in the MIB.
    pub(crate) fn update(&self, values: impl IntoIterator<Item = (Oid, SnmpValue)>) {
        self.mib.borrow_mut().extend(values);
    }

    pub(crate) fn respond(&self, request: &SnmpMessage) -> SnmpMessage {
        SnmpMessage::new(request.community.clone(), self.respond_pdu(&request.pdu))
    }

    /// Produces the response PDU (also used by the SNMPv3 double).
    pub(crate) fn respond_pdu(&self, pdu: &Pdu) -> Pdu {
        let mib = self.mib.borrow();
        let next = |oid: &Oid| match mib.range(oid.clone()..).find(|(key, _)| *key > oid) {
            Some((key, value)) => VarBind {
                oid: key.clone(),
                value: value.clone(),
            },
            None => VarBind {
                oid: oid.clone(),
                value: SnmpValue::EndOfMibView,
            },
        };
        let var_binds = match pdu.pdu_type {
            PduType::Get => pdu
                .var_binds
                .iter()
                .map(|b| VarBind {
                    oid: b.oid.clone(),
                    value: mib
                        .get(&b.oid)
                        .cloned()
                        .unwrap_or(SnmpValue::NoSuchInstance),
                })
                .collect(),
            PduType::GetNext => pdu.var_binds.iter().map(|b| next(&b.oid)).collect(),
            PduType::GetBulk => {
                let non_repeaters = usize::try_from(pdu.error_status).unwrap_or(0);
                let max_repetitions = usize::try_from(pdu.error_index).unwrap_or(0);
                let mut binds: Vec<_> = pdu
                    .var_binds
                    .iter()
                    .take(non_repeaters)
                    .map(|b| next(&b.oid))
                    .collect();
                let mut cursors: Vec<_> = pdu
                    .var_binds
                    .iter()
                    .skip(non_repeaters)
                    .map(|b| b.oid.clone())
                    .collect();
                if !cursors.is_empty() {
                    for _ in 0..max_repetitions {
                        for cursor in &mut cursors {
                            let bind = next(cursor);
                            *cursor = bind.oid.clone();
                            binds.push(bind);
                        }
                    }
                }
                binds
            }
            _ => Vec::new(),
        };
        Pdu {
            pdu_type: PduType::Response,
            request_id: pdu.request_id,
            error_status: 0,
            error_index: 0,
            var_binds,
        }
    }
}
