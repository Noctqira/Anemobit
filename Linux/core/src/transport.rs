//! Abstraction for sending and receiving UDP datagrams, so [`SnmpClient`](crate::SnmpClient) can use a test double.

use std::future::Future;
use std::io;

mod udp;

pub use udp::UdpTransport;

/// Sends and receives UDP datagrams. Closing the connection is handled by drop.
pub trait Transport {
    /// Sends one datagram.
    fn send(&self, datagram: &[u8]) -> impl Future<Output = io::Result<()>>;

    /// Waits for the next received datagram.
    fn receive(&self) -> impl Future<Output = io::Result<Vec<u8>>>;
}
