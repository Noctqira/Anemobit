use std::fmt;

use super::UsmFailure;
use super::v3::{AuthProtocol, PrivProtocol};

/// SNMP security settings.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum SnmpSecurity {
    /// SNMPv2c community (RouterOS security = none).
    Community(String),
    /// SNMPv3 USM (RouterOS authorized / private).
    Usm(UsmCredentials),
}

/// SNMPv3 USM account. The RouterOS community name is the user name.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct UsmCredentials {
    pub user_name: String,
    pub auth_protocol: AuthProtocol,
    pub auth_password: String,
    /// `None` means authentication without privacy (authNoPriv, RouterOS's authorized).
    pub privacy: Option<UsmPrivacy>,
}

/// SNMPv3 privacy settings (authPriv, RouterOS's private).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct UsmPrivacy {
    pub protocol: PrivProtocol,
    pub password: String,
}

impl UsmCredentials {
    /// Passwords must be at least 8 characters (RFC 3414 §11.2).
    pub const MINIMUM_PASSWORD_LENGTH: usize = 8;

    /// Checks password lengths (in UTF-8 bytes, same as the Swift version); returns the reason when invalid.
    pub fn validation_failure(&self) -> Option<UsmFailure> {
        let minimum = Self::MINIMUM_PASSWORD_LENGTH;
        if self.auth_password.len() < minimum {
            return Some(UsmFailure::AuthPasswordTooShort { minimum });
        }
        match &self.privacy {
            Some(privacy) if privacy.password.len() < minimum => {
                Some(UsmFailure::PrivPasswordTooShort { minimum })
            }
            _ => None,
        }
    }
}

// The community, user name (= community name) and passwords must never appear in logs: Debug doesn't print them.
const REDACTED: &str = "<redacted>";

impl fmt::Debug for SnmpSecurity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Community(_) => f
                .debug_tuple("Community")
                .field(&format_args!("{REDACTED}"))
                .finish(),
            Self::Usm(credentials) => f.debug_tuple("Usm").field(credentials).finish(),
        }
    }
}

impl fmt::Debug for UsmCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsmCredentials")
            .field("user_name", &format_args!("{REDACTED}"))
            .field("auth_protocol", &self.auth_protocol)
            .field("auth_password", &format_args!("{REDACTED}"))
            .field("privacy", &self.privacy)
            .finish()
    }
}

impl fmt::Debug for UsmPrivacy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsmPrivacy")
            .field("protocol", &self.protocol)
            .field("password", &format_args!("{REDACTED}"))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials(auth_password: &str, priv_password: Option<&str>) -> UsmCredentials {
        UsmCredentials {
            user_name: "secret-user".into(),
            auth_protocol: AuthProtocol::Sha1,
            auth_password: auth_password.into(),
            privacy: priv_password.map(|password| UsmPrivacy {
                protocol: PrivProtocol::Aes,
                password: password.into(),
            }),
        }
    }

    #[test]
    fn debug_hides_community_user_name_and_passwords() {
        let community = format!("{:?}", SnmpSecurity::Community("s3cret-community".into()));
        assert_eq!(community, "Community(<redacted>)");
        let usm = format!(
            "{:?}",
            SnmpSecurity::Usm(credentials("authpass12", Some("privpass12")))
        );
        for secret in ["secret-user", "authpass12", "privpass12"] {
            assert!(!usm.contains(secret), "{usm}");
        }
        assert!(usm.contains("Sha1") && usm.contains("Aes"), "{usm}");
    }

    #[test]
    fn passwords_must_have_at_least_eight_bytes() {
        assert_eq!(credentials("authpass", None).validation_failure(), None);
        assert_eq!(
            credentials("authpass", Some("privpass")).validation_failure(),
            None
        );
        assert_eq!(
            credentials("short", Some("privpass12")).validation_failure(),
            Some(UsmFailure::AuthPasswordTooShort { minimum: 8 })
        );
        assert_eq!(
            credentials("authpass12", Some("short")).validation_failure(),
            Some(UsmFailure::PrivPasswordTooShort { minimum: 8 })
        );
        // Counted in UTF-8 bytes: three Chinese characters are 9 bytes.
        assert_eq!(credentials("密碼長", None).validation_failure(), None);
    }
}
