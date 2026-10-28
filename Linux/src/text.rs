//! Text composed in the UI language (uptime, error messages, connection status). The English source matches the other platforms,
//! and translations follow `docs/glossary.md`.

use std::time::Duration;

use anemobit_core::formatting::{UptimeUnit, uptime_parts};
use anemobit_core::{SnmpError, UsmFailure};
use gettextrs::{gettext, ngettext};

use crate::settings::{SecurityMode, SecurityProblem};
use crate::settings_draft::DraftProblem;

/// Replaces placeholders with arguments (gettext strings can't use `format!`). A single argument uses `{}`; several use numbered
/// `{0}`, `{1}`, so translations can reorder them.
pub fn fill(template: &str, values: &[&str]) -> String {
    if template.contains("{0}") {
        let mut text = template.to_owned();
        for (index, value) in values.iter().enumerate() {
            text = text.replace(&format!("{{{index}}}"), value);
        }
        return text;
    }
    let mut text = template.to_owned();
    for value in values {
        text = text.replacen("{}", value, 1);
    }
    text
}

/// Result for each interface when testing the connection: "pppoe-out1 is ifIndex 7.".
pub fn interface_index(name: &str, index: u32) -> String {
    // Translators: {0} is an interface name, {1} its index.
    fill(&gettext("{0} is ifIndex {1}."), &[name, &index.to_string()])
}

/// Uptime → "12 days, 5 hours" / "12天5小時" (at most two units, rounded down).
pub fn uptime(duration: Duration) -> String {
    let parts: Vec<String> = uptime_parts(duration)
        .into_iter()
        .map(|part| {
            let n = u32::try_from(part.value).unwrap_or(u32::MAX);
            let template = match part.unit {
                UptimeUnit::Days => ngettext("{} day", "{} days", n),
                UptimeUnit::Hours => ngettext("{} hour", "{} hours", n),
                UptimeUnit::Minutes => ngettext("{} minute", "{} minutes", n),
            };
            fill(&template, &[&part.value.to_string()])
        })
        .collect();
    match parts.as_slice() {
        // Translators: two uptime units, as in "12 days, 5 hours". Chinese and Japanese use no separator ("{0}{1}").
        [first, second] => fill(&gettext("{0}, {1}"), &[first, second]),
        _ => parts.concat(),
    }
}

/// Description of an SNMP error.
pub fn snmp_error(error: &SnmpError) -> String {
    match error {
        SnmpError::Malformed(ber) => fill(&gettext("Malformed packet ({})"), &[&ber.to_string()]),
        SnmpError::UnsupportedVersion(version) => fill(
            &gettext("Unsupported SNMP version {}"),
            &[&version.to_string()],
        ),
        SnmpError::UnexpectedPduType(pdu_type) => fill(
            &gettext("Unexpected PDU type 0x{}"),
            &[&format!("{pdu_type:02x}")],
        ),
        SnmpError::ErrorStatus { status, index } => {
            let name = anemobit_core::ErrorStatus::from_code(*status)
                .map_or_else(|| status.to_string(), |s| s.name().to_owned());
            // Translators: {0} is the RFC 3416 error name (for example noSuchName), {1} the varbind index.
            fill(
                &gettext("SNMP error {0} (varbind {1})"),
                &[&name, &index.to_string()],
            )
        }
        SnmpError::Timeout => gettext("Connection timed out"),
        SnmpError::UnexpectedResponse(detail) => {
            fill(&gettext("Unexpected response: {}"), &[detail])
        }
        SnmpError::InterfaceNotFound(name) => fill(&gettext("Interface {} not found"), &[name]),
        SnmpError::Transport(detail) => fill(&gettext("Connection error: {}"), &[detail]),
        SnmpError::Authentication(failure) => usm_failure(*failure),
    }
}

