//! USM state for one SNMPv3 agent: engine info, localized keys, time sync, salt.

use std::fmt;
use std::time::Instant;

use crate::Oid;
use crate::snmp::pdu::{Pdu, PduType};
use crate::snmp::{SnmpError, UsmCredentials, UsmFailure};

use super::crypto;
use super::message::{MsgFlags, ScopedData, ScopedPdu, UsmSecurityParameters, V3Message};

/// usmStats Reports (RFC 3414 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsmReport {
    UnsupportedSecLevels,
    NotInTimeWindows,
    UnknownUserNames,
    UnknownEngineIds,
    WrongDigests,
    DecryptionErrors,
}

impl UsmReport {
    const ALL: [UsmReport; 6] = [
        Self::UnsupportedSecLevels,
        Self::NotInTimeWindows,
        Self::UnknownUserNames,
        Self::UnknownEngineIds,
        Self::WrongDigests,
        Self::DecryptionErrors,
    ];

    /// 1.3.6.1.6.3.15.1.1.N.0
    pub(crate) fn oid(self) -> Oid {
        let index = Self::ALL
            .iter()
            .position(|&r| r == self)
            .unwrap_or_default() as u32
            + 1;
        Oid::from([1, 3, 6, 1, 6, 3, 15, 1, 1, index, 0])
    }

    pub(crate) fn from_oid(oid: &Oid) -> Option<Self> {
        Self::ALL.into_iter().find(|report| report.oid() == *oid)
    }

    pub(crate) fn error(self) -> SnmpError {
        SnmpError::Authentication(match self {
            Self::UnsupportedSecLevels => UsmFailure::UnsupportedSecurityLevel,
            Self::NotInTimeWindows => UsmFailure::NotInTimeWindow,
            Self::UnknownUserNames => UsmFailure::UnknownUserName,
            Self::UnknownEngineIds => UsmFailure::UnknownEngineId,
            Self::WrongDigests => UsmFailure::WrongDigest,
            Self::DecryptionErrors => UsmFailure::DecryptionError,
        })
    }
}

/// Error for a Report's varbind OID; unrecognized Reports are `UnexpectedResponse`.
pub(crate) fn report_error(oid: Option<&Oid>) -> SnmpError {
    match oid.and_then(UsmReport::from_oid) {
        Some(report) => report.error(),
        None => {
            let name = oid.map_or_else(|| "-".to_owned(), Oid::to_string);
            SnmpError::UnexpectedResponse(format!("report {name}"))
        }
    }
}

/// A decoded incoming message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Incoming {
    Response(Pdu),
    Report {
        oid: Option<Oid>,
        security: UsmSecurityParameters,
    },
}

pub(crate) struct UsmSession {
    credentials: UsmCredentials,
    engine_id: Option<Vec<u8>>,
    engine_boots: u32,
    engine_time_at_reference: u32,
    reference: Instant,
    auth_key: Vec<u8>,
    priv_key: Vec<u8>,
    salt_counter: u64,
}

// Keys and account details must never appear in logs.
impl fmt::Debug for UsmSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsmSession")
            .field("discovered", &self.is_discovered())
            .finish_non_exhaustive()
    }
}

impl UsmSession {
    pub(crate) fn new(credentials: UsmCredentials, now: Instant) -> Self {
        UsmSession {
            credentials,
            engine_id: None,
            engine_boots: 0,
            engine_time_at_reference: 0,
            reference: now,
            auth_key: Vec::new(),
            priv_key: Vec::new(),
            // Random starting value, so reconnecting doesn't always start from the same salt.
            salt_counter: (u64::from(glib::random_int()) << 32) | u64::from(glib::random_int()),
        }
    }

    pub(crate) fn credentials(&self) -> &UsmCredentials {
        &self.credentials
    }

    /// `true` for authPriv (privacy).
    pub(crate) fn is_encrypted(&self) -> bool {
        self.credentials.privacy.is_some()
    }

    pub(crate) fn is_discovered(&self) -> bool {
        self.engine_id.is_some()
    }

