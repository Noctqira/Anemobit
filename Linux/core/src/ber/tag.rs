//! BER tag constants.

pub(crate) const INTEGER: u8 = 0x02;
pub(crate) const OCTET_STRING: u8 = 0x04;
pub(crate) const NULL: u8 = 0x05;
pub(crate) const OID: u8 = 0x06;
pub(crate) const SEQUENCE: u8 = 0x30;

// SMIv2 application types
pub(crate) const IP_ADDRESS: u8 = 0x40;
pub(crate) const COUNTER32: u8 = 0x41;
pub(crate) const GAUGE32: u8 = 0x42;
pub(crate) const TIME_TICKS: u8 = 0x43;
pub(crate) const COUNTER64: u8 = 0x46;

// SNMPv2 varbind exception values (context-specific, primitive)
pub(crate) const NO_SUCH_OBJECT: u8 = 0x80;
pub(crate) const NO_SUCH_INSTANCE: u8 = 0x81;
pub(crate) const END_OF_MIB_VIEW: u8 = 0x82;
