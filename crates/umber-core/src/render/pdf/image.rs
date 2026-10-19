//! Images: the pixels of an [`Item::Image`](crate::Item::Image) as a Flate-compressed RGB image
//! XObject, with its alpha as a grayscale soft mask when any pixel is not opaque. Both are
//! marked `/Interpolate false`, so viewers scale the pixels without smoothing and cells stay
//! sharp at any zoom.
//!
//! Cell images are compressed at low deflate levels, chosen by measurement on cell images (a
//! heatmap of noisy values, a noisy smooth field, a smooth mesh, a 2-D histogram with empty
//! cells, and noise): level 2 compresses a heatmap of a colormap's colors smaller than the
//! default level 6 in a fifth of the time, and an image it shrinks to a third or less, such as
//! a smooth field, is compressed again at level 3, which costs little on such images and
//! shrinks them further. The samples are
//! written as they are: PNG-style prediction made every measured cell image larger, since
//! neighbouring cells repeat colormap colors rather than vary smoothly in each channel.

use pdf_writer::{Chunk, Filter, Finish, Ref};

use super::write::{Refs, compress_at};
use crate::color::Rgba;

/// The deflate level every image stream is compressed at first.
const FIRST_LEVEL: u8 = 2;

/// The deflate level an image stream that [`FIRST_LEVEL`] shrinks to a third or less is
/// compressed at again.
const SECOND_LEVEL: u8 = 3;

/// Image streams of at most this many bytes are compressed once.
const ONE_PASS_BYTES: usize = 32 * 1024;

/// `samples` compressed for an image stream: at [`FIRST_LEVEL`], and when they are larger than
/// [`ONE_PASS_BYTES`] and that shrinks them to a third or less, at [`SECOND_LEVEL`] too,
/// keeping the smaller.
fn compressed(samples: &[u8]) -> Vec<u8> {
    let first = compress_at(samples, FIRST_LEVEL);
    if samples.len() > ONE_PASS_BYTES && first.len() <= samples.len() / 3 {
        let second = compress_at(samples, SECOND_LEVEL);
        if second.len() < first.len() {
            return second;
        }
    }
    first
}

/// The pixel count of an image side as PDF writes it, `None` beyond what PDF integers hold.
pub(super) fn side(pixels: u32) -> Option<i32> {
    i32::try_from(pixels).ok()
}