    /// RFC 3414 §4: unauthenticated discovery request with an empty engineID.
    pub(crate) fn discovery_message(msg_id: i32) -> Vec<u8> {
        let scoped = ScopedPdu {
            context_engine_id: vec![],
            context_name: vec![],
            pdu: Pdu::get(msg_id, &[]),
        };
        V3Message {
            msg_id,
            flags: MsgFlags::REPORTABLE,
            security: UsmSecurityParameters::default(),
            data: ScopedData::Plaintext(scoped),
        }
        .encode()
        .0
    }

    /// Takes the engineID from the discovery Report and localizes the keys.
    pub(crate) fn complete_discovery(
        &mut self,
        security: &UsmSecurityParameters,
        now: Instant,
    ) -> Result<(), SnmpError> {
        if security.engine_id.is_empty() {
            return Err(SnmpError::UnexpectedResponse(
                "no engine ID in discovery report".into(),
            ));
        }
        let auth = self.credentials.auth_protocol;
        self.auth_key =
            crypto::localized_key(&self.credentials.auth_password, &security.engine_id, auth);
        if let Some(privacy) = &self.credentials.privacy {
            // The privacy key is taken from the localized authentication hash (DES uses 16 bytes, AES-128 the first 16 bytes).
            self.priv_key = crypto::localized_key(&privacy.password, &security.engine_id, auth);
        }
        self.engine_id = Some(security.engine_id.clone());
        self.update_time(security, now);
        Ok(())
    }

    pub(crate) fn update_time(&mut self, security: &UsmSecurityParameters, now: Instant) {
        self.engine_boots = security.engine_boots;
        self.engine_time_at_reference = security.engine_time;
        self.reference = now;
    }

    /// Estimates the agent's current engineTime from locally elapsed time.
    pub(crate) fn engine_time(&self, now: Instant) -> u32 {
        let elapsed = now.saturating_duration_since(self.reference).as_secs();
        u32::try_from(u64::from(self.engine_time_at_reference) + elapsed).unwrap_or(u32::MAX)
    }

    /// Encodes, encrypts and signs a request.
    pub(crate) fn encode(
        &mut self,
        pdu: &Pdu,
        msg_id: i32,
        now: Instant,
    ) -> Result<Vec<u8>, SnmpError> {
        let Some(engine_id) = self.engine_id.clone() else {
            return Err(SnmpError::Authentication(UsmFailure::DiscoveryIncomplete));
        };
        let time = self.engine_time(now);
        let scoped = ScopedPdu {
            context_engine_id: engine_id.clone(),
            context_name: vec![],
            pdu: pdu.clone(),
        };
        let mut security = UsmSecurityParameters {
            engine_id,
            engine_boots: self.engine_boots,
            engine_time: time,
            user_name: self.credentials.user_name.as_bytes().to_vec(),
            auth_params: vec![0; self.credentials.auth_protocol.mac_length()],
            priv_params: vec![],
        };
        let (flags, data) = match &self.credentials.privacy {
            Some(privacy) => {
                self.salt_counter = self.salt_counter.wrapping_add(1);
                let (ciphertext, priv_params) = crypto::encrypt(
                    &scoped.encode(),
                    privacy.protocol,
                    &self.priv_key,
                    self.engine_boots,
                    time,
                    self.salt_counter,
                );
                security.priv_params = priv_params.to_vec();
                (
                    MsgFlags::REPORTABLE | MsgFlags::AUTH | MsgFlags::PRIV,
                    ScopedData::Encrypted(ciphertext),
                )
            }
            None => (
                MsgFlags::REPORTABLE | MsgFlags::AUTH,
                ScopedData::Plaintext(scoped),
            ),
        };
        let (mut bytes, range) = V3Message {
            msg_id,
            flags,
            security,
            data,
        }
        .encode();
        sign(
            &mut bytes,
            range.expect("auth params are present"),
            &self.auth_key,
            &self.credentials,
        );
        Ok(bytes)
    }

