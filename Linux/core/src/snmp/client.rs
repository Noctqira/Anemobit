use std::fmt;
use std::time::{Duration, Instant};

use crate::transport::{Transport, UdpTransport};
use crate::{Oid, VarBind};

use super::message::SnmpMessage;
use super::pdu::{Pdu, PduType};
use super::v3::session::{Incoming, UsmReport, UsmSession, report_error};
use super::{SnmpError, SnmpSecurity, UsmFailure};

/// Minimal SNMP client: v2c community or SNMPv3 USM; request-id / msgID matching, timeouts and retries.
///
/// One client per transport, closed together on drop. Request methods all take `&mut self`,
/// so only one request is in flight at a time; dropping an in-flight future cancels that request.
/// Async waiting uses the glib event loop, so this must run on a `glib::MainContext`.
pub struct SnmpClient<T: Transport = UdpTransport> {
    transport: T,
    mode: Mode,
    timeout: Duration,
    retries: u32,
    next_request_id: i32,
}

/// State that depends on the security settings.
enum Mode {
    Community(String),
    /// SNMPv3: engine discovery before the first request, then the session's keys and time are reused.
    Usm(Box<UsmSession>),
}

// The community and keys must never appear in logs.
impl<T: Transport> fmt::Debug for SnmpClient<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let version = match self.mode {
            Mode::Community(_) => "v2c",
            Mode::Usm(_) => "v3",
        };
        f.debug_struct("SnmpClient")
            .field("version", &version)
            .field("timeout", &self.timeout)
            .field("retries", &self.retries)
            .finish_non_exhaustive()
    }
}

impl<T: Transport> SnmpClient<T> {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);
    pub const DEFAULT_RETRIES: u32 = 1;

    /// Uses the default timeout (2 seconds) and retry count (1).
    pub fn new(transport: T, security: SnmpSecurity) -> Self {
        let mode = match security {
            SnmpSecurity::Community(community) => Mode::Community(community),
            SnmpSecurity::Usm(credentials) => {
                Mode::Usm(Box::new(UsmSession::new(credentials, Instant::now())))
            }
        };
        SnmpClient {
            transport,
            mode,
            timeout: Self::DEFAULT_TIMEOUT,
            retries: Self::DEFAULT_RETRIES,
            // Random starting value, so request-ids after reconnecting don't collide with old requests.
            next_request_id: glib::random_int_range(1, i32::MAX / 2),
        }
    }

    /// How long to wait for a reply after each send.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// How many times to resend after a timeout (0 means send only once).
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    pub async fn get(&mut self, oids: &[Oid]) -> Result<Vec<VarBind>, SnmpError> {
        Ok(self.request(Pdu::get(0, oids)).await?.var_binds)
    }

    pub async fn get_next(&mut self, oids: &[Oid]) -> Result<Vec<VarBind>, SnmpError> {
        Ok(self.request(Pdu::get_next(0, oids)).await?.var_binds)
    }

    pub async fn get_bulk(
        &mut self,
        oids: &[Oid],
        non_repeaters: u32,
        max_repetitions: u32,
    ) -> Result<Vec<VarBind>, SnmpError> {
        let pdu = Pdu::get_bulk(0, non_repeaters, max_repetitions, oids);
        Ok(self.request(pdu).await?.var_binds)
    }

    async fn request(&mut self, pdu: Pdu) -> Result<Pdu, SnmpError> {
        let response = match self.mode {
            Mode::Community(_) => self.community_request(pdu).await?,
            Mode::Usm(_) => self.usm_request(pdu).await?,
        };
        if response.error_status != 0 {
            return Err(SnmpError::ErrorStatus {
                status: response.error_status,
                index: response.error_index,
            });
        }
        Ok(response)
    }

    async fn community_request(&mut self, mut pdu: Pdu) -> Result<Pdu, SnmpError> {
        pdu.request_id = self.make_request_id();
        let id = pdu.request_id;
        let Mode::Community(community) = &self.mode else {
            unreachable!("community mode")
        };
        let datagram = SnmpMessage::new(community.as_bytes(), pdu).encode();
        exchange(
            &self.transport,
            self.timeout,
            self.retries,
            &datagram,
            |reply| {
                let message = SnmpMessage::decode(reply).ok()?;
                let matches =
                    message.pdu.pdu_type == PduType::Response && message.pdu.request_id == id;
                matches.then_some(message.pdu)
            },
        )
        .await
    }

    async fn usm_request(&mut self, mut pdu: Pdu) -> Result<Pdu, SnmpError> {
        self.discover_if_needed().await?;
        // If the first attempt reports out-of-sync time, update the time and resend once.
        for attempt in 0..2 {
            pdu.request_id = self.make_request_id();
            let id = pdu.request_id;
            let Mode::Usm(session) = &mut self.mode else {
                unreachable!("USM mode")
            };
            let encrypted = session.is_encrypted();
            let datagram = session.encode(&pdu, id, Instant::now())?;
            let result = exchange(
                &self.transport,
                self.timeout,
                self.retries,
                &datagram,
                |reply| accept_v3(session, reply, id),
            )
            .await;
            let incoming = match result {
                // Discovery succeeded (the device responds) but the encrypted request got no reply: usually the agent failed to decrypt and dropped the packet.
                Err(SnmpError::Timeout) if encrypted => {
                    return Err(SnmpError::Authentication(
                        UsmFailure::NoResponseToEncryptedRequest,
                    ));
                }
                result => result?,
            };
            match incoming {
                Incoming::Response(response) => return Ok(response),
                Incoming::Report { oid, security }
                    if attempt == 0
                        && oid.as_ref().and_then(UsmReport::from_oid)
                            == Some(UsmReport::NotInTimeWindows) =>
                {
                    session.update_time(&security, Instant::now());
                }
                Incoming::Report { oid, .. } => return Err(report_error(oid.as_ref())),
            }
        }
        Err(UsmReport::NotInTimeWindows.error())
    }

    /// SNMPv3 engine discovery (RFC 3414 §4). Sends nothing if the account settings are invalid.
    async fn discover_if_needed(&mut self) -> Result<(), SnmpError> {
        let Mode::Usm(session) = &self.mode else {
            return Ok(());
        };
        if let Some(failure) = session.credentials().validation_failure() {
            return Err(SnmpError::Authentication(failure));
        }
        if session.is_discovered() {
            return Ok(());
        }
        let id = self.make_request_id();
        let Mode::Usm(session) = &mut self.mode else {
            unreachable!("USM mode")
        };
        let datagram = UsmSession::discovery_message(id);
        let incoming = exchange(
            &self.transport,
            self.timeout,
            self.retries,
            &datagram,
            |reply| accept_v3(session, reply, id),
        )
        .await?;
        match incoming {
            Incoming::Report { security, .. } => {
                session.complete_discovery(&security, Instant::now())
            }
            Incoming::Response(_) => Err(SnmpError::UnexpectedResponse(
                "discovery got a response instead of a report".into(),
            )),
        }
    }

    fn make_request_id(&mut self) -> i32 {
        let id = self.next_request_id;
        self.next_request_id = if id == i32::MAX { 1 } else { id + 1 };
        id
    }
}

