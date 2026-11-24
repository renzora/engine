//! The UDP transport, written from scratch.
//!
//! - [`packet`]: the datagram format.
//! - [`connection`]: reliability over one peer (acks, ordering, fragments,
//!   round trip, loss).
//! - [`endpoint`]: the server and client, sans-IO: bytes in, bytes out.
//! - [`udp`]: the only part that touches a socket.
//!
//! Not encrypted. The handshake stops spoofed addresses from taking slots,
//! and nothing else; do not carry secrets over it.

pub mod connection;
pub mod endpoint;
pub mod packet;
pub mod udp;

pub use connection::{Channel, ConnectionStats};
pub use endpoint::{ClientEndpoint, ClientEvent, ClientState, ServerEndpoint, ServerEvent};