    /// Verifies and decrypts an incoming packet. Returns `None` when invalid (wrong HMAC, missing required authentication, etc.) so the caller ignores it.
    pub(crate) fn decode(&mut self, bytes: &[u8], now: Instant) -> Option<(i32, Incoming)> {
        let (message, auth_range) = V3Message::decode(bytes).ok()?;
        let authenticated = message.flags.contains(MsgFlags::AUTH);
        if authenticated {
            let auth = self.credentials.auth_protocol;
            if self.engine_id.as_ref() != Some(&message.security.engine_id)
                || auth_range.len() != auth.mac_length()
            {
                return None;
            }
            let mut zeroed = bytes.to_vec();
            zeroed[auth_range].fill(0);
            if crypto::authentication_code(&self.auth_key, &zeroed, auth)
                != message.security.auth_params
            {
                return None;
            }
        }

        let scoped = match message.data {
            ScopedData::Plaintext(scoped) => scoped,
            ScopedData::Encrypted(ciphertext) => {
                let privacy = self.credentials.privacy.as_ref()?;
                if !authenticated || !message.flags.contains(MsgFlags::PRIV) {
                    return None;
                }
                let security = &message.security;
                let plaintext = crypto::decrypt(
                    &ciphertext,
                    privacy.protocol,
                    &self.priv_key,
                    security.engine_boots,
                    security.engine_time,
                    &security.priv_params,
                )
                .ok()?;
                // DES-decrypted data may carry padding bytes; read only the first SEQUENCE.
                ScopedPdu::decode(&mut crate::ber::BerReader::new(&plaintext)).ok()?
            }
        };

        match scoped.pdu.pdu_type {
            PduType::Report => {
                if authenticated {
                    self.update_time(&message.security, now);
                }
                let oid = scoped.pdu.var_binds.first().map(|bind| bind.oid.clone());
                Some((
                    message.msg_id,
                    Incoming::Report {
                        oid,
                        security: message.security,
                    },
                ))
            }
            // When authentication is configured, unauthenticated responses are never accepted.
            PduType::Response if authenticated => {
                self.update_time(&message.security, now);
                Some((message.msg_id, Incoming::Response(scoped.pdu)))
            }
            _ => None,
        }
    }
}