/// Verifies and decrypts an incoming SNMPv3 packet, accepting it only if the msgID matches.
fn accept_v3(session: &mut UsmSession, reply: &[u8], id: i32) -> Option<Incoming> {
    let (msg_id, incoming) = session.decode(reply, Instant::now())?;
    (msg_id == id).then_some(incoming)
}

/// Sends `datagram` and waits for a reply that `accept` accepts; resends on timeout (the same packet),
/// and returns `Timeout` when retries run out. Packets for which `accept` returns `None` (malformed,
/// not a response, mismatched id) are ignored.
async fn exchange<T: Transport, R>(
    transport: &T,
    timeout: Duration,
    retries: u32,
    datagram: &[u8],
    mut accept: impl FnMut(&[u8]) -> Option<R>,
) -> Result<R, SnmpError> {
    for _ in 0..=retries {
        transport
            .send(datagram)
            .await
            .map_err(|e| SnmpError::Transport(e.to_string()))?;
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            // glib timers count in milliseconds (truncated) and may fire slightly before the deadline: go back and check again.
            let Ok(received) = glib::future_with_timeout(remaining, transport.receive()).await
            else {
                continue;
            };
            let reply = received.map_err(|e| SnmpError::Transport(e.to_string()))?;
            if let Some(result) = accept(&reply) {
                return Ok(result);
            }
        }
    }
    Err(SnmpError::Timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SnmpValue;
    use crate::oids::{IF_HC_IN_OCTETS, SYS_UP_TIME};
    use crate::testing::{FakeAgent, MockTransport, response, response_with, run};

    const SHORT_TIMEOUT: Duration = Duration::from_millis(50);

    fn client(transport: &MockTransport) -> SnmpClient<MockTransport> {
        SnmpClient::new(transport.clone(), SnmpSecurity::Community("public".into()))
            .with_timeout(SHORT_TIMEOUT)
    }

    fn up_time(ticks: u32) -> VarBind {
        VarBind {
            oid: SYS_UP_TIME.clone(),
            value: SnmpValue::TimeTicks(ticks),
        }
    }

    fn sys_up_time() -> Vec<Oid> {
        vec![SYS_UP_TIME.clone()]
    }

    #[test]
    fn get_returns_values() {
        let transport =
            MockTransport::v2c(|request, _| vec![response(request, vec![up_time(4242)])]);
        let binds = run(client(&transport).get(&sys_up_time())).unwrap();
        assert_eq!(binds, [up_time(4242)]);
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].community, b"public");
        assert_eq!(sent[0].pdu.pdu_type, PduType::Get);
    }

    #[test]
    fn get_next_and_get_bulk_use_their_pdu_types() {
        let agent = FakeAgent::new(FakeAgent::router_os_interface_mib());
        let transport = MockTransport::with_agent(agent);
        let mut client = client(&transport);
        run(client.get_next(&sys_up_time())).unwrap();
        run(client.get_bulk(&sys_up_time(), 0, 5)).unwrap();
        let sent = transport.sent();
        assert_eq!(sent[0].pdu.pdu_type, PduType::GetNext);
        assert_eq!(sent[1].pdu.pdu_type, PduType::GetBulk);
        assert_eq!((sent[1].pdu.error_status, sent[1].pdu.error_index), (0, 5));
    }

    #[test]
    fn ignores_response_with_mismatched_request_id() {
        let transport = MockTransport::v2c(|request, _| {
            let wrong_id = request.pdu.request_id.wrapping_add(1);
            vec![
                response_with(request, wrong_id, 0, 0, vec![up_time(1)]),
                response(request, vec![up_time(2)]),
            ]
        });
        let binds = run(client(&transport).get(&sys_up_time())).unwrap();
        assert_eq!(binds, [up_time(2)]);
    }

    #[test]
    fn only_mismatched_responses_lead_to_timeout() {
        let transport = MockTransport::v2c(|request, _| {
            vec![response_with(
                request,
                request.pdu.request_id.wrapping_add(1),
                0,
                0,
                vec![],
            )]
        });
        assert_eq!(
            run(client(&transport).get(&sys_up_time())),
            Err(SnmpError::Timeout)
        );
    }

    #[test]
    fn ignores_garbage_and_non_response_pdus() {
        let transport = MockTransport::v2c(|request, _| {
            vec![
                vec![0xDE, 0xAD],
                request.encode(), // Echo of our own GetRequest
                response(request, vec![up_time(9)]),
            ]
        });
        assert_eq!(
            run(client(&transport).get(&sys_up_time())),
            Ok(vec![up_time(9)])
        );
    }

    #[test]
    fn retries_after_timeout_with_same_request_id() {
        let transport = MockTransport::v2c(|request, count| {
            // The first packet is lost; only the second gets a reply.
            if count < 2 {
                vec![]
            } else {
                vec![response(request, vec![up_time(7)])]
            }
        });
        let binds = run(client(&transport).with_retries(1).get(&sys_up_time())).unwrap();
        assert_eq!(binds, [up_time(7)]);
        let sent = transport.sent();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].pdu.request_id, sent[1].pdu.request_id);
    }

    #[test]
    fn times_out_after_retries_are_exhausted() {
        let transport = MockTransport::silent();
        let started = Instant::now();
        let result = run(client(&transport).with_retries(2).get(&sys_up_time()));
        assert_eq!(result, Err(SnmpError::Timeout));
        assert_eq!(transport.sent().len(), 3);
        assert!(started.elapsed() >= SHORT_TIMEOUT * 3);
    }

    #[test]
    fn defaults_follow_the_spec() {
        let client = SnmpClient::new(MockTransport::silent(), SnmpSecurity::Community("c".into()));
        assert_eq!(client.timeout, Duration::from_secs(2));
        assert_eq!(client.retries, 1);
    }

    #[test]
    fn error_status_is_returned_as_error() {
        let transport = MockTransport::v2c(|request, _| {
            vec![response_with(
                request,
                request.pdu.request_id,
                2,
                1,
                request.pdu.var_binds.clone(),
            )]
        });
        assert_eq!(
            run(client(&transport).get(&sys_up_time())),
            Err(SnmpError::ErrorStatus {
                status: 2,
                index: 1
            })
        );
    }

    #[test]
    fn no_such_instance_is_returned_as_value() {
        let agent = FakeAgent::new([(SYS_UP_TIME.clone(), SnmpValue::TimeTicks(1))].into());
        let transport = MockTransport::with_agent(agent);
        let binds = run(client(&transport).get(&[IF_HC_IN_OCTETS.child(99)])).unwrap();
        assert_eq!(binds[0].value, SnmpValue::NoSuchInstance);
    }

    #[test]
    fn request_ids_are_unique_per_request() {
        let agent = FakeAgent::new([(SYS_UP_TIME.clone(), SnmpValue::TimeTicks(1))].into());
        let transport = MockTransport::with_agent(agent);
        let mut client = client(&transport);
        run(client.get(&sys_up_time())).unwrap();
        run(client.get(&sys_up_time())).unwrap();
        let sent = transport.sent();
        assert_ne!(sent[0].pdu.request_id, sent[1].pdu.request_id);
        assert!(sent[0].pdu.request_id > 0);
    }

    #[test]
    fn request_id_wraps_to_one_after_i32_max() {
        let agent = FakeAgent::new([(SYS_UP_TIME.clone(), SnmpValue::TimeTicks(1))].into());
        let transport = MockTransport::with_agent(agent);
        let mut client = client(&transport);
        client.next_request_id = i32::MAX;
        run(client.get(&sys_up_time())).unwrap();
        run(client.get(&sys_up_time())).unwrap();
        let ids: Vec<_> = transport.sent().iter().map(|m| m.pdu.request_id).collect();
        assert_eq!(ids, [i32::MAX, 1]);
    }

    #[test]
    fn send_failure_is_a_transport_error() {
        let transport = MockTransport::silent();
        transport.fail_sends("boom");
        assert!(matches!(
            run(client(&transport).get(&sys_up_time())),
            Err(SnmpError::Transport(_))
        ));
    }

    #[test]
    fn incoming_failure_fails_the_request() {
        let transport = MockTransport::silent();
        transport.fail_incoming("boom");
        let mut client = client(&transport).with_timeout(Duration::from_secs(5));
        let started = Instant::now();
        assert!(matches!(
            run(client.get(&sys_up_time())),
            Err(SnmpError::Transport(_))
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn dropping_the_future_cancels_and_client_stays_usable() {
        // Don't answer the first request; answer later requests normally.
        let transport = MockTransport::v2c(|request, count| {
            if count == 1 {
                vec![]
            } else {
                vec![response(request, vec![up_time(2)])]
            }
        });
        let mut client = client(&transport).with_timeout(Duration::from_secs(10));
        let started = Instant::now();
        let cancelled = run(glib::future_with_timeout(
            Duration::from_millis(20),
            client.get(&sys_up_time()),
        ));
        assert!(cancelled.is_err());
        assert!(started.elapsed() < Duration::from_secs(1));

        // A late reply to the cancelled request must be ignored, and the next request gets its own reply.
        transport.deliver(response(&transport.sent()[0], vec![up_time(1)]));
        assert_eq!(run(client.get(&sys_up_time())), Ok(vec![up_time(2)]));
    }
}

#[cfg(test)]
mod v3_tests {
    use super::*;
    use crate::oids::SYS_UP_TIME;
    use crate::snmp::v3::message::{MsgFlags, ScopedData, V3Message};
    use crate::snmp::{UsmCredentials, UsmPrivacy};
    use crate::testing::{FakeV3Agent, MockTransport, run};
    use crate::{AuthProtocol, PrivProtocol, SnmpValue};

    fn credentials(auth: AuthProtocol, privacy: Option<PrivProtocol>) -> UsmCredentials {
        credentials_with(auth, privacy, "anemobit", "authpass12", "privpass12")
    }

    fn credentials_with(
        auth: AuthProtocol,
        privacy: Option<PrivProtocol>,
        user: &str,
        auth_password: &str,
        priv_password: &str,
    ) -> UsmCredentials {
        UsmCredentials {
            user_name: user.into(),
            auth_protocol: auth,
            auth_password: auth_password.into(),
            privacy: privacy.map(|protocol| UsmPrivacy {
                protocol,
                password: priv_password.into(),
            }),
        }
    }

    fn client(transport: &MockTransport, credentials: UsmCredentials) -> SnmpClient<MockTransport> {
        SnmpClient::new(transport.clone(), SnmpSecurity::Usm(credentials))
            .with_timeout(Duration::from_millis(50))
            .with_retries(0)
    }

    /// Transport that only answers discovery (simulating an agent silently dropping encrypted requests).
    fn answers_only_discovery(agent: FakeV3Agent) -> MockTransport {
        MockTransport::raw(move |bytes, _| match V3Message::decode(bytes) {
            Ok((message, _)) if message.security.engine_id.is_empty() => agent.respond(bytes),
            _ => vec![],
        })
    }

    fn sys_up_time() -> Vec<Oid> {
        vec![SYS_UP_TIME.clone()]
    }

    #[test]
    fn discovery_then_authenticated_get() {
        let cases = [
            (AuthProtocol::Md5, None),
            (AuthProtocol::Sha1, None),
            (AuthProtocol::Md5, Some(PrivProtocol::Des)),
            (AuthProtocol::Md5, Some(PrivProtocol::Aes)),
            (AuthProtocol::Sha1, Some(PrivProtocol::Des)),
            (AuthProtocol::Sha1, Some(PrivProtocol::Aes)),
            (AuthProtocol::Sha224, None),
            (AuthProtocol::Sha224, Some(PrivProtocol::Aes)),
            (AuthProtocol::Sha256, Some(PrivProtocol::Aes)),
            (AuthProtocol::Sha384, Some(PrivProtocol::Des)),
            (AuthProtocol::Sha512, Some(PrivProtocol::Aes)),
        ];
        for (auth, privacy) in cases {
            let creds = credentials(auth, privacy);
            let agent = FakeV3Agent::new(creds.clone());
            let transport = MockTransport::with_v3_agent(agent.clone());
            let mut client = client(&transport, creds);

            let binds = run(client.get(&sys_up_time())).unwrap();
            assert_eq!(
                binds,
                [VarBind {
                    oid: SYS_UP_TIME.clone(),
                    value: SnmpValue::TimeTicks(100)
                }]
            );

            let requests = agent.requests();
            assert_eq!(requests.len(), 2, "{auth:?} {privacy:?}");
            assert_eq!(requests[0].flags, MsgFlags::REPORTABLE);
            assert!(requests[0].security.engine_id.is_empty());
            assert!(requests[1].flags.contains(MsgFlags::AUTH));
            assert_eq!(
                requests[1].flags.contains(MsgFlags::PRIV),
                privacy.is_some()
            );
            assert_eq!(requests[1].security.engine_id, agent.engine_id());
            assert_eq!(
                matches!(requests[1].data, ScopedData::Encrypted(_)),
                privacy.is_some()
            );

            // The second request doesn't run discovery again.
            run(client.get(&sys_up_time())).unwrap();
            assert_eq!(agent.requests().len(), 3);
        }
    }

    #[test]
    fn walk_and_resolve_over_v3() {
        let creds = credentials(AuthProtocol::Sha1, Some(PrivProtocol::Aes));
        let transport = MockTransport::with_v3_agent(FakeV3Agent::new(creds.clone()));
        assert_eq!(
            run(client(&transport, creds).resolve_interface_indexes(&["pppoe-out1"])),
            Ok(vec![7])
        );
    }

    fn assert_failure(
        agent_creds: UsmCredentials,
        client_creds: UsmCredentials,
        expected: UsmFailure,
    ) {
        let transport = MockTransport::with_v3_agent(FakeV3Agent::new(agent_creds));
        assert_eq!(
            run(client(&transport, client_creds).get(&sys_up_time())),
            Err(SnmpError::Authentication(expected))
        );
    }

    #[test]
    fn unknown_user_is_reported() {
        assert_failure(
            credentials(AuthProtocol::Sha1, Some(PrivProtocol::Aes)),
            credentials_with(
                AuthProtocol::Sha1,
                Some(PrivProtocol::Aes),
                "nobody",
                "authpass12",
                "privpass12",
            ),
            UsmFailure::UnknownUserName,
        );
    }

    #[test]
    fn wrong_auth_password_is_reported() {
        assert_failure(
            credentials(AuthProtocol::Sha1, Some(PrivProtocol::Aes)),
            credentials_with(
                AuthProtocol::Sha1,
                Some(PrivProtocol::Aes),
                "anemobit",
                "wrongpass99",
                "privpass12",
            ),
            UsmFailure::WrongDigest,
        );
    }

    #[test]
    fn wrong_auth_protocol_is_reported() {
        assert_failure(
            credentials(AuthProtocol::Sha1, None),
            credentials(AuthProtocol::Md5, None),
            UsmFailure::WrongDigest,
        );
    }

    #[test]
    fn wrong_priv_password_is_reported() {
        for privacy in [PrivProtocol::Des, PrivProtocol::Aes] {
            assert_failure(
                credentials(AuthProtocol::Sha1, Some(privacy)),
                credentials_with(
                    AuthProtocol::Sha1,
                    Some(privacy),
                    "anemobit",
                    "authpass12",
                    "wrongpriv99",
                ),
                UsmFailure::DecryptionError,
            );
        }
    }

    #[test]
    fn security_level_mismatch_is_reported() {
        assert_failure(
            credentials(AuthProtocol::Sha1, Some(PrivProtocol::Aes)),
            credentials(AuthProtocol::Sha1, None),
            UsmFailure::UnsupportedSecurityLevel,
        );
    }

    #[test]
    fn silently_dropped_encrypted_request_hints_at_privacy_settings() {
        // Real agents (net-snmp, RouterOS) often drop the packet without a Report when decryption fails.
        let creds = credentials(AuthProtocol::Sha1, Some(PrivProtocol::Aes));
        let transport = answers_only_discovery(FakeV3Agent::new(creds.clone()));
        assert_eq!(
            run(client(&transport, creds).get(&sys_up_time())),
            Err(SnmpError::Authentication(
                UsmFailure::NoResponseToEncryptedRequest
            ))
        );
    }

    #[test]
    fn auth_no_priv_timeout_stays_timeout() {
        let creds = credentials(AuthProtocol::Sha1, None);
        let transport = answers_only_discovery(FakeV3Agent::new(creds.clone()));
        assert_eq!(
            run(client(&transport, creds).get(&sys_up_time())),
            Err(SnmpError::Timeout)
        );
    }

    #[test]
    fn not_in_time_window_is_retried_once() {
        let creds = credentials(AuthProtocol::Md5, Some(PrivProtocol::Des));
        let agent = FakeV3Agent::new(creds.clone());
        agent.set_discovery_omits_time(true);
        let transport = MockTransport::with_v3_agent(agent.clone());

        let binds = run(client(&transport, creds).get(&sys_up_time())).unwrap();
        assert_eq!(binds[0].value, SnmpValue::TimeTicks(100));
        // Discovery, rejection (time out of sync), then resend after updating the time.
        let requests = agent.requests();
        assert_eq!(requests.len(), 3);
        assert!(requests[2].security.engine_time >= FakeV3Agent::ENGINE_TIME);
    }

    #[test]
    fn response_with_bad_mac_is_ignored() {
        let creds = credentials(AuthProtocol::Sha1, None);
        let agent = FakeV3Agent::new(creds.clone());
        let transport = MockTransport::raw(move |bytes, _| {
            agent
                .respond(bytes)
                .into_iter()
                .map(|mut reply| {
                    // Discovery responses aren't authenticated and pass through unchanged; others get a tampered HMAC.
                    if let Ok((message, range)) = V3Message::decode(&reply)
                        && message.flags.contains(MsgFlags::AUTH)
                    {
                        reply[range.start] ^= 0xFF;
                    }
                    reply
                })
                .collect()
        });
        assert_eq!(
            run(client(&transport, creds).get(&sys_up_time())),
            Err(SnmpError::Timeout)
        );
    }

    #[test]
    fn short_password_is_rejected_before_sending() {
        let creds = credentials_with(
            AuthProtocol::Sha1,
            Some(PrivProtocol::Aes),
            "anemobit",
            "authpass12",
            "short",
        );
        let transport = MockTransport::silent();
        assert_eq!(
            run(client(&transport, creds).get(&sys_up_time())),
            Err(SnmpError::Authentication(
                UsmFailure::PrivPasswordTooShort { minimum: 8 }
            ))
        );
        assert!(transport.sent_raw().is_empty());
    }

    #[test]
    fn debug_does_not_reveal_secrets() {
        let creds = credentials(AuthProtocol::Sha1, Some(PrivProtocol::Aes));
        let text = format!("{:?}", client(&MockTransport::silent(), creds));
        for secret in ["anemobit", "authpass12", "privpass12"] {
            assert!(!text.contains(secret), "{text}");
        }
    }
}
