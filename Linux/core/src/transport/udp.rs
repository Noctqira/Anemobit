use std::io;

use gio::prelude::*;

use super::Transport;

/// UDP transport built on `gio::Socket` (connected to the device, so it only receives packets from that address).
///
/// Async waiting uses the glib event loop, so this must run on a `glib::MainContext`.
#[derive(Debug)]
pub struct UdpTransport {
    socket: gio::Socket,
}

/// Maximum UDP packet size.
const MAX_DATAGRAM: usize = 65_535;

impl UdpTransport {
    /// Resolves `host` (name or IP) and creates a UDP socket connected to `host:port`.
    pub async fn connect(host: &str, port: u16) -> io::Result<Self> {
        let addresses = gio::Resolver::default()
            .lookup_by_name_future(host)
            .await
            .map_err(io::Error::other)?;
        let address = addresses.into_iter().next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("no address for {host}"))
        })?;
        let socket = gio::Socket::new(
            address.family(),
            gio::SocketType::Datagram,
            gio::SocketProtocol::Udp,
        )
        .map_err(io::Error::other)?;
        socket.set_blocking(false);
        // gio::Socket also has GObject's connect (for signals), so name SocketExt explicitly.
        SocketExt::connect(
            &socket,
            &gio::InetSocketAddress::new(&address, port),
            gio::Cancellable::NONE,
        )
        .map_err(io::Error::other)?;
        Ok(UdpTransport { socket })
    }

    async fn wait(&self, condition: glib::IOCondition) {
        // DatagramBased has a method with the same name, so name SocketExtManual explicitly.
        SocketExtManual::create_source_future(
            &self.socket,
            condition,
            gio::Cancellable::NONE,
            glib::Priority::DEFAULT,
        )
        .await;
    }
}

/// When an earlier packet got an ICMP port unreachable, Linux reports ECONNREFUSED on the next send / receive.
/// That doesn't mean the connection failed; ignore it and leave it to the timeout and retries (same as the Windows version).
fn is_connection_refused(error: &glib::Error) -> bool {
    error.matches(gio::IOErrorEnum::ConnectionRefused)
}

impl Transport for UdpTransport {
    async fn send(&self, datagram: &[u8]) -> io::Result<()> {
        loop {
            match self.socket.send(datagram, gio::Cancellable::NONE) {
                Ok(_) => return Ok(()),
                Err(error) if error.matches(gio::IOErrorEnum::WouldBlock) => {
                    self.wait(glib::IOCondition::OUT).await;
                }
                // The reported error is now cleared and this packet wasn't sent yet, so send it again.
                Err(error) if is_connection_refused(&error) => {}
                Err(error) => return Err(io::Error::other(error)),
            }
        }
    }

    async fn receive(&self) -> io::Result<Vec<u8>> {
        let mut buffer = vec![0; MAX_DATAGRAM];
        loop {
            match self.socket.receive(&mut buffer, gio::Cancellable::NONE) {
                Ok(length) => {
                    buffer.truncate(length);
                    return Ok(buffer);
                }
                Err(error) if error.matches(gio::IOErrorEnum::WouldBlock) => {
                    self.wait(glib::IOCondition::IN).await;
                }
                Err(error) if is_connection_refused(&error) => {}
                Err(error) => return Err(io::Error::other(error)),
            }
        }
    }
}
