use crate::transport::Transport;
use crate::{Oid, VarBind, oids};

use super::{SnmpClient, SnmpError};

/// A network interface on the device.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnmpInterface {
    pub index: u32,
    pub name: String,
}

impl<T: Transport> SnmpClient<T> {
    /// Number of entries fetched per GETBULK when listing interfaces and resolving names.
    pub const DEFAULT_MAX_REPETITIONS: u32 = 20;

    /// Walks the `root` subtree with GETBULK and returns all varbinds (in OID order).
    pub async fn walk(
        &mut self,
        root: &Oid,
        max_repetitions: u32,
    ) -> Result<Vec<VarBind>, SnmpError> {
        let mut results = Vec::new();
        let mut cursor = root.clone();
        loop {
            let binds = self
                .get_bulk(std::slice::from_ref(&cursor), 0, max_repetitions)
                .await?;
            if binds.is_empty() {
                return Ok(results);
            }
            for bind in binds {
                if bind.value == crate::SnmpValue::EndOfMibView
                    || !bind.oid.starts_with(root)
                    || bind.oid == *root
                {
                    return Ok(results);
                }
                // OIDs returned by the agent must increase, or this would loop forever.
                if bind.oid <= cursor {
                    return Err(SnmpError::UnexpectedResponse(format!(
                        "OID not increasing: {}",
                        bind.oid
                    )));
                }
                cursor = bind.oid.clone();
                results.push(bind);
            }
        }
    }

    /// Lists all interfaces (walks ifName; falls back to ifDescr if the device has no ifName).
    pub async fn list_interfaces(&mut self) -> Result<Vec<SnmpInterface>, SnmpError> {
        let interfaces = self.interfaces(&oids::IF_NAME).await?;
        if !interfaces.is_empty() {
            return Ok(interfaces);
        }
        self.interfaces(&oids::IF_DESCR).await
    }

    /// Resolves several interface names (e.g. `pppoe-out1`) to ifIndex in one walk, in the same order as `names`.
    /// Returns `InterfaceNotFound` if any name is missing.
    pub async fn resolve_interface_indexes(
        &mut self,
        names: &[impl AsRef<str>],
    ) -> Result<Vec<u32>, SnmpError> {
        let interfaces = self.list_interfaces().await?;
        names
            .iter()
            .map(|name| {
                let name = name.as_ref();
                interfaces
                    .iter()
                    .find(|interface| interface.name == name)
                    .map(|interface| interface.index)
                    .ok_or_else(|| SnmpError::InterfaceNotFound(name.to_owned()))
            })
            .collect()
    }

