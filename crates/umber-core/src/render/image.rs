//! Images shared by the render backends: the exact area-weighted resampling of an image's pixels
//! into the PNG backend's device pixels, and the PNG data URI the SVG backend writes.
//!
//! An image pixel is a solid rectangle of its color. A device pixel takes the sum, over the
//! image pixels, of each one's premultiplied color times the area of the device pixel it
//! covers. Every image pixel covers a product of an interval along x and one along y, so the
//! sum is computed one axis at a time: first along the axis where that leaves less to do, then
//! along the other. The work is proportional to the device pixels plus the image pixels.

use crate::color::Rgba;

/// A premultiplied color, each channel in `0..=255`.
type Premultiplied = [f32; 4];

/// One overlap of a device pixel and an image pixel along one axis.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Overlap {
    /// The device pixel, counted from the first one resampled.
    device: usize,
    /// The image pixel.
    pixel: usize,
    /// The length of the overlap, in device pixels, in `0..=1`.
    length: f32,
}

/// The overlaps of the device pixels `first..first + count` along one axis with the `pixels`
/// image pixels that divide `start..end` (device coordinates, `start < end`) into equal parts,
/// by device pixel and then by image pixel. Overlaps of length zero are left out.
fn overlaps(start: f64, end: f64, pixels: usize, first: usize, count: usize) -> Vec<Overlap> {
    let size = (end - start) / pixels as f64;
    // Each boundary is computed one way only, so neighbouring pixels share it exactly.
    let edge = |i: usize| {
        if i == pixels {
            end
        } else {
            start + size * i as f64
        }
    };
    let mut out = Vec::with_capacity(count + pixels);
    for device in 0..count {
        let (lo, hi) = ((first + device) as f64, (first + device + 1) as f64);
        let mut pixel = (((lo - start) / size).floor().max(0.0) as usize).min(pixels - 1);
        // The division can land one pixel late; step back while the previous one still reaches in.
        while pixel > 0 && edge(pixel) > lo {
            pixel -= 1;
        }
        while pixel < pixels && edge(pixel) < hi {
            let length = edge(pixel + 1).min(hi) - edge(pixel).max(lo);
            if length > 0.0 {
                out.push(Overlap {
                    device,
                    pixel,
                    length: length as f32,
                });
            }
            pixel += 1;
        }
    }
    out
}

/// The range of image pixels that `overlaps` touch; empty when there is none.
fn touched(overlaps: &[Overlap]) -> std::ops::Range<usize> {
    let lo = overlaps.iter().map(|o| o.pixel).min().unwrap_or(0);
    let hi = overlaps.iter().map(|o| o.pixel + 1).max().unwrap_or(0);
    lo..hi
}

/// The device pixels `columns` by `rows` (ranges of whole device pixels) of an image of
/// `width` by `height` `pixels` that covers `rect` (left, top, right and bottom, in device
/// coordinates, with left < right and top < bottom), each the area-weighted sum of the
/// premultiplied colors of the image pixels it covers, row by row from the top. A device pixel
/// fully inside one image pixel is exactly that pixel's color; one the image covers in part is
/// as transparent as the part it leaves uncovered.
pub(super) fn resample(
    rect: [f64; 4],
    (width, height): (usize, usize),
    pixels: &[Rgba],
    (columns, rows): (std::ops::Range<usize>, std::ops::Range<usize>),
) -> Vec<Premultiplied> {
    let [left, top, right, bottom] = rect;
    let (device_w, device_h) = (columns.len(), rows.len());
    let along_x = overlaps(left, right, width, columns.start, device_w);
    let along_y = overlaps(top, bottom, height, rows.start, device_h);
    let (used_x, used_y) = (touched(&along_x), touched(&along_y));
    let (used_w, used_h) = (used_x.len(), used_y.len());
    let source = |row: usize, column: usize| -> Premultiplied {
        let c = pixels[row * width + column];
        let a = f32::from(c.a) / 255.0;
        [
            f32::from(c.r) * a,
            f32::from(c.g) * a,
            f32::from(c.b) * a,
            f32::from(c.a),
        ]
    };
    let add = |into: &mut Premultiplied, from: Premultiplied, weight: f32| {
        for (to, value) in into.iter_mut().zip(from) {
            *to += value * weight;
        }
    };
    let mut out = vec![[0.0; 4]; device_w * device_h];
    // Resampling along x first costs a pass over the used rows for each x overlap, then one
    // over the device columns for each y overlap; the other order the reverse.
    let x_first = used_h * along_x.len() + device_w * along_y.len();
    let y_first = used_w * along_y.len() + device_h * along_x.len();
    if x_first <= y_first {
        // The used rows, each resampled to the device columns.
        let mut rows_out = vec![[0.0; 4]; used_h * device_w];
        for (r, row) in used_y.clone().enumerate() {
            for o in &along_x {
                add(
                    &mut rows_out[r * device_w + o.device],
                    source(row, o.pixel),
                    o.length,
                );
            }
        }
        for o in &along_y {
            let r = o.pixel - used_y.start;
            for column in 0..device_w {
                let value = rows_out[r * device_w + column];
                add(&mut out[o.device * device_w + column], value, o.length);
            }
        }
    } else {
        // The used columns, each resampled to the device rows.
        let mut columns_out = vec![[0.0; 4]; device_h * used_w];
        for o in &along_y {
            for (c, column) in used_x.clone().enumerate() {
                add(
                    &mut columns_out[o.device * used_w + c],
                    source(o.pixel, column),
                    o.length,
                );
            }
        }
        for row in 0..device_h {
            for o in &along_x {
                let value = columns_out[row * used_w + (o.pixel - used_x.start)];
                add(&mut out[row * device_w + o.device], value, o.length);
            }
        }
    }
    out
}

