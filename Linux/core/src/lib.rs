//! Anemobit core logic: BER, SNMPv2c / SNMPv3 USM, rate calculation and polling.
//!
//! No GTK dependency; only glib / gio for the event loop and UDP, and the network layer can be swapped for test doubles.

mod ber;
pub mod formatting;
mod snmp;
#[cfg(test)]
mod testing;
pub mod traffic;
pub mod transport;

pub use ber::{BerError, Oid, ParseOidError, SnmpValue};
pub use snmp::{
    AuthProtocol, ErrorStatus, PrivProtocol, SnmpClient, SnmpError, SnmpInterface, SnmpSecurity,
    UsmCredentials, UsmFailure, UsmPrivacy, VarBind, oids,
};
