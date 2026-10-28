//! MIB OIDs used by Anemobit.

use std::sync::LazyLock;

use crate::Oid;

/// IF-MIB::ifName (table column, followed by ifIndex)
pub static IF_NAME: LazyLock<Oid> = LazyLock::new(|| Oid::from([1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 1]));
/// IF-MIB::ifDescr
pub static IF_DESCR: LazyLock<Oid> = LazyLock::new(|| Oid::from([1, 3, 6, 1, 2, 1, 2, 2, 1, 2]));
/// IF-MIB::ifHCInOctets (64-bit)
pub static IF_HC_IN_OCTETS: LazyLock<Oid> =
    LazyLock::new(|| Oid::from([1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 6]));
/// IF-MIB::ifHCOutOctets (64-bit)
pub static IF_HC_OUT_OCTETS: LazyLock<Oid> =
    LazyLock::new(|| Oid::from([1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 10]));
/// SNMPv2-MIB::sysUpTime.0
pub static SYS_UP_TIME: LazyLock<Oid> = LazyLock::new(|| Oid::from([1, 3, 6, 1, 2, 1, 1, 3, 0]));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oids_match_the_spec() {
        // Matches the OID table in the top-level CLAUDE.md.
        assert_eq!(IF_NAME.to_string(), "1.3.6.1.2.1.31.1.1.1.1");
        assert_eq!(IF_DESCR.to_string(), "1.3.6.1.2.1.2.2.1.2");
        assert_eq!(IF_HC_IN_OCTETS.to_string(), "1.3.6.1.2.1.31.1.1.1.6");
        assert_eq!(IF_HC_OUT_OCTETS.to_string(), "1.3.6.1.2.1.31.1.1.1.10");
        assert_eq!(SYS_UP_TIME.to_string(), "1.3.6.1.2.1.1.3.0");
    }
}
