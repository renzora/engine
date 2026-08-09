//! The byte codec both sides of the scripting boundary compile.
//!
//! ## Why this is hand-rolled
//!
//! The obvious move is `serde` + `bincode`, and it is the wrong one here for a
//! reason that has nothing to do with taste. A derive puts the *format* in a
//! dependency, and the host and the plugin resolve their dependencies
//! separately — different machines, different lockfiles, possibly different
//! rustc. Two builds that each say "bincode 2" can still disagree about varint
//! encoding or enum tag width across a patch release, and the failure mode is
//! not a link error: it is a script that reads a float out of the middle of a
//! string.
//!
//! This module sidesteps that by being compiled into **both** sides from the
//! same source. There is one encoder and one decoder, they cannot drift, and
//! the format is whatever this file says it is. That is also why the guest side
//! of `renzora_plugin` is allowed to stay dependency-free — see the note at the
//! bottom of its `Cargo.toml`, which forbids exactly the dependency serde would
//! have added.
//!
//! ## Why decoding returns `Result`
//!
//! The bytes come from another compilation unit. A length prefix is a number
//! some other binary wrote, so it is untrusted input in the ordinary sense —
//! `buf[pos..pos + len]` on a bad pair is a panic at best and an out-of-bounds
//! read at worst, and a panic unwinding out of an `extern "C"` frame aborts the
//! process. Every read here bounds-checks first and returns [`WireError`]
//! instead, so a malformed payload costs one logged line.
//!
//! ## The format
//!
//! Little-endian fixed-width scalars, `u32` length prefixes on strings and
//! lists, one `u8` tag for `Option`, one `u16` tag for enums. No padding, no
//! alignment requirements, no self-description. Little-endian is stated
//! explicitly rather than inherited from the host's byte order because the two
//! sides are separate binaries and "they're both x86" is a fact about today.

use std::string::String;
use std::vec::Vec;

/// Why a byte slice could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// A read ran off the end of the buffer. Either the payload was cut short
    /// or the two sides disagree about the layout, which looks the same from
    /// here.
    Truncated,
    /// A tag named a variant this build does not have — i.e. the writer was
    /// built against a newer version of this file.
    UnknownTag(u32),
    /// A string field was not valid UTF-8.
    BadUtf8,
}

impl core::fmt::Display for WireError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => f.write_str("payload ended mid-value"),
            Self::UnknownTag(t) => write!(f, "unknown tag {t} (writer is newer than reader)"),
            Self::BadUtf8 => f.write_str("string field was not valid UTF-8"),
        }
    }
}

impl std::error::Error for WireError {}

