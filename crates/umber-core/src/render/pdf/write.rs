//! The low-level pieces every part of the PDF backend writes with: numbers as the document
//! writes them, object numbers, stream compression and content hashes.

use pdf_writer::Ref;

/// Limits a PDF scalar to the reader range, without rounding its fractional part.
///
/// A whole-valued float is written as an integer. Keeping its magnitude at most 1e9 puts it
/// inside Annex C's signed 32-bit integer range, as well as the real-number range. Subnormal
/// reals become zero. Geometry is already normalized and clipped; this also bounds widths,
/// dash lengths and derived bounds, which can be much larger than their path coordinates.
pub(super) fn limited(v: f32) -> f32 {
    let v = v.clamp(-1.0e9, 1.0e9);
    if v.abs() < f32::MIN_POSITIVE { 0.0 } else { v }
}

/// `v` limited to the reader range and rounded to three decimals, with `-0` as `0`.
/// Callers discard non-finite scene values before writing; derived overflow saturates here.
pub(super) fn rounded(v: f32) -> f32 {
    let r = ((f64::from(limited(v)) * 1000.0).round() / 1000.0) as f32;
    if r == 0.0 { 0.0 } else { r }
}

/// A color channel or an opacity of `0..=255` as a PDF number in `0..=1`.
pub(super) fn unit(channel: u8) -> f32 {
    rounded(f32::from(channel) / 255.0)
}

/// Allocates object numbers in order, from 1.
#[derive(Debug)]
pub(super) struct Refs(i32);

impl Refs {
    pub(super) fn new() -> Refs {
        Refs(1)
    }

    /// The next unused object number.
    pub(super) fn next(&mut self) -> Ref {
        let id = Ref::new(self.0);
        self.0 += 1;
        id
    }
}

/// `data` compressed for a `FlateDecode` stream (zlib format), at a fixed level, so the same
/// data always gives the same bytes.
pub(super) fn compress(data: &[u8]) -> Vec<u8> {
    compress_at(data, 6)
}

/// `data` compressed for a `FlateDecode` stream at deflate `level` (0 to 10); the same data and
/// level always give the same bytes.
pub(super) fn compress_at(data: &[u8], level: u8) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, level)
}

/// The 64-bit FNV-1a hash of `data`, starting from `basis`.
fn fnv1a(data: &[u8], basis: u64) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    data.iter().fold(basis, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

/// The standard FNV-1a offset basis.
const BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// A 16-byte identifier of a document derived from its body: two FNV-1a hashes with different
/// starting values.
pub(super) fn document_id(body: &[u8]) -> Vec<u8> {
    let first = fnv1a(body, BASIS);
    let second = fnv1a(body, first ^ 0x9e37_79b9_7f4a_7c15);
    [first.to_be_bytes(), second.to_be_bytes()].concat()
}

/// The six uppercase letters that tag a font subset, derived from the subset's bytes.
pub(super) fn subset_tag(program: &[u8]) -> String {
    let mut hash = fnv1a(program, BASIS);
    (0..6)
        .map(|_| {
            let letter = char::from(b'A' + (hash % 26) as u8);
            hash /= 26;
            letter
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_keep_three_decimals_and_no_negative_zero() {
        assert_eq!(rounded(1.234_56), 1.235);
        assert_eq!(rounded(-0.000_1), 0.0);
        assert!(rounded(-0.000_1).is_sign_positive());
        assert_eq!(rounded(14_400.0), 14_400.0);
        assert_eq!(unit(255), 1.0);
        assert_eq!(unit(0), 0.0);
        assert_eq!(unit(128), 0.502);
    }

    #[test]
    fn numbers_stay_within_reader_limits() {
        for value in [f32::MIN, -1.0e30, -1.0e9, 1.0e9, 1.0e30, f32::MAX] {
            let number = rounded(value);
            assert!(
                number.is_finite() && number.abs() <= 1.0e9,
                "{value}: {number}"
            );
            assert_eq!(number.is_sign_negative(), value.is_sign_negative());
        }
        assert_eq!(rounded(f32::MIN_POSITIVE), 0.0);
        assert_eq!(rounded(f32::from_bits(1)), 0.0);
        assert_eq!(limited(f32::MIN_POSITIVE), f32::MIN_POSITIVE);
        assert_eq!(limited(-f32::from_bits(1)), 0.0);
        assert!(limited(-f32::from_bits(1)).is_sign_positive());
        assert_eq!(limited(0.123_456), 0.123_456);
        assert_eq!(rounded(f32::INFINITY), 1.0e9);
        assert_eq!(rounded(f32::NEG_INFINITY), -1.0e9);
    }

    #[test]
    fn object_numbers_count_from_one() {
        let mut refs = Refs::new();
        assert_eq!(refs.next(), Ref::new(1));
        assert_eq!(refs.next(), Ref::new(2));
    }

    #[test]
    fn compression_round_trips_and_is_stable() {
        let data = b"BT /F0 12 Tf ET ".repeat(50);
        let packed = compress(&data);
        assert_eq!(packed, compress(&data));
        assert!(packed.len() < data.len());
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib(&packed).unwrap(),
            data
        );
    }

    #[test]
    fn ids_and_tags_follow_the_content() {
        let id = document_id(b"one");
        assert_eq!(id.len(), 16);
        assert_eq!(id, document_id(b"one"));
        assert_ne!(id, document_id(b"two"));
        let tag = subset_tag(b"font");
        assert_eq!(tag.len(), 6);
        assert!(tag.bytes().all(|b| b.is_ascii_uppercase()), "{tag}");
        assert_eq!(tag, subset_tag(b"font"));
        assert_ne!(tag, subset_tag(b"other font"));
    }
}
