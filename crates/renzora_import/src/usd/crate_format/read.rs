//! Little-endian scalar reads that cannot panic, whatever the file says.
//!
//! Every reader here replaces one instance of
//!
//! ```ignore
//! u64::from_le_bytes(data[off..off + 8].try_into().unwrap())
//! ```
//!
//! and the `unwrap` was the least interesting part of that line. It could not
//! fail: `try_into` converts a slice to `[u8; 8]`, the range is a literal 8
//! wide, so the conversion is infallible wherever the indexing succeeded. It
//! read as a hazard and was in fact unreachable.
//!
//! The hazard is the `off + 8`. Every offset in this parser is a `u64` lifted
//! straight out of the file (`header.toc_offset`, a section's `offset`/`size`,
//! a position accumulated from those), and the guards written against them all
//! had the shape
//!
//! ```ignore
//! if off + 8 > data.len() { return Err(...) }
//! ```
//!
//! which is not a guard when `off` is hostile. `off + 8` wraps at `usize::MAX`,
//! and this crate builds under `[profile.dist]`, which inherits `release` and
//! therefore has `overflow-checks` off: the addition wraps in silence rather
//! than panicking. A `toc_offset` of `u64::MAX` makes `off + 8` equal 7, sails
//! past a comparison against any non-trivial length, and panics one line later
//! inside the slice index with `slice index starts at 18446744073709551615 but
//! ends at 7`. The panic names an offset nobody can connect to the malformed
//! file that produced it, and it takes the editor with it.
//!
//! So the arithmetic is `checked_add` and the indexing is `get`. A truncated or
//! crafted file yields `None`, the caller turns that into a `Parse` error naming
//! the section, and the importer rejects the asset while the editor keeps
//! running. That is the whole point: an import failure is a message, never a
//! crash.

/// `N` bytes at `off`, or `None` if they are not both in range and in bounds.
///
/// The single place the two checks live. `checked_add` is what stops a hostile
/// offset from wrapping into a plausible-looking one, and `get` is what turns
/// "past the end" into a value instead of a panic.
fn at<const N: usize>(data: &[u8], off: usize) -> Option<[u8; N]> {
    let end = off.checked_add(N)?;
    data.get(off..end)?.try_into().ok()
}

pub(crate) fn le_u64(data: &[u8], off: usize) -> Option<u64> {
    at::<8>(data, off).map(u64::from_le_bytes)
}

pub(crate) fn le_u32(data: &[u8], off: usize) -> Option<u32> {
    at::<4>(data, off).map(u32::from_le_bytes)
}

pub(crate) fn le_i32(data: &[u8], off: usize) -> Option<i32> {
    at::<4>(data, off).map(i32::from_le_bytes)
}

pub(crate) fn le_i16(data: &[u8], off: usize) -> Option<i16> {
    at::<2>(data, off).map(i16::from_le_bytes)
}

/// The half-open range `off .. off + len`, or `None` if it does not fit.
///
/// For the section reads, where the length is itself out of the file and so is
/// subject to the same wrap: `s + section.size` overflowing produced an `e`
/// SMALLER than `s`, which passes an `e > data.len()` test and then panics in
/// `&data[s..e]` with "slice index starts at N but ends at M".
pub(crate) fn slice(data: &[u8], off: usize, len: usize) -> Option<&[u8]> {
    let end = off.checked_add(len)?;
    data.get(off..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression the module exists for: an offset near the top of the
    /// range used to wrap past its own bounds check and panic in the index.
    #[test]
    fn a_hostile_offset_reports_rather_than_wrapping() {
        let data = [0u8; 32];
        // `usize::MAX + 8` wraps to 7, which is < 32 and would have passed
        // `if off + 8 > data.len()`.
        assert_eq!(le_u64(&data, usize::MAX), None);
        assert_eq!(le_u64(&data, usize::MAX - 4), None);
        assert_eq!(slice(&data, usize::MAX, 8), None);
    }

    #[test]
    fn past_the_end_is_none_not_a_panic() {
        let data = [1u8, 2, 3, 4];
        assert_eq!(le_u64(&data, 0), None);
        assert_eq!(le_u32(&data, 1), None);
        assert_eq!(slice(&data, 2, 4), None);
    }

    #[test]
    fn a_read_that_fits_still_reads() {
        let data = [1u8, 0, 0, 0, 0, 0, 0, 0, 9, 9];
        assert_eq!(le_u64(&data, 0), Some(1));
        assert_eq!(le_u32(&data, 0), Some(1));
        assert_eq!(le_i32(&data, 0), Some(1));
        assert_eq!(le_i16(&data, 0), Some(1));
        assert_eq!(slice(&data, 8, 2), Some(&[9u8, 9][..]));
        // Exactly reaching the end is in bounds, one past it is not.
        assert_eq!(slice(&data, 8, 3), None);
    }
}
