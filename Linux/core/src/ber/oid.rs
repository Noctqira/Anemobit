use std::fmt;
use std::str::FromStr;

/// SNMP Object Identifier; each arc is a 32-bit unsigned integer.
///
/// Ordered arc by arc (SNMP's lexicographic order).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Oid(Vec<u32>);

impl Oid {
    pub fn arcs(&self) -> &[u32] {
        &self.0
    }

    /// Appends one arc (e.g. a column OID plus an index).
    pub fn child(&self, arc: u32) -> Oid {
        let mut arcs = self.0.clone();
        arcs.push(arc);
        Oid(arcs)
    }

    pub fn starts_with(&self, prefix: &Oid) -> bool {
        self.0.starts_with(&prefix.0)
    }

    /// The arcs left after removing `prefix`; `None` if `prefix` isn't a prefix.
    pub fn strip_prefix(&self, prefix: &Oid) -> Option<&[u32]> {
        self.0.strip_prefix(prefix.0.as_slice())
    }
}

impl From<Vec<u32>> for Oid {
    fn from(arcs: Vec<u32>) -> Self {
        Oid(arcs)
    }
}

impl From<&[u32]> for Oid {
    fn from(arcs: &[u32]) -> Self {
        Oid(arcs.to_vec())
    }
}

impl<const N: usize> From<[u32; N]> for Oid {
    fn from(arcs: [u32; N]) -> Self {
        Oid(arcs.to_vec())
    }
}

/// The string format of an [`Oid`] is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOidError;

impl fmt::Display for ParseOidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid OID string")
    }
}

impl std::error::Error for ParseOidError {}

impl FromStr for Oid {
    type Err = ParseOidError;

    /// Parses `1.3.6.1` or `.1.3.6.1`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let text = s.strip_prefix('.').unwrap_or(s);
        if text.is_empty() {
            return Err(ParseOidError);
        }
        text.split('.')
            .map(|part| {
                // u32::from_str accepts "+1"; only ASCII digits are allowed here.
                if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(ParseOidError);
                }
                part.parse().map_err(|_| ParseOidError)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Oid)
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, arc) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            write!(f, "{arc}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dotted_string() {
        let oid: Oid = "1.3.6.1.2.1.1.3.0".parse().unwrap();
        assert_eq!(oid.arcs(), [1, 3, 6, 1, 2, 1, 1, 3, 0]);
    }

    #[test]
    fn parses_leading_dot() {
        assert_eq!(".1.3.6".parse(), Ok(Oid::from([1, 3, 6])));
    }

    #[test]
    fn rejects_invalid_strings() {
        for text in [
            "",
            ".",
            "1..3",
            "1.3.a",
            "1.3.",
            "1.3.4294967296",
            "-1.3",
            "+1.3",
            "1. 3",
            "１.3",
        ] {
            assert_eq!(text.parse::<Oid>(), Err(ParseOidError), "{text:?}");
        }
    }

    #[test]
    fn display_round_trips() {
        let text = "1.3.6.1.2.1.31.1.1.1.6";
        assert_eq!(text.parse::<Oid>().unwrap().to_string(), text);
    }

    #[test]
    fn prefix_and_child() {
        let root = Oid::from([1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 1]);
        let child = root.child(7);
        assert!(child.starts_with(&root));
        assert!(!root.starts_with(&child));
        assert_eq!(child.arcs().last(), Some(&7));
        assert_eq!(child.strip_prefix(&root), Some(&[7][..]));
        assert_eq!(root.strip_prefix(&child), None);
        assert!(!child.starts_with(&Oid::from([1, 3, 6, 2])));
    }

    #[test]
    fn lexicographic_order() {
        assert!(Oid::from([1, 3, 6]) < Oid::from([1, 3, 6, 1]));
        assert!(Oid::from([1, 3, 6, 1]) < Oid::from([1, 3, 7]));
        assert!(Oid::from([1, 3, 7]) > Oid::from([1, 3, 6, 9]));
    }
}