/// Appends encoded values to a growable buffer.
///
/// Deliberately infallible: writing cannot fail, so no call site is cluttered
/// with a `?` that can only ever succeed. All the fallibility lives in
/// [`Reader`], which is where the untrusted bytes are.
#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Pre-sized. Worth using for the per-frame context, which is encoded once
    /// per frame and is the one payload big enough for reallocation to show up.
    pub fn with_capacity(n: usize) -> Self {
        Self {
            buf: Vec::with_capacity(n),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Reuse the allocation for the next frame rather than freeing it.
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn bool(&mut self, v: bool) {
        self.buf.push(v as u8);
    }

    /// A `u32` count. Lists and strings share this so the two are consistent,
    /// and `usize` is narrowed here rather than at every call site — the two
    /// sides could in principle have different pointer widths, and a payload
    /// with four billion elements is a bug however it arrived.
    pub fn count(&mut self, n: usize) {
        self.u32(n as u32);
    }

    pub fn str(&mut self, s: &str) {
        self.count(s.len());
        self.buf.extend_from_slice(s.as_bytes());
    }

    pub fn bytes_field(&mut self, b: &[u8]) {
        self.count(b.len());
        self.buf.extend_from_slice(b);
    }

    pub fn opt_str(&mut self, s: Option<&str>) {
        match s {
            Some(s) => {
                self.bool(true);
                self.str(s);
            }
            None => self.bool(false),
        }
    }

    pub fn opt_u64(&mut self, v: Option<u64>) {
        match v {
            Some(v) => {
                self.bool(true);
                self.u64(v);
            }
            None => self.bool(false),
        }
    }

    pub fn opt_f32(&mut self, v: Option<f32>) {
        match v {
            Some(v) => {
                self.bool(true);
                self.f32(v);
            }
            None => self.bool(false),
        }
    }

    pub fn f32x2(&mut self, v: [f32; 2]) {
        for c in v {
            self.f32(c);
        }
    }

    pub fn f32x3(&mut self, v: [f32; 3]) {
        for c in v {
            self.f32(c);
        }
    }

    pub fn f32x4(&mut self, v: [f32; 4]) {
        for c in v {
            self.f32(c);
        }
    }
}

/// Reads encoded values out of a borrowed buffer.
///
/// Strings come back borrowed from the buffer where the caller can use them
/// that way. That is not micro-optimisation: the per-frame context carries the
/// pressed-key and action tables, and a language plugin re-reads those for
/// every scripted entity every frame. Copying each name into a fresh `String`
/// there is allocation proportional to entities × keys.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Bytes not yet consumed. A decoder that finishes with this non-zero read
    /// fewer fields than the writer wrote, which means the two disagree about
    /// the layout even though every individual read succeeded.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        // `checked_add` rather than `self.pos + n`: `n` is attacker-controlled
        // in the sense that matters — it came from a length prefix in the
        // payload — and a wrapping add would turn a huge length into a small
        // one that passes the bounds check.
        let end = self.pos.checked_add(n).ok_or(WireError::Truncated)?;
        if end > self.buf.len() {
            return Err(WireError::Truncated);
        }
        let out = &self.buf[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, WireError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, WireError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> Result<u64, WireError> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn i64(&mut self) -> Result<i64, WireError> {
        Ok(self.u64()? as i64)
    }

    pub fn f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_bits(self.u32()?))
    }

    pub fn bool(&mut self) -> Result<bool, WireError> {
        Ok(self.u8()? != 0)
    }

    pub fn count(&mut self) -> Result<usize, WireError> {
        Ok(self.u32()? as usize)
    }

    pub fn str(&mut self) -> Result<&'a str, WireError> {
        let n = self.count()?;
        let b = self.take(n)?;
        core::str::from_utf8(b).map_err(|_| WireError::BadUtf8)
    }

    pub fn string(&mut self) -> Result<String, WireError> {
        Ok(self.str()?.to_string())
    }

    pub fn bytes_field(&mut self) -> Result<&'a [u8], WireError> {
        let n = self.count()?;
        self.take(n)
    }

    pub fn opt_str(&mut self) -> Result<Option<&'a str>, WireError> {
        if self.bool()? {
            Ok(Some(self.str()?))
        } else {
            Ok(None)
        }
    }

    pub fn opt_string(&mut self) -> Result<Option<String>, WireError> {
        Ok(self.opt_str()?.map(str::to_string))
    }

    pub fn opt_u64(&mut self) -> Result<Option<u64>, WireError> {
        if self.bool()? {
            Ok(Some(self.u64()?))
        } else {
            Ok(None)
        }
    }

    pub fn opt_f32(&mut self) -> Result<Option<f32>, WireError> {
        if self.bool()? {
            Ok(Some(self.f32()?))
        } else {
            Ok(None)
        }
    }

    pub fn f32x2(&mut self) -> Result<[f32; 2], WireError> {
        Ok([self.f32()?, self.f32()?])
    }

    pub fn f32x3(&mut self) -> Result<[f32; 3], WireError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    pub fn f32x4(&mut self) -> Result<[f32; 4], WireError> {
        Ok([self.f32()?, self.f32()?, self.f32()?, self.f32()?])
    }

    /// Decode a length-prefixed list.
    ///
    /// Does **not** pre-allocate from the count. The count is untrusted, and
    /// `Vec::with_capacity(u32::MAX as usize)` on a corrupt payload is an
    /// instant multi-gigabyte allocation — the decode would have failed a
    /// moment later on [`WireError::Truncated`] anyway, but only after the
    /// allocator had already been asked.
    pub fn list<T>(
        &mut self,
        mut item: impl FnMut(&mut Self) -> Result<T, WireError>,
    ) -> Result<Vec<T>, WireError> {
        let n = self.count()?;
        let mut out = Vec::new();
        for _ in 0..n {
            out.push(item(self)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_round_trip() {
        let mut w = Writer::new();
        w.u8(7);
        w.u16(1000);
        w.u32(70_000);
        w.u64(5_000_000_000);
        w.i64(-42);
        w.f32(1.5);
        w.bool(true);
        w.bool(false);

        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u8().unwrap(), 7);
        assert_eq!(r.u16().unwrap(), 1000);
        assert_eq!(r.u32().unwrap(), 70_000);
        assert_eq!(r.u64().unwrap(), 5_000_000_000);
        assert_eq!(r.i64().unwrap(), -42);
        assert_eq!(r.f32().unwrap(), 1.5);
        assert!(r.bool().unwrap());
        assert!(!r.bool().unwrap());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn strings_and_options_round_trip() {
        let mut w = Writer::new();
        w.str("hello");
        w.str("");
        w.opt_str(Some("there"));
        w.opt_str(None);
        w.opt_u64(Some(9));
        w.opt_u64(None);
        w.f32x3([1.0, 2.0, 3.0]);

        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.str().unwrap(), "hello");
        assert_eq!(r.str().unwrap(), "");
        assert_eq!(r.opt_str().unwrap(), Some("there"));
        assert_eq!(r.opt_str().unwrap(), None);
        assert_eq!(r.opt_u64().unwrap(), Some(9));
        assert_eq!(r.opt_u64().unwrap(), None);
        assert_eq!(r.f32x3().unwrap(), [1.0, 2.0, 3.0]);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn lists_round_trip() {
        let mut w = Writer::new();
        w.count(3);
        for s in ["a", "bb", "ccc"] {
            w.str(s);
        }

        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        let out = r.list(|r| r.string()).unwrap();
        assert_eq!(out, ["a", "bb", "ccc"]);
    }

    #[test]
    fn truncated_payload_errors_rather_than_panicking() {
        let mut w = Writer::new();
        w.str("hello");
        let mut bytes = w.into_bytes();
        bytes.truncate(6); // length prefix plus two of the five bytes

        let mut r = Reader::new(&bytes);
        assert_eq!(r.str(), Err(WireError::Truncated));
    }

    #[test]
    fn absurd_length_prefix_errors_rather_than_allocating() {
        // A four-billion-element list header with no elements behind it. The
        // decode must fail on the first item, not on the pre-allocation.
        let mut w = Writer::new();
        w.count(u32::MAX as usize);
        let bytes = w.into_bytes();

        let mut r = Reader::new(&bytes);
        let out: Result<Vec<u64>, _> = r.list(|r| r.u64());
        assert_eq!(out, Err(WireError::Truncated));
    }

    #[test]
    fn non_utf8_string_errors() {
        let mut w = Writer::new();
        w.count(2);
        let mut bytes = w.into_bytes();
        bytes.extend_from_slice(&[0xff, 0xfe]);

        let mut r = Reader::new(&bytes);
        assert_eq!(r.str(), Err(WireError::BadUtf8));
    }

    #[test]
    fn reading_past_the_end_errors() {
        let bytes: [u8; 0] = [];
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u8(), Err(WireError::Truncated));
        assert_eq!(r.u32(), Err(WireError::Truncated));
        assert_eq!(r.str(), Err(WireError::Truncated));
    }
}
