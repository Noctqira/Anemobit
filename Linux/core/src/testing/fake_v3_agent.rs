use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::ber::BerReader;
use crate::snmp::UsmCredentials;
use crate::snmp::pdu::{Pdu, PduType};
use crate::snmp::v3::crypto;
use crate::snmp::v3::message::{MsgFlags, ScopedData, ScopedPdu, UsmSecurityParameters, V3Message};
use crate::snmp::v3::session::{UsmReport, sign};
use crate::{SnmpValue, VarBind};

use super::FakeAgent;

/// Simulated SNMPv3 agent (USM). MIB lookups go to `FakeAgent`; this handles discovery, authentication, privacy and the various Reports.
/// Clones share state.
#[derive(Clone)]
pub(crate) struct FakeV3Agent {
    credentials: UsmCredentials,
    mib: FakeAgent,
    auth_key: Vec<u8>,
    priv_key: Vec<u8>,
    requests: Rc<RefCell<Vec<V3Message>>>,
    discovery_omits_time: Rc<Cell<bool>>,
}

impl FakeV3Agent {
    pub(crate) const ENGINE_ID: [u8; 7] = [0x80, 0x00, 0x3A, 0x8C, 0x04, 0x50, 0x47];
    pub(crate) const ENGINE_BOOTS: u32 = 5;
    pub(crate) const ENGINE_TIME: u32 = 10_000;

    /// Uses the RouterOS-style interface table.
    pub(crate) fn new(credentials: UsmCredentials) -> Self {
        Self::with_mib(
            credentials,
            FakeAgent::new(FakeAgent::router_os_interface_mib()),
        )
    }

    pub(crate) fn with_mib(credentials: UsmCredentials, mib: FakeAgent) -> Self {
        let auth = credentials.auth_protocol;
        let auth_key = crypto::localized_key(&credentials.auth_password, &Self::ENGINE_ID, auth);
        let priv_key = credentials
            .privacy
            .as_ref()
            .map(|p| crypto::localized_key(&p.password, &Self::ENGINE_ID, auth))
            .unwrap_or_default();
        FakeV3Agent {
            credentials,
            mib,
            auth_key,
            priv_key,
            requests: Rc::default(),
            discovery_omits_time: Rc::default(),
        }
    }

    pub(crate) fn engine_id(&self) -> Vec<u8> {
        Self::ENGINE_ID.to_vec()
    }

    /// Received requests (decoded).
    pub(crate) fn requests(&self) -> Vec<V3Message> {
        self.requests.borrow().clone()
    }

    /// Discovery responses carry the wrong time (like some agents), forcing the client to resend after notInTimeWindows.
    pub(crate) fn set_discovery_omits_time(&self, omits: bool) {
        self.discovery_omits_time.set(omits);
    }

    pub(crate) fn respond(&self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let Ok((message, auth_range)) = V3Message::decode(bytes) else {
            return Vec::new();
        };
        self.requests.borrow_mut().push(message.clone());
        let id = message.msg_id;
        let time = Self::ENGINE_TIME;

        // Discovery
        if message.security.engine_id.is_empty() {
            let time = if self.discovery_omits_time.get() {
                0
            } else {
                time
            };
            return vec![self.report(UsmReport::UnknownEngineIds, id, time, false)];
        }
        if message.security.user_name != self.credentials.user_name.as_bytes() {
            return vec![self.report(UsmReport::UnknownUserNames, id, time, false)];
        }
        let wants_priv = self.credentials.privacy.is_some();
        if !message.flags.contains(MsgFlags::AUTH)
            || message.flags.contains(MsgFlags::PRIV) != wants_priv
        {
            return vec![self.report(UsmReport::UnsupportedSecLevels, id, time, false)];
        }
        let mut zeroed = bytes.to_vec();
        zeroed[auth_range.clone()].fill(0);
        let expected =
            crypto::authentication_code(&self.auth_key, &zeroed, self.credentials.auth_protocol);
        if expected != message.security.auth_params {
            return vec![self.report(UsmReport::WrongDigests, id, time, false)];
        }
        let skew = i64::from(message.security.engine_time) - i64::from(time);
        if message.security.engine_boots != Self::ENGINE_BOOTS || skew.abs() > 150 {
            return vec![self.report(UsmReport::NotInTimeWindows, id, time, true)];
        }

        let scoped = match message.data {
            ScopedData::Plaintext(plain) => plain,
            ScopedData::Encrypted(ciphertext) => match self.decrypt(&ciphertext, &message.security)
            {
                Ok(scoped) => scoped,
                Err(_) => return vec![self.report(UsmReport::DecryptionErrors, id, time, false)],
            },
        };
        let response = self.mib.respond_pdu(&scoped.pdu);
        vec![self.encode(response, id, time)]
    }

