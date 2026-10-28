use std::net::Ipv6Addr;

/// Device address: IPv4, IPv6 or host name. Used to validate input in Settings and to get the host string for connecting.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeviceAddress {
    Ipv4(String),
    /// May include `%interface` (link-local addresses, e.g. `fe80::1%eth0`).
    Ipv6(String),
    HostName(String),
}

impl DeviceAddress {
    /// Trims surrounding whitespace; IPv6 may be written as `[2001:db8::1]`, and a trailing `.` on a host name is removed. Returns `None` when invalid.
    pub fn parse(text: &str) -> Option<Self> {
        let value = text.trim();
        if value.is_empty() {
            return None;
        }
        if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            return is_ipv6(inner).then(|| Self::Ipv6(inner.to_owned()));
        }
        if value.contains(':') {
            return is_ipv6(value).then(|| Self::Ipv6(value.to_owned()));
        }
        if is_dotted_quad(value) {
            return Some(Self::Ipv4(value.to_owned()));
        }
        let value = value.strip_suffix('.').unwrap_or(value);
        is_host_name(value).then(|| Self::HostName(value.to_owned()))
    }

    /// Host used for connecting (without brackets).
    pub fn host(&self) -> &str {
        match self {
            Self::Ipv4(value) | Self::Ipv6(value) | Self::HostName(value) => value,
        }
    }
}

/// Four decimal numbers 0–255 (1–3 digits each).
fn is_dotted_quad(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|part| {
            (1..=3).contains(&part.len())
                && part.bytes().all(|b| b.is_ascii_digit())
                && part.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}

/// IPv6, optionally with `%interface`.
fn is_ipv6(value: &str) -> bool {
    match value.split_once('%') {
        Some((address, zone)) => {
            !zone.is_empty()
                && zone
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && address.parse::<Ipv6Addr>().is_ok()
        }
        None => value.parse::<Ipv6Addr>().is_ok(),
    }
}

/// RFC 1123: total length ≤ 253; each label is 1–63 letters, digits or `-`, not starting or ending with `-`;
/// the last label can't be all digits (so a mistyped IP isn't taken as a host name).
fn is_host_name(value: &str) -> bool {
    if value.len() > 253 {
        return false;
    }
    let labels: Vec<_> = value.split('.').collect();
    let valid_labels = labels.iter().all(|label| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    });
    valid_labels
        && labels
            .last()
            .is_some_and(|last| !last.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_addresses() {
        let cases = [
            ("192.168.88.1", DeviceAddress::Ipv4("192.168.88.1".into())),
            (" 10.0.0.254 ", DeviceAddress::Ipv4("10.0.0.254".into())),
            ("2001:db8::1", DeviceAddress::Ipv6("2001:db8::1".into())),
            ("[2001:db8::1]", DeviceAddress::Ipv6("2001:db8::1".into())), // Bracket notation
            ("fd00::1", DeviceAddress::Ipv6("fd00::1".into())),
            (
                "::ffff:192.168.88.1",
                DeviceAddress::Ipv6("::ffff:192.168.88.1".into()),
            ),
            ("fe80::1%eth0", DeviceAddress::Ipv6("fe80::1%eth0".into())),
            (
                "[fe80::1%wlp2s0]",
                DeviceAddress::Ipv6("fe80::1%wlp2s0".into()),
            ),
            ("router.lan", DeviceAddress::HostName("router.lan".into())),
            ("Router.LAN.", DeviceAddress::HostName("Router.LAN".into())), // Trailing dot
            ("mikrotik", DeviceAddress::HostName("mikrotik".into())),
            (
                "my-router-1.example.com",
                DeviceAddress::HostName("my-router-1.example.com".into()),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(DeviceAddress::parse(input), Some(expected), "{input:?}");
        }
    }

    #[test]
    fn rejects_invalid_addresses() {
        let cases = [
            "",
            "   ",
            "999.1.1.1",  // Number above 255
            "192.168.88", // Only three parts
            "192.168.88.1.5",
            "2001:db8:::1",   // Invalid IPv6
            "[192.168.88.1]", // Brackets are for IPv6 only
            "fe80::1%",       // Empty interface name
            "-router.lan",    // Labels can't start with -
            "router-.lan",
            "router..lan",
            "router_lan", // Underscores not allowed
            "123.456",    // All-digit last label (looks like an IP)
            "rοuter.lan", // Non-ASCII (Greek letter ο)
        ];
        for input in cases {
            assert_eq!(DeviceAddress::parse(input), None, "{input:?}");
        }
    }

    #[test]
    fn rejects_too_long_host_names() {
        let label = "a".repeat(64);
        assert_eq!(DeviceAddress::parse(&format!("{label}.lan")), None);
        let name = vec!["a".repeat(60); 5].join(".");
        assert!(name.len() > 253);
        assert_eq!(DeviceAddress::parse(&name), None);
    }

    #[test]
    fn host_is_what_the_transport_connects_to() {
        assert_eq!(
            DeviceAddress::parse("[2001:db8::1]").unwrap().host(),
            "2001:db8::1"
        );
        assert_eq!(
            DeviceAddress::parse(" router.lan. ").unwrap().host(),
            "router.lan"
        );
        assert_eq!(
            DeviceAddress::parse("192.168.88.1").unwrap().host(),
            "192.168.88.1"
        );
    }
}
