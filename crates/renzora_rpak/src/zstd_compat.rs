//! zstd decompression — one call, two implementations.
//!
//! The `zstd` crate builds a vendored libzstd through `cc`, and targeting
//! wasm32-unknown-unknown there is no libc sysroot for that C to compile
//! against: clang stops at `'stdlib.h' file not found` before it reaches any
//! Renzora code. So the web build decodes with `ruzstd` (pure Rust) instead.
//!
//! Decode-only is enough, not a downgrade: packing lives in `mod pack`, which is
//! already `cfg(not(target_arch = "wasm32"))` because export is a desktop
//! operation. The web runtime only ever *reads* `.rpak` archives.
//!
//! `ruzstd` is not new weight for the tree either — Bevy already pulls the same
//! version for its own `zstd_rust` feature, so the wasm build links one copy
//! that was going to be there regardless.

use std::io;

#[cfg(not(target_arch = "wasm32"))]
pub fn decode_all(src: &[u8]) -> io::Result<Vec<u8>> {
    zstd::decode_all(src)
}

#[cfg(target_arch = "wasm32")]
pub fn decode_all(src: &[u8]) -> io::Result<Vec<u8>> {
    use std::io::Read;

    // `StreamingDecoder::new` eagerly parses the frame header, so a malformed
    // archive fails here rather than partway through `read_to_end`.
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(src)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}
