//! BER (X.690) encoding and decoding, implementing only what SNMP uses.

pub(crate) mod encoding;
mod error;
mod oid;
mod reader;
pub(crate) mod tag;
mod value;

pub use error::BerError;
pub use oid::{Oid, ParseOidError};
pub(crate) use reader::BerReader;
pub use value::SnmpValue;
