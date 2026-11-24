//! Moving datagrams between the OS and the sans-IO endpoints.

use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};

use super::packet::MAX_PACKET_BYTES;

/// A non-blocking UDP socket.
pub struct Socket(UdpSocket);

impl Socket {
    pub fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self(socket))
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.0.local_addr().ok()
    }

    /// Every datagram waiting, handed to `f`.
    pub fn drain(&self, mut f: impl FnMut(SocketAddr, &[u8])) {
        let mut buf = [0u8; MAX_PACKET_BYTES * 2];
        loop {
            match self.0.recv_from(&mut buf) {
                Ok((n, from)) => f(from, &buf[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                // Windows reports an ICMP "port unreachable" from an earlier
                // send as an error on the next receive (WSAECONNRESET). It
                // says a peer went away, not that this socket broke, so
                // keep reading.
                Err(e) if e.kind() == ErrorKind::ConnectionReset => continue,
                Err(_) => break,
            }
        }
    }

    pub fn send(&self, to: SocketAddr, bytes: &[u8]) {
        let _ = self.0.send_to(bytes, to);
    }
}