/// Description of an SNMPv3 authentication / privacy failure.
pub fn usm_failure(failure: UsmFailure) -> String {
    match failure {
        UsmFailure::UnknownUserName => gettext("Unknown user name"),
        UsmFailure::WrongDigest => gettext("Wrong authentication password or protocol"),
        UsmFailure::DecryptionError => gettext("Wrong privacy password or protocol"),
        UsmFailure::UnsupportedSecurityLevel => {
            gettext("Security level does not match the device configuration")
        }
        UsmFailure::NotInTimeWindow => gettext("Time synchronization with the device failed"),
        UsmFailure::UnknownEngineId => gettext("Device engine ID mismatch"),
        UsmFailure::NoResponseToEncryptedRequest => gettext(
            "The device did not answer the encrypted request. Check the privacy password and protocol.",
        ),
        UsmFailure::DiscoveryIncomplete => gettext("SNMPv3 discovery has not completed"),
        UsmFailure::AuthPasswordTooShort { minimum } => fill(
            &gettext("The authentication password must be at least {} characters"),
            &[&minimum.to_string()],
        ),
        UsmFailure::PrivPasswordTooShort { minimum } => fill(
            &gettext("The privacy password must be at least {} characters"),
            &[&minimum.to_string()],
        ),
        UsmFailure::InvalidPrivacyParameters => gettext("Invalid encryption parameters"),
    }
}

/// Description of incomplete security settings.
pub fn security_problem(problem: SecurityProblem) -> String {
    match problem {
        SecurityProblem::MissingCommunity => gettext("Enter a community"),
        SecurityProblem::MissingUserName => gettext("Enter a user name"),
        SecurityProblem::Usm(failure) => usm_failure(failure),
    }
}

/// Description of why the Settings draft can't connect.
pub fn draft_problem(problem: DraftProblem) -> String {
    match problem {
        DraftProblem::MissingHost => gettext("Enter the device address"),
        DraftProblem::InvalidHost => gettext("Enter a valid IPv4 or IPv6 address, or a host name"),
        DraftProblem::InvalidPort => gettext("Port must be 1–65535"),
        DraftProblem::Security(problem) => security_problem(problem),
    }
}

/// Description of the security mode (shown under the SNMP Security section).
pub fn mode_explanation(mode: SecurityMode) -> String {
    match mode {
        SecurityMode::None => gettext("SNMPv2c with a community only."),
        SecurityMode::Authorized => gettext("SNMPv3 with authentication."),
        SecurityMode::Private => gettext("SNMPv3 with authentication and encryption."),
    }
}

/// Refresh interval option: "1 sec".
pub fn interval(seconds: u32) -> String {
    // Translators: {} is a number of seconds (1, 5 or 10).
    fill(&gettext("{} sec"), &[&seconds.to_string()])
}

/// Status pill text when failed: "Error · retry in 4 s".
pub fn retry_status(retry_in: Duration) -> String {
    // Translators: {} is a number of seconds.
    fill(
        &gettext("Error · retry in {} s"),
        &[&retry_in.as_secs().to_string()],
    )
}