/// The PNG encoding of `width` by `height` `pixels`, row by row from the top, as straight
/// (not premultiplied) 8-bit RGBA; the same pixels always give the same bytes. `None` if the
/// encoder refuses them, which needs more pixels than any scene holds.
pub(super) fn png_bytes(width: u32, height: u32, pixels: &[Rgba]) -> Option<Vec<u8>> {
    let data: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
    let mut out = Vec::new();
    let mut encoder = ::png::Encoder::new(&mut out, width, height);
    encoder.set_color(::png::ColorType::Rgba);
    encoder.set_depth(::png::BitDepth::Eight);
    encoder.set_deflate_compression(::png::DeflateCompression::Level(6));
    encoder.set_filter(::png::Filter::Adaptive);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&data).ok()?;
    writer.finish().ok()?;
    Some(out)
}

/// `bytes` in standard base64 (RFC 4648, with `+`, `/` and `=` padding), as data URIs carry it.
pub(super) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{SvgText, render_png, render_svg};
    use crate::scene::{Item, Rect, Scene};
    use crate::test_support::image::decode_rgba8;

    const WHITE: Rgba = Rgba {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    const RED: Rgba = Rgba {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    const BLUE: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };
    const CLEAR: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(base64(plain.as_bytes()), encoded, "{plain:?}");
        }
        assert_eq!(base64(&[0xFB, 0xFF, 0xBF]), "+/+/");
    }

    #[test]
    fn overlaps_cover_each_device_pixel_by_the_image_it_holds() {
        // Three image pixels over device x 0.5..3.5: pixel i spans 0.5 + i .. 1.5 + i.
        let found = overlaps(0.5, 3.5, 3, 0, 4);
        let lengths: Vec<(usize, usize, f32)> = found
            .iter()
            .map(|o| (o.device, o.pixel, o.length))
            .collect();
        assert_eq!(
            lengths,
            [
                (0, 0, 0.5),
                (1, 0, 0.5),
                (1, 1, 0.5),
                (2, 1, 0.5),
                (2, 2, 0.5),
                (3, 2, 0.5)
            ]
        );
        // Many image pixels in one device pixel share it in full.
        let total: f32 = overlaps(0.0, 1.0, 1000, 0, 1)
            .iter()
            .map(|o| o.length)
            .sum();
        assert!((total - 1.0).abs() < 1e-4, "{total}");
    }

    #[test]
    fn resampling_keeps_interiors_and_averages_edges() {
        // Two image pixels, red then blue, over device x 0..4 and y 0..1.
        let out = resample([0.0, 0.0, 4.0, 1.0], (2, 1), &[RED, BLUE], (0..4, 0..1));
        assert_eq!(out[0], [255.0, 0.0, 0.0, 255.0]);
        assert_eq!(out[3], [0.0, 0.0, 255.0, 255.0]);
        // Offset by half a device pixel, the shared edge falls inside device pixel 2.
        let out = resample([0.5, 0.0, 4.5, 1.0], (2, 1), &[RED, BLUE], (0..5, 0..1));
        assert_eq!(out[0], [127.5, 0.0, 0.0, 127.5]);
        assert_eq!(out[1], [255.0, 0.0, 0.0, 255.0]);
        assert_eq!(out[2], [127.5, 0.0, 127.5, 255.0]);
        assert_eq!(out[4], [0.0, 0.0, 127.5, 127.5]);
        // A transparent pixel contributes nothing.
        let out = resample([0.0, 0.0, 2.0, 2.0], (1, 2), &[CLEAR, RED], (0..2, 0..2));
        assert_eq!(out[0], [0.0; 4]);
        assert_eq!(out[2], [255.0, 0.0, 0.0, 255.0]);
    }

    #[test]
    fn both_resampling_orders_agree() {
        let pixels: Vec<Rgba> = (0..60)
            .map(|i| Rgba {
                r: (i * 4) as u8,
                g: (255 - i * 3) as u8,
                b: (i * 7 % 256) as u8,
                a: 255,
            })
            .collect();
        // Wide (10 x 6) over a tall region and tall (6 x 10) over a wide one take different
        // orders; each is checked against a direct sum over every pair of pixels.
        for ((w, h), rect, region) in [
            ((10, 6), [0.3, 0.7, 3.9, 40.2], (0..4, 0..41)),
            ((6, 10), [0.3, 0.7, 40.2, 3.9], (0..41, 0..4)),
        ] {
            let out = resample(rect, (w, h), &pixels, region.clone());
            let device_w = region.0.len();
            let xs = overlaps(rect[0], rect[2], w, 0, region.0.len());
            let ys = overlaps(rect[1], rect[3], h, 0, region.1.len());
            for dy in region.1.clone() {
                for dx in region.0.clone() {
                    let mut want = [0.0_f64; 4];
                    for oy in ys.iter().filter(|o| o.device == dy) {
                        for ox in xs.iter().filter(|o| o.device == dx) {
                            let c = pixels[oy.pixel * w + ox.pixel];
                            let weight = f64::from(oy.length) * f64::from(ox.length);
                            for (to, v) in want.iter_mut().zip([c.r, c.g, c.b, c.a]) {
                                *to += f64::from(v) * weight;
                            }
                        }
                    }
                    let got = out[dy * device_w + dx];
                    for (g, w) in got.iter().zip(want) {
                        assert!((f64::from(*g) - w).abs() < 1e-2, "{got:?} {want:?}");
                    }
                }
            }
        }
    }

    /// A scene of 40 x 30 pt on white holding `item`.
    fn scene_with(item: Item) -> Scene {
        let mut scene = Scene::new(40.0, 30.0, WHITE);
        scene.push(item);
        scene
    }

    /// Two image pixels, red then blue, over 10..30 x 5..25 pt.
    fn two_pixels() -> Item {
        Item::Image {
            rect: Rect {
                x: 10.0,
                y: 5.0,
                width: 20.0,
                height: 20.0,
            },
            width: 2,
            height: 1,
            pixels: vec![RED, BLUE],
        }
    }

    #[test]
    fn png_draws_pixels_as_rectangles_through_the_clip() {
        let png = |scene: &Scene| decode_rgba8(&render_png(scene, 72).unwrap()).unwrap();
        let image = png(&scene_with(two_pixels()));
        assert_eq!(image.at(5, 10), [255, 255, 255, 255]);
        assert_eq!(image.at(10, 5), [255, 0, 0, 255]);
        assert_eq!(image.at(19, 24), [255, 0, 0, 255]);
        assert_eq!(image.at(20, 5), [0, 0, 255, 255]);
        assert_eq!(image.at(29, 24), [0, 0, 255, 255]);
        assert_eq!(image.at(30, 10), [255, 255, 255, 255]);
        // A clip at x 15..25 keeps only the middle of the image.
        let clipped = png(&scene_with(Item::Group {
            clip: Some(Rect {
                x: 15.0,
                y: 0.0,
                width: 10.0,
                height: 30.0,
            }),
            items: vec![two_pixels()],
        }));
        assert_eq!(clipped.at(14, 10), [255, 255, 255, 255]);
        assert_eq!(clipped.at(15, 10), [255, 0, 0, 255]);
        assert_eq!(clipped.at(24, 10), [0, 0, 255, 255]);
        assert_eq!(clipped.at(25, 10), [255, 255, 255, 255]);
    }

    #[test]
    fn png_averages_pixels_smaller_than_a_device_pixel() {
        // 40 columns alternating red and blue over 4 pt: each device pixel at 72 dpi covers
        // ten, five of each.
        let pixels: Vec<Rgba> = (0..40)
            .map(|i| if i % 2 == 0 { RED } else { BLUE })
            .collect();
        let scene = scene_with(Item::Image {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 1.0,
            },
            width: 40,
            height: 1,
            pixels,
        });
        let image = decode_rgba8(&render_png(&scene, 72).unwrap()).unwrap();
        for x in 0..4 {
            assert_eq!(image.at(x, 0), [128, 0, 128, 255], "{x}");
        }
    }

    #[test]
    fn invalid_images_are_skipped_by_both_backends() {
        let empty = Scene::new(40.0, 30.0, WHITE);
        let invalid = |rect: Rect, width: u32, height: u32, pixels: Vec<Rgba>| Item::Image {
            rect,
            width,
            height,
            pixels,
        };
        let good = Rect {
            x: 1.0,
            y: 1.0,
            width: 10.0,
            height: 10.0,
        };
        let cases = [
            invalid(good, 2, 1, vec![RED]),
            invalid(good, 0, 0, Vec::new()),
            invalid(
                Rect {
                    x: f32::NAN,
                    ..good
                },
                1,
                1,
                vec![RED],
            ),
            invalid(Rect { width: 0.0, ..good }, 1, 1, vec![RED]),
            invalid(Rect { x: 2.0e6, ..good }, 1, 1, vec![RED]),
            invalid(good, 1, 1, vec![CLEAR]),
        ];
        for item in cases {
            let scene = scene_with(item.clone());
            assert_eq!(
                render_png(&scene, 72).unwrap(),
                render_png(&empty, 72).unwrap(),
                "{item:?}"
            );
            assert_eq!(
                render_svg(&scene, SvgText::Outlines),
                render_svg(&empty, SvgText::Outlines),
                "{item:?}"
            );
        }
    }

    #[test]
    fn svg_writes_an_unsmoothed_image_with_a_png_data_uri() {
        let svg = render_svg(&scene_with(two_pixels()), SvgText::Outlines);
        let uri = base64(&png_bytes(2, 1, &[RED, BLUE]).unwrap());
        let expected = format!(
            "<image x=\"10\" y=\"5\" width=\"20\" height=\"20\" preserveAspectRatio=\"none\" \
             image-rendering=\"optimizeSpeed\" class=\"umber-raster\" \
             href=\"data:image/png;base64,{uri}\"/>\n"
        );
        assert!(svg.contains(&expected), "{svg}");
        // One stylesheet, right after the root element, however many images there are.
        let style = "<style>@supports (image-rendering: pixelated) { .umber-raster { \
                     image-rendering: pixelated } }</style>\n";
        let mut two = scene_with(two_pixels());
        two.push(two_pixels());
        let svg = render_svg(&two, SvgText::Editable);
        assert_eq!(svg.matches("<style>").count(), 1, "{svg}");
        assert_eq!(svg.split('\n').nth(1), Some(style.trim_end()));
        assert_eq!(svg.matches("class=\"umber-raster\"").count(), 2);
        // A document without an image has no stylesheet.
        let plain = render_svg(&Scene::new(40.0, 30.0, WHITE), SvgText::Outlines);
        assert!(!plain.contains("<style"), "{plain}");
    }

    #[test]
    fn png_bytes_are_deterministic_and_decode_to_the_pixels() {
        let pixels = [RED, BLUE, CLEAR, RED];
        let bytes = png_bytes(2, 2, &pixels).unwrap();
        assert_eq!(png_bytes(2, 2, &pixels).unwrap(), bytes);
        let image = decode_rgba8(&bytes).unwrap();
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(image.at(1, 0), [0, 0, 255, 255]);
        assert_eq!(image.at(0, 1), [0, 0, 0, 0]);
    }
}
