use std::fmt;

use crate::ber::BerError;

/// RFC 3416 error-status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorStatus {
    NoError,
    TooBig,
    NoSuchName,
    BadValue,
    ReadOnly,
    GenErr,
    NoAccess,
    WrongType,
    WrongLength,
    WrongEncoding,
    WrongValue,
    NoCreation,
    InconsistentValue,
    ResourceUnavailable,
    CommitFailed,
    UndoFailed,
    AuthorizationError,
    NotWritable,
    InconsistentName,
}

impl ErrorStatus {
    const ALL: [ErrorStatus; 19] = [
        Self::NoError,
        Self::TooBig,
        Self::NoSuchName,
        Self::BadValue,
        Self::ReadOnly,
        Self::GenErr,
        Self::NoAccess,
        Self::WrongType,
        Self::WrongLength,
        Self::WrongEncoding,
        Self::WrongValue,
        Self::NoCreation,
        Self::InconsistentValue,
        Self::ResourceUnavailable,
        Self::CommitFailed,
        Self::UndoFailed,
        Self::AuthorizationError,
        Self::NotWritable,
        Self::InconsistentName,
    ];

    /// Value in the protocol; `None` when outside the RFC 3416 range.
    pub fn from_code(code: i64) -> Option<Self> {
        usize::try_from(code)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }

    pub fn code(self) -> i64 {
        // ALL is ordered by RFC 3416 value.
        Self::ALL
            .iter()
            .position(|&s| s == self)
            .unwrap_or_default() as i64
    }

    /// Name in RFC 3416 (e.g. `noSuchName`).
    pub fn name(self) -> &'static str {
        match self {
            Self::NoError => "noError",
            Self::TooBig => "tooBig",
            Self::NoSuchName => "noSuchName",
            Self::BadValue => "badValue",
            Self::ReadOnly => "readOnly",
            Self::GenErr => "genErr",
            Self::NoAccess => "noAccess",
            Self::WrongType => "wrongType",
            Self::WrongLength => "wrongLength",
            Self::WrongEncoding => "wrongEncoding",
            Self::WrongValue => "wrongValue",
            Self::NoCreation => "noCreation",
            Self::InconsistentValue => "inconsistentValue",
            Self::ResourceUnavailable => "resourceUnavailable",
            Self::CommitFailed => "commitFailed",
            Self::UndoFailed => "undoFailed",
            Self::AuthorizationError => "authorizationError",
            Self::NotWritable => "notWritable",
            Self::InconsistentName => "inconsistentName",
        }
    }
}

/// Error from an SNMP operation.
///
/// `Display` is English for logs; the app translates UI text by error kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnmpError {
    /// Malformed packet.
    Malformed(BerError),
    UnsupportedVersion(i64),
    UnexpectedPduType(u8),
    /// The response has a non-zero error-status. `index` is the 1-based varbind position (0 means the whole request).
    ErrorStatus {
        status: i64,
        index: i64,
    },
    /// All retries timed out.
    Timeout,
    /// The response's varbinds don't match the request, or a required value is missing.
    UnexpectedResponse(String),
    /// No interface with the given name.
    InterfaceNotFound(String),
    /// Transport error (connection failed, socket closed, etc.).
    Transport(String),
    /// SNMPv3 authentication / privacy failure.
    Authentication(UsmFailure),
}

/// Why SNMPv3 (USM) authentication or privacy failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UsmFailure {
    UnknownUserName,
    WrongDigest,
    DecryptionError,
    UnsupportedSecurityLevel,
    NotInTimeWindow,
    UnknownEngineId,
    /// Discovery succeeded but the encrypted request got no reply: usually the agent failed to decrypt and dropped the packet.
    NoResponseToEncryptedRequest,
    DiscoveryIncomplete,
    AuthPasswordTooShort {
        minimum: usize,
    },
    PrivPasswordTooShort {
        minimum: usize,
    },
    InvalidPrivacyParameters,
}

impl From<BerError> for SnmpError {
    fn from(error: BerError) -> Self {
        SnmpError::Malformed(error)
    }
}