/// Fills in the HMAC of the whole packet at `range` (authParams).
pub(crate) fn sign(
    bytes: &mut [u8],
    range: std::ops::Range<usize>,
    key: &[u8],
    credentials: &UsmCredentials,
) {
    let mac = crypto::authentication_code(key, bytes, credentials.auth_protocol);
    bytes[range].copy_from_slice(&mac);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::oids::SYS_UP_TIME;
    use crate::snmp::UsmPrivacy;
    use crate::snmp::v3::{AuthProtocol, PrivProtocol};
    use crate::testing::FakeV3Agent;

    fn credentials() -> UsmCredentials {
        UsmCredentials {
            user_name: "anemobit".into(),
            auth_protocol: AuthProtocol::Sha1,
            auth_password: "authpass12".into(),
            privacy: Some(UsmPrivacy {
                protocol: PrivProtocol::Aes,
                password: "privpass12".into(),
            }),
        }
    }

    fn discovered(agent: &FakeV3Agent, now: Instant) -> UsmSession {
        let mut session = UsmSession::new(credentials(), now);
        let report = agent.respond(&UsmSession::discovery_message(1)).remove(0);
        let Some((1, Incoming::Report { security, .. })) = session.decode(&report, now) else {
            panic!("discovery should yield a report");
        };
        session.complete_discovery(&security, now).unwrap();
        session
    }

    #[test]
    fn report_oids_map_to_errors() {
        let base = "1.3.6.1.6.3.15.1.1";
        let cases = [
            (1, UsmFailure::UnsupportedSecurityLevel),
            (2, UsmFailure::NotInTimeWindow),
            (3, UsmFailure::UnknownUserName),
            (4, UsmFailure::UnknownEngineId),
            (5, UsmFailure::WrongDigest),
            (6, UsmFailure::DecryptionError),
        ];
        for (n, failure) in cases {
            let oid: Oid = format!("{base}.{n}.0").parse().unwrap();
            assert_eq!(
                UsmReport::from_oid(&oid).map(UsmReport::oid),
                Some(oid.clone())
            );
            assert_eq!(report_error(Some(&oid)), SnmpError::Authentication(failure));
        }
        assert!(matches!(
            report_error(Some(&SYS_UP_TIME)),
            SnmpError::UnexpectedResponse(_)
        ));
        assert!(matches!(
            report_error(None),
            SnmpError::UnexpectedResponse(_)
        ));
    }

    #[test]
    fn encode_requires_discovery() {
        let mut session = UsmSession::new(credentials(), Instant::now());
        assert!(!session.is_discovered());
        assert_eq!(
            session.encode(&Pdu::get(1, &[]), 1, Instant::now()),
            Err(SnmpError::Authentication(UsmFailure::DiscoveryIncomplete))
        );
    }

    #[test]
    fn engine_time_advances_with_local_clock() {
        let now = Instant::now();
        let agent = FakeV3Agent::new(credentials());
        let session = discovered(&agent, now);
        assert!(session.is_discovered());
        assert_eq!(session.engine_time(now), FakeV3Agent::ENGINE_TIME);
        assert_eq!(
            session.engine_time(now + Duration::from_millis(2_900)),
            FakeV3Agent::ENGINE_TIME + 2
        );
    }

    #[test]
    fn discovery_without_engine_id_fails() {
        let mut session = UsmSession::new(credentials(), Instant::now());
        assert!(matches!(
            session.complete_discovery(&UsmSecurityParameters::default(), Instant::now()),
            Err(SnmpError::UnexpectedResponse(_))
        ));
    }

    #[test]
    fn requests_are_authenticated_and_encrypted() {
        let now = Instant::now();
        let agent = FakeV3Agent::new(credentials());
        let mut session = discovered(&agent, now);
        let request = session
            .encode(&Pdu::get(9, std::slice::from_ref(&*SYS_UP_TIME)), 9, now)
            .unwrap();
        let (message, _) = V3Message::decode(&request).unwrap();
        assert_eq!(
            message.flags,
            MsgFlags::REPORTABLE | MsgFlags::AUTH | MsgFlags::PRIV
        );
        assert!(matches!(message.data, ScopedData::Encrypted(_)));
        // The agent only responds after verifying and decrypting successfully.
        let reply = agent.respond(&request).remove(0);
        assert!(matches!(
            session.decode(&reply, now),
            Some((9, Incoming::Response(_)))
        ));
    }

    #[test]
    fn salt_changes_between_requests() {
        let now = Instant::now();
        let agent = FakeV3Agent::new(credentials());
        let mut session = discovered(&agent, now);
        let pdu = Pdu::get(1, &[]);
        let first = V3Message::decode(&session.encode(&pdu, 1, now).unwrap())
            .unwrap()
            .0;
        let second = V3Message::decode(&session.encode(&pdu, 1, now).unwrap())
            .unwrap()
            .0;
        assert_ne!(first.security.priv_params, second.security.priv_params);
    }

    #[test]
    fn unauthenticated_response_is_ignored() {
        let now = Instant::now();
        let agent = FakeV3Agent::new(credentials());
        let mut session = discovered(&agent, now);
        let unauthenticated = V3Message {
            msg_id: 3,
            flags: MsgFlags::default(),
            security: UsmSecurityParameters {
                engine_id: agent.engine_id(),
                ..Default::default()
            },
            data: ScopedData::Plaintext(ScopedPdu {
                context_engine_id: agent.engine_id(),
                context_name: vec![],
                pdu: Pdu {
                    pdu_type: PduType::Response,
                    ..Pdu::get(3, &[])
                },
            }),
        };
        assert_eq!(session.decode(&unauthenticated.encode().0, now), None);
    }

    #[test]
    fn authenticated_message_from_other_engine_is_ignored() {
        let now = Instant::now();
        let agent = FakeV3Agent::new(credentials());
        let mut session = discovered(&agent, now);
        let request = session.encode(&Pdu::get(4, &[]), 4, now).unwrap();
        let mut reply = agent.respond(&request).remove(0);
        // Change the last byte of the engineID (so the HMAC no longer matches either).
        let (message, _) = V3Message::decode(&reply).unwrap();
        let id = message.security.engine_id;
        let at = reply
            .windows(id.len())
            .position(|w| w == id.as_slice())
            .unwrap()
            + id.len()
            - 1;
        reply[at] ^= 0x01;
        assert_eq!(session.decode(&reply, now), None);
    }
}
