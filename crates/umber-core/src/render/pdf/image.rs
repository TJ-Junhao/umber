//! Images: the pixels of an [`Item::Image`](crate::Item::Image) as a Flate-compressed RGB image
//! XObject, with its alpha as a grayscale soft mask when any pixel is not opaque. Both are
//! marked `/Interpolate false`, so viewers scale the pixels without smoothing and cells stay
//! sharp at any zoom.

use pdf_writer::{Chunk, Filter, Finish, Ref};

use super::write::{Refs, compress};
use crate::color::Rgba;

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
    let data = compress(&rgb);
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
        let data = compress(&alpha);
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

    #[test]
    fn sides_beyond_pdf_integers_are_refused() {
        assert_eq!(side(7), Some(7));
        assert_eq!(side(u32::MAX), None);
    }
}