    fn decrypt(
        &self,
        ciphertext: &[u8],
        security: &UsmSecurityParameters,
    ) -> Result<ScopedPdu, ()> {
        let privacy = self.credentials.privacy.as_ref().ok_or(())?;
        let plaintext = crypto::decrypt(
            ciphertext,
            privacy.protocol,
            &self.priv_key,
            security.engine_boots,
            security.engine_time,
            &security.priv_params,
        )
        .map_err(|_| ())?;
        ScopedPdu::decode(&mut BerReader::new(&plaintext)).map_err(|_| ())
    }

    fn security(&self, time: u32) -> UsmSecurityParameters {
        UsmSecurityParameters {
            engine_id: self.engine_id(),
            engine_boots: Self::ENGINE_BOOTS,
            engine_time: time,
            user_name: self.credentials.user_name.as_bytes().to_vec(),
            auth_params: vec![0; self.credentials.auth_protocol.mac_length()],
            priv_params: vec![],
        }
    }

    /// An authenticated (and encrypted) response.
    pub(crate) fn encode(&self, pdu: Pdu, msg_id: i32, time: u32) -> Vec<u8> {
        let scoped = ScopedPdu {
            context_engine_id: self.engine_id(),
            context_name: vec![],
            pdu,
        };
        let mut security = self.security(time);
        let (flags, data) = match &self.credentials.privacy {
            Some(privacy) => {
                let (ciphertext, priv_params) = crypto::encrypt(
                    &scoped.encode(),
                    privacy.protocol,
                    &self.priv_key,
                    Self::ENGINE_BOOTS,
                    time,
                    msg_id as u64,
                );
                security.priv_params = priv_params.to_vec();
                (
                    MsgFlags::AUTH | MsgFlags::PRIV,
                    ScopedData::Encrypted(ciphertext),
                )
            }
            None => (MsgFlags::AUTH, ScopedData::Plaintext(scoped)),
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
            range.expect("auth params present"),
            &self.auth_key,
            &self.credentials,
        );
        bytes
    }

    fn report(&self, report: UsmReport, msg_id: i32, time: u32, authenticated: bool) -> Vec<u8> {
        let pdu = Pdu {
            pdu_type: PduType::Report,
            request_id: msg_id,
            error_status: 0,
            error_index: 0,
            var_binds: vec![VarBind {
                oid: report.oid(),
                value: SnmpValue::Counter32(1),
            }],
        };
        let scoped = ScopedPdu {
            context_engine_id: self.engine_id(),
            context_name: vec![],
            pdu,
        };
        if authenticated {
            // Authenticated Reports aren't encrypted (RFC 3414 allows sending Reports as authNoPriv).
            let message = V3Message {
                msg_id,
                flags: MsgFlags::AUTH,
                security: self.security(time),
                data: ScopedData::Plaintext(scoped),
            };
            let (mut bytes, range) = message.encode();
            sign(
                &mut bytes,
                range.expect("auth params present"),
                &self.auth_key,
                &self.credentials,
            );
            return bytes;
        }
        let security = UsmSecurityParameters {
            engine_id: self.engine_id(),
            engine_boots: Self::ENGINE_BOOTS,
            engine_time: time,
            ..Default::default()
        };
        V3Message {
            msg_id,
            flags: MsgFlags::default(),
            security,
            data: ScopedData::Plaintext(scoped),
        }
        .encode()
        .0
    }
}
