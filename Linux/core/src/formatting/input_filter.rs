//! Filters the device address and port the user types: whatever keyboard, input method or paste is used, only allowed characters remain.
//! Full-width letters, digits and periods, and the CJK full stop are converted to ASCII first. [`DeviceAddress`](super::DeviceAddress) decides whether the format is valid.

/// Host names are at most 253 characters; IPv6 with brackets and an interface name also fits.
pub const HOST_MAXIMUM_LENGTH: usize = 253;
/// Ports have at most 5 digits (65535).
pub const PORT_MAXIMUM_LENGTH: usize = 5;

/// Characters used by IPv4, IPv6 (including `[...]` and `%interface`) and host names: letters, digits, `.`, `-`, `:`, `[`, `]`, `%`.
pub fn filter_host(text: &str) -> String {
    text.chars()
        .map(normalize)
        .filter(|c| c.is_ascii_alphanumeric() || ".-:[]%".contains(*c))
        .take(HOST_MAXIMUM_LENGTH)
        .collect()
}

/// Keeps digits only.
pub fn filter_port(text: &str) -> String {
    text.chars()
        .map(normalize)
        .filter(char::is_ascii_digit)
        .take(PORT_MAXIMUM_LENGTH)
        .collect()
}

/// Converts full-width letters, digits and symbols (U+FF01–U+FF5E) to ASCII; the CJK full stop "。" and the half-width "｡" become ".".
fn normalize(c: char) -> char {
    match c {
        '。' | '｡' => '.',
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_keeps_only_address_characters() {
        let cases = [
            ("192.168.88.1", "192.168.88.1"),
            ("", ""),
            ("router.lan", "router.lan"),
            ("my-router.example.com", "my-router.example.com"),
            ("2001:db8::1", "2001:db8::1"),
            ("[2001:db8::1]", "[2001:db8::1]"),
            ("fe80::1%en0", "fe80::1%en0"),
            ("192 .168.88.1 ", "192.168.88.1"), // Whitespace
            ("１９２．１６８．８８．１", "192.168.88.1"), // Full-width digits and periods (Chinese / Japanese input methods)
            ("192。168。88。1", "192.168.88.1"),          // CJK full stops
            ("192｡168｡88｡1", "192.168.88.1"),             // Half-width ideographic full stops
            ("ｒｏｕｔｅｒ．ＬＡＮ", "router.LAN"),       // Full-width letters
            ("ㄅ路由器router", "router"),                 // Zhuyin and Chinese
            ("router/admin?x=1", "routeradminx1"),        // Other URL symbols
            ("［2001:db8::1］", "[2001:db8::1]"),         // Full-width brackets
        ];
        for (input, expected) in cases {
            assert_eq!(filter_host(input), expected, "{input:?}");
        }
    }

    #[test]
    fn host_is_limited_to_maximum_length() {
        assert_eq!(filter_host(&"a".repeat(300)).len(), HOST_MAXIMUM_LENGTH);
    }

    #[test]
    fn port_keeps_only_digits() {
        let cases = [
            ("161", "161"),
            ("", ""),
            ("16a1", "161"),
            ("１６１", "161"),
            ("1.6,1", "161"),
            ("ㄅ1ㄆ6ㄇ1", "161"),
            ("655350", "65535"), // At most 5 digits
        ];
        for (input, expected) in cases {
            assert_eq!(filter_port(input), expected, "{input:?}");
        }
    }
}