impl fmt::Display for SnmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => write!(f, "Malformed packet ({error})"),
            Self::UnsupportedVersion(version) => write!(f, "Unsupported SNMP version {version}"),
            Self::UnexpectedPduType(pdu_type) => write!(f, "Unexpected PDU type 0x{pdu_type:02x}"),
            Self::ErrorStatus { status, index } => match ErrorStatus::from_code(*status) {
                Some(known) => write!(f, "SNMP error {} (varbind {index})", known.name()),
                None => write!(f, "SNMP error {status} (varbind {index})"),
            },
            Self::Timeout => write!(f, "Connection timed out"),
            Self::UnexpectedResponse(detail) => write!(f, "Unexpected response: {detail}"),
            Self::InterfaceNotFound(name) => write!(f, "Interface {name} not found"),
            Self::Transport(detail) => write!(f, "Connection error: {detail}"),
            Self::Authentication(failure) => failure.fmt(f),
        }
    }
}

impl std::error::Error for SnmpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Malformed(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for UsmFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUserName => write!(f, "Unknown user name"),
            Self::WrongDigest => write!(f, "Wrong authentication password or protocol"),
            Self::DecryptionError => write!(f, "Wrong privacy password or protocol"),
            Self::UnsupportedSecurityLevel => {
                write!(f, "Security level does not match the device configuration")
            }
            Self::NotInTimeWindow => write!(f, "Time synchronization with the device failed"),
            Self::UnknownEngineId => write!(f, "Device engine ID mismatch"),
            Self::NoResponseToEncryptedRequest => write!(
                f,
                "The device did not answer the encrypted request. Check the privacy password and protocol."
            ),
            Self::DiscoveryIncomplete => write!(f, "SNMPv3 discovery has not completed"),
            Self::AuthPasswordTooShort { minimum } => {
                write!(
                    f,
                    "The authentication password must be at least {minimum} characters"
                )
            }
            Self::PrivPasswordTooShort { minimum } => {
                write!(
                    f,
                    "The privacy password must be at least {minimum} characters"
                )
            }
            Self::InvalidPrivacyParameters => write!(f, "Invalid encryption parameters"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_status_codes_follow_rfc_3416() {
        assert_eq!(ErrorStatus::from_code(0), Some(ErrorStatus::NoError));
        assert_eq!(ErrorStatus::from_code(2), Some(ErrorStatus::NoSuchName));
        assert_eq!(ErrorStatus::from_code(5), Some(ErrorStatus::GenErr));
        assert_eq!(
            ErrorStatus::from_code(16),
            Some(ErrorStatus::AuthorizationError)
        );
        assert_eq!(
            ErrorStatus::from_code(18),
            Some(ErrorStatus::InconsistentName)
        );
        assert_eq!(ErrorStatus::from_code(19), None);
        assert_eq!(ErrorStatus::from_code(-1), None);
        for code in 0..=18 {
            assert_eq!(
                ErrorStatus::from_code(code).map(ErrorStatus::code),
                Some(code)
            );
        }
        assert_eq!(ErrorStatus::NoSuchName.name(), "noSuchName");
        assert_eq!(ErrorStatus::InconsistentName.name(), "inconsistentName");
    }

    #[test]
    fn ber_errors_become_malformed() {
        let error: SnmpError = BerError::Truncated.into();
        assert_eq!(error, SnmpError::Malformed(BerError::Truncated));
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn display_is_english_for_logs() {
        assert_eq!(SnmpError::Timeout.to_string(), "Connection timed out");
        assert_eq!(
            SnmpError::ErrorStatus {
                status: 2,
                index: 1
            }
            .to_string(),
            "SNMP error noSuchName (varbind 1)"
        );
        assert_eq!(
            SnmpError::ErrorStatus {
                status: 99,
                index: 0
            }
            .to_string(),
            "SNMP error 99 (varbind 0)"
        );
        assert_eq!(
            SnmpError::UnexpectedPduType(0xA7).to_string(),
            "Unexpected PDU type 0xa7"
        );
        assert_eq!(
            SnmpError::InterfaceNotFound("pppoe-out1".into()).to_string(),
            "Interface pppoe-out1 not found"
        );
        assert_eq!(
            SnmpError::Authentication(UsmFailure::AuthPasswordTooShort { minimum: 8 }).to_string(),
            "The authentication password must be at least 8 characters"
        );
    }
}
