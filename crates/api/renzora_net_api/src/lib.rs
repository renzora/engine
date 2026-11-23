//! HTTP request vocabulary, the submission queue, and the client contract.
//!
//! Moved out of `renzora` so the contract crate stops rebuilding the whole
//! engine for a change to the network layer. The modules keep their names, so
//! a plugin still writes `renzora::net::Request` and `renzora::net_backend::Backend`:
//! `renzora_sdk` re-exports both at those paths. Engine crates depend on this
//! crate directly and write `renzora_net_api::net::Request`.
//!
//! `net`'s queue is process-global state, so it must exist once per process.
//! That holds because `renzora_dylib` embeds this crate the same way it embeds
//! `renzora`: every crate that links it, engine or plugin, reaches the one copy
//! inside that image.

// HTTP request vocabulary + the submission queue. `Request` and `Response` are
// names generic enough to collide, so callers name the module. The engine ships
// no HTTP client: the socket is opened by whichever crate implements
// `net_backend::Backend`. See the module doc.
pub mod net;

// The client contract itself: the `Backend` trait, what it answers with, and the
// events it sends back. Beside `net` because its queue holds these types.
pub mod net_backend;