/// Subtitle when monitoring two or more interfaces: "3 interfaces".
pub fn interface_count(count: usize) -> String {
    let n = u32::try_from(count).unwrap_or(u32::MAX);
    fill(
        &ngettext("{} interface", "{} interfaces", n),
        &[&count.to_string()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SecurityProblem;
    use anemobit_core::BerError;

    // The test environment doesn't load translations, so gettext returns the English source.

    #[test]
    fn uptime_uses_up_to_two_units() {
        let cases = [
            (0, "0 minutes"),
            (59, "0 minutes"),
            (60, "1 minute"),
            (3_660, "1 hour, 1 minute"),
            (86_399, "23 hours, 59 minutes"),
            (86_400 * 12 + 3_600 * 5 + 120, "12 days, 5 hours"),
            (86_400, "1 day"),
            (86_400 * 2 + 60, "2 days, 1 minute"),
        ];
        for (seconds, expected) in cases {
            assert_eq!(uptime(Duration::from_secs(seconds)), expected, "{seconds}");
        }
    }

    #[test]
    fn snmp_errors_match_other_platforms() {
        let cases = [
            (SnmpError::Timeout, "Connection timed out"),
            (
                SnmpError::InterfaceNotFound("pppoe-out1".into()),
                "Interface pppoe-out1 not found",
            ),
            (
                SnmpError::Transport("unreachable".into()),
                "Connection error: unreachable",
            ),
            (
                SnmpError::UnsupportedVersion(0),
                "Unsupported SNMP version 0",
            ),
            (
                SnmpError::UnexpectedPduType(0xA7),
                "Unexpected PDU type 0xa7",
            ),
            (
                SnmpError::ErrorStatus {
                    status: 2,
                    index: 1,
                },
                "SNMP error noSuchName (varbind 1)",
            ),
            (
                SnmpError::UnexpectedResponse("x".into()),
                "Unexpected response: x",
            ),
            (
                SnmpError::Malformed(BerError::Truncated),
                "Malformed packet (BER data is truncated)",
            ),
            (
                SnmpError::Authentication(UsmFailure::WrongDigest),
                "Wrong authentication password or protocol",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(snmp_error(&error), expected);
        }
    }

    #[test]
    fn usm_failures_match_other_platforms() {
        let cases = [
            (UsmFailure::UnknownUserName, "Unknown user name"),
            (
                UsmFailure::WrongDigest,
                "Wrong authentication password or protocol",
            ),
            (
                UsmFailure::DecryptionError,
                "Wrong privacy password or protocol",
            ),
            (
                UsmFailure::UnsupportedSecurityLevel,
                "Security level does not match the device configuration",
            ),
            (
                UsmFailure::NotInTimeWindow,
                "Time synchronization with the device failed",
            ),
            (UsmFailure::UnknownEngineId, "Device engine ID mismatch"),
            (
                UsmFailure::NoResponseToEncryptedRequest,
                "The device did not answer the encrypted request. Check the privacy password and protocol.",
            ),
            (
                UsmFailure::DiscoveryIncomplete,
                "SNMPv3 discovery has not completed",
            ),
            (
                UsmFailure::AuthPasswordTooShort { minimum: 8 },
                "The authentication password must be at least 8 characters",
            ),
            (
                UsmFailure::PrivPasswordTooShort { minimum: 8 },
                "The privacy password must be at least 8 characters",
            ),
            (
                UsmFailure::InvalidPrivacyParameters,
                "Invalid encryption parameters",
            ),
        ];
        for (failure, expected) in cases {
            assert_eq!(usm_failure(failure), expected);
        }
    }

    #[test]
    fn security_problems() {
        assert_eq!(
            security_problem(SecurityProblem::MissingCommunity),
            "Enter a community"
        );
        assert_eq!(
            security_problem(SecurityProblem::MissingUserName),
            "Enter a user name"
        );
        assert_eq!(
            security_problem(SecurityProblem::Usm(UsmFailure::AuthPasswordTooShort {
                minimum: 8
            })),
            "The authentication password must be at least 8 characters"
        );
    }

    #[test]
    fn settings_texts_match_macos() {
        assert_eq!(
            draft_problem(DraftProblem::MissingHost),
            "Enter the device address"
        );
        assert_eq!(
            draft_problem(DraftProblem::InvalidHost),
            "Enter a valid IPv4 or IPv6 address, or a host name"
        );
        assert_eq!(
            draft_problem(DraftProblem::InvalidPort),
            "Port must be 1–65535"
        );
        assert_eq!(
            draft_problem(DraftProblem::Security(SecurityProblem::MissingUserName)),
            "Enter a user name"
        );
        assert_eq!(
            mode_explanation(SecurityMode::None),
            "SNMPv2c with a community only."
        );
        assert_eq!(
            mode_explanation(SecurityMode::Authorized),
            "SNMPv3 with authentication."
        );
        assert_eq!(
            mode_explanation(SecurityMode::Private),
            "SNMPv3 with authentication and encryption."
        );
        assert_eq!(interval(5), "5 sec");
    }

    #[test]
    fn placeholders_can_be_reordered() {
        assert_eq!(fill("Max {}", &["1.0 Mbps"]), "Max 1.0 Mbps");
        assert_eq!(
            fill("{0} is ifIndex {1}.", &["pppoe-out1", "7"]),
            "pppoe-out1 is ifIndex 7."
        );
        // Still correct when a translation swaps the argument order.
        assert_eq!(
            fill("ifIndex {1} = {0}", &["pppoe-out1", "7"]),
            "ifIndex 7 = pppoe-out1"
        );
        assert_eq!(interface_index("ether1", 1), "ether1 is ifIndex 1.");
    }

    #[test]
    fn status_texts() {
        assert_eq!(retry_status(Duration::from_secs(4)), "Error · retry in 4 s");
        assert_eq!(interface_count(3), "3 interfaces");
    }
}