/// Writes the `width × height` image of `pixels`, row by row from the top, and returns its
/// object. The sides must fit [`side`].
pub(super) fn write_image(
    chunk: &mut Chunk,
    refs: &mut Refs,
    (width, height): (i32, i32),
    pixels: &[Rgba],
) -> Ref {
    let id = refs.next();
    let mask = pixels.iter().any(|p| p.a < 255).then(|| refs.next());
    let rgb: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
    let data = compressed(&rgb);
    let mut image = chunk.image_xobject(id, &data);
    image.filter(Filter::FlateDecode);
    image.width(width).height(height).bits_per_component(8);
    image.color_space().device_rgb();
    image.interpolate(false);
    if let Some(mask) = mask {
        image.s_mask(mask);
    }
    image.finish();
    if let Some(mask) = mask {
        let alpha: Vec<u8> = pixels.iter().map(|p| p.a).collect();
        let data = compressed(&alpha);
        let mut soft = chunk.image_xobject(mask, &data);
        soft.filter(Filter::FlateDecode);
        soft.width(width).height(height).bits_per_component(8);
        soft.color_space().device_gray();
        soft.interpolate(false);
        soft.finish();
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba = Rgba {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };

    fn written(pixels: &[Rgba], size: (i32, i32)) -> String {
        let mut chunk = Chunk::new();
        let mut refs = Refs::new();
        write_image(&mut chunk, &mut refs, size, pixels);
        String::from_utf8_lossy(chunk.as_bytes()).into_owned()
    }

    #[test]
    fn opaque_images_have_no_mask() {
        let text = written(&[RED, RED], (2, 1));
        assert!(text.contains("/Subtype /Image"));
        assert!(text.contains("/Interpolate false"));
        assert!(text.contains("/ColorSpace /DeviceRGB"));
        assert!(!text.contains("/SMask"));
        assert_eq!(text.matches("/Subtype /Image").count(), 1);
    }

    #[test]
    fn translucent_pixels_add_a_grayscale_mask() {
        let clear = Rgba { a: 0, ..RED };
        let text = written(&[RED, clear], (1, 2));
        assert!(text.contains("/SMask 2 0 R"));
        assert_eq!(text.matches("/Subtype /Image").count(), 2);
        assert_eq!(text.matches("/Interpolate false").count(), 2);
        assert!(text.contains("/ColorSpace /DeviceGray"));
    }

    /// The pixels of a deterministic pattern: `kind` 0 is noise, 1 blocks of 10 × 10 cells of
    /// a gradient, 2 noise with transparent holes.
    fn pattern(width: usize, height: usize, kind: u8) -> Vec<Rgba> {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        (0..width * height)
            .map(|i| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let [r, g, b, a, ..] = state.to_le_bytes();
                let (x, y) = (i % width, i / width);
                match kind {
                    0 => Rgba { r, g, b, a: 255 },
                    1 => Rgba {
                        r: (x / 10 * 255 / width) as u8,
                        g: (y / 10 * 255 / height) as u8,
                        b: 40,
                        a: 255,
                    },
                    _ => Rgba {
                        r,
                        g,
                        b,
                        a: if a < 96 { 0 } else { a },
                    },
                }
            })
            .collect()
    }

    /// The decoded samples of every image stream of `pdf`, in order.
    fn decoded_images(pdf: &[u8]) -> Vec<Vec<u8>> {
        let find = |from: usize, needle: &[u8]| {
            pdf[from..]
                .windows(needle.len())
                .position(|w| w == needle)
                .map(|at| from + at)
        };
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(at) = find(from, b"/Subtype /Image") {
            let start = pdf[..at].windows(4).rposition(|w| w == b" obj").unwrap();
            let stream = find(at, b"stream\n").unwrap() + 7;
            let dict = String::from_utf8_lossy(&pdf[start..stream]).into_owned();
            assert!(dict.contains("/Filter /FlateDecode") && !dict.contains("/DecodeParms"));
            let length: usize = dict[dict.find("/Length ").unwrap() + 8..]
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .unwrap()
                .parse()
                .unwrap();
            out.push(
                miniz_oxide::inflate::decompress_to_vec_zlib(&pdf[stream..stream + length])
                    .unwrap(),
            );
            from = stream;
        }
        out
    }

    #[test]
    fn image_streams_decode_to_the_pixels() {
        for (width, height, kind) in [
            (37, 23, 0),
            (64, 40, 2),
            (1, 6000, 0),
            (6000, 1, 2),
            (300, 200, 1),
            (300, 200, 2),
        ] {
            let pixels = pattern(width, height, kind);
            let size = (width as i32, height as i32);
            let mut chunk = Chunk::new();
            write_image(&mut chunk, &mut Refs::new(), size, &pixels);
            let images = decoded_images(chunk.as_bytes());
            let rgb: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
            assert_eq!(images[0], rgb, "{width} x {height}, kind {kind}");
            if pixels.iter().any(|p| p.a < 255) {
                let alpha: Vec<u8> = pixels.iter().map(|p| p.a).collect();
                assert_eq!(images.len(), 2);
                assert_eq!(images[1], alpha, "{width} x {height}, kind {kind}");
            } else {
                assert_eq!(images.len(), 1, "an opaque image has no mask");
            }
            // The same pixels always give the same bytes.
            let mut again = Chunk::new();
            write_image(&mut again, &mut Refs::new(), size, &pixels);
            assert_eq!(chunk.as_bytes(), again.as_bytes());
        }
    }

    #[test]
    fn compressible_images_are_compressed_twice_and_keep_the_smaller() {
        let smooth: Vec<u8> = pattern(300, 200, 1)
            .iter()
            .flat_map(|p| [p.r, p.g, p.b])
            .collect();
        let first = compress_at(&smooth, FIRST_LEVEL);
        assert!(first.len() <= smooth.len() / 3);
        let expected = first.len().min(compress_at(&smooth, SECOND_LEVEL).len());
        assert_eq!(compressed(&smooth).len(), expected);
        // Noise does not shrink, and a small image is compressed once.
        let noise: Vec<u8> = pattern(300, 200, 0)
            .iter()
            .flat_map(|p| [p.r, p.g, p.b])
            .collect();
        assert_eq!(compressed(&noise), compress_at(&noise, FIRST_LEVEL));
        let small = &smooth[..ONE_PASS_BYTES];
        assert_eq!(compressed(small), compress_at(small, FIRST_LEVEL));
    }

    #[test]
    fn sides_beyond_pdf_integers_are_refused() {
        assert_eq!(side(7), Some(7));
        assert_eq!(side(u32::MAX), None);
    }
}