    /// Walks a text column indexed by ifIndex (ifName / ifDescr).
    async fn interfaces(&mut self, column: &Oid) -> Result<Vec<SnmpInterface>, SnmpError> {
        let binds = self.walk(column, Self::DEFAULT_MAX_REPETITIONS).await?;
        Ok(binds
            .iter()
            .filter_map(|bind| {
                let &[index] = bind.oid.strip_prefix(column)? else {
                    return None;
                };
                Some(SnmpInterface {
                    index,
                    name: bind.value.text()?.into_owned(),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::SnmpValue;
    use crate::oids::{IF_DESCR, IF_NAME, SYS_UP_TIME};
    use crate::snmp::SnmpSecurity;
    use crate::snmp::pdu::PduType;
    use crate::testing::{FakeAgent, MockTransport, response, run};

    fn client(transport: &MockTransport) -> SnmpClient<MockTransport> {
        SnmpClient::new(transport.clone(), SnmpSecurity::Community("public".into()))
            .with_timeout(Duration::from_millis(50))
    }

    fn router_os() -> MockTransport {
        MockTransport::with_agent(FakeAgent::new(FakeAgent::router_os_interface_mib()))
    }

    fn text(value: &str) -> SnmpValue {
        SnmpValue::OctetString(value.as_bytes().to_vec())
    }

    #[test]
    fn walk_returns_subtree_across_multiple_bulk_requests() {
        let transport = router_os();
        let binds = run(client(&transport).walk(&IF_NAME, 2)).unwrap();
        let indexes: Vec<_> = binds
            .iter()
            .map(|b| b.oid.strip_prefix(&IF_NAME).unwrap()[0])
            .collect();
        assert_eq!(indexes, [1, 2, 3, 7, 12]);
        let names: Vec<_> = binds
            .iter()
            .map(|b| b.value.text().unwrap().into_owned())
            .collect();
        assert_eq!(names, ["ether1", "ether2", "bridge", "pppoe-out1", "wlan1"]);
        let sent = transport.sent();
        assert_eq!(sent.len(), 3);
        assert!(
            sent.iter()
                .all(|m| m.pdu.pdu_type == PduType::GetBulk && m.pdu.error_index == 2)
        );
    }

    #[test]
    fn walk_stops_at_end_of_mib_view() {
        let agent = FakeAgent::new([(IF_NAME.child(1), text("ether1"))].into());
        let binds = run(client(&MockTransport::with_agent(agent)).walk(&IF_NAME, 20)).unwrap();
        assert_eq!(binds.len(), 1);
    }

    #[test]
    fn walk_of_empty_subtree_is_empty() {
        let agent = FakeAgent::new([(SYS_UP_TIME.clone(), SnmpValue::TimeTicks(1))].into());
        assert_eq!(
            run(client(&MockTransport::with_agent(agent)).walk(&IF_NAME, 20)),
            Ok(vec![])
        );
    }

    #[test]
    fn walk_of_empty_response_is_empty() {
        let transport = MockTransport::v2c(|request, _| vec![response(request, vec![])]);
        assert_eq!(run(client(&transport).walk(&IF_NAME, 20)), Ok(vec![]));
    }

    #[test]
    fn walk_rejects_non_increasing_oids() {
        let looping = IF_NAME.child(1);
        let transport = MockTransport::v2c(move |request, _| {
            vec![response(
                request,
                vec![VarBind {
                    oid: looping.clone(),
                    value: text("a"),
                }],
            )]
        });
        assert!(matches!(
            run(client(&transport).walk(&IF_NAME, 20)),
            Err(SnmpError::UnexpectedResponse(_))
        ));
    }

    #[test]
    fn list_interfaces() {
        let interfaces = run(client(&router_os()).list_interfaces()).unwrap();
        let indexes: Vec<_> = interfaces.iter().map(|i| i.index).collect();
        assert_eq!(indexes, [1, 2, 3, 7, 12]);
        assert_eq!(
            interfaces[3],
            SnmpInterface {
                index: 7,
                name: "pppoe-out1".into()
            }
        );
    }

    #[test]
    fn list_interfaces_falls_back_to_if_descr() {
        let agent = FakeAgent::new([(IF_DESCR.child(4), text("eth0"))].into());
        assert_eq!(
            run(client(&MockTransport::with_agent(agent)).list_interfaces()),
            Ok(vec![SnmpInterface {
                index: 4,
                name: "eth0".into()
            }])
        );
    }

    #[test]
    fn list_interfaces_skips_rows_with_unexpected_shape() {
        let agent = FakeAgent::new(
            [
                (IF_NAME.child(1), text("ether1")),
                (IF_NAME.child(2).child(9), text("nested")),
                (IF_NAME.child(3), SnmpValue::Integer(3)),
            ]
            .into(),
        );
        assert_eq!(
            run(client(&MockTransport::with_agent(agent)).list_interfaces()),
            Ok(vec![SnmpInterface {
                index: 1,
                name: "ether1".into()
            }])
        );
    }

    #[test]
    fn resolve_interface_indexes_by_name() {
        let transport = router_os();
        let mut client = client(&transport);
        assert_eq!(
            run(client.resolve_interface_indexes(&["wlan1", "pppoe-out1"])),
            Ok(vec![12, 7])
        );
        assert_eq!(
            run(client.resolve_interface_indexes(&["pppoe-out1", "pppoe-out9"])),
            Err(SnmpError::InterfaceNotFound("pppoe-out9".into()))
        );
    }
}
