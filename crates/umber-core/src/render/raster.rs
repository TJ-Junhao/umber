//! Items drawn as one image in SVG and PDF output ([`Item::Raster`]): the pixel grid of the
//! image, and its pixels, drawn by the PNG backend.
//!
//! The image lies on the grid of device pixels that a PNG of the scene at the raster resolution
//! has: it covers the item's rectangle widened outward to whole pixels of that grid, and cut to
//! the scene. Its pixels are those the PNG backend draws for the items there, on a transparent
//! background ([`super::png::draw_region`]), so at the raster resolution the image is what a PNG
//! shows of the items, and the edges of the shapes are antialiased as in a PNG. The image is not
//! clipped while it is drawn: it reaches past the rectangle by less than a pixel, and the clip
//! of the groups around the item cuts it exactly where it cuts vector shapes, antialiased by the
//! viewer as theirs are.
//!
//! An image is at most as large as a PNG may be: 32,767 pixels per side and 2^26 pixels in all,
//! and a larger one fails with `Error::Style`. Its pixmap takes 4 bytes per pixel, 256 MiB at
//! that limit. Writing it takes up to about four times as much again for an image that does not
//! compress (the PNG encodings and base64 text of an SVG `<image>`, the color and alpha samples
//! of a PDF image and their compressed streams); the images of marks, mostly transparent,
//! compress to a small part of their pixmap.

use super::normalize::{clip_is_valid, valid_point};
use super::png::{DeviceOrigin, MAX_PIXELS, MAX_SIDE, draw_region};
use crate::error::{Error, Result};
use crate::geometry::Point;
use crate::scene::{Item, Rect};

/// An [`Item::Raster`] drawn: the rectangle its image covers, in points, the image's size in
/// pixels, and its pixels, row by row from the top, as straight (not premultiplied) RGBA bytes.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RasterImage {
    /// The rectangle the pixels cover, in points: whole pixels of the device grid.
    pub(super) rect: Rect,
    /// The number of pixel columns and rows.
    pub(super) size: (u32, u32),
    /// `4 * columns * rows` bytes, RGBA, not premultiplied.
    pub(super) rgba: Vec<u8>,
}

/// The pixels of the device grid at `dpi` that an image covering `rect` takes on a scene of
/// `scene` (width and height) points: the first column and row, and the number of columns and
/// rows. `None` when `rect` cannot be drawn (not finite, not positive in size, or beyond the
/// renderers' coordinate limit) or leaves nothing of the scene.
fn pixel_region(rect: &Rect, scene: (f32, f32), dpi: u32) -> Option<(DeviceOrigin, (f64, f64))> {
    let corner = Point {
        x: rect.x + rect.width,
        y: rect.y + rect.height,
    };
    if !clip_is_valid(rect)
        || !valid_point(Point {
            x: rect.x,
            y: rect.y,
        })
        || !valid_point(corner)
    {
        return None;
    }
    // The scale of the PNG backend, so that the grid is its grid.
    let s = f64::from(dpi as f32 / 72.0);
    let page = |side: f32| {
        if side.is_finite() && side > 0.0 {
            (f64::from(side) * s).ceil()
        } else {
            0.0
        }
    };
    let left = (f64::from(rect.x) * s).floor().max(0.0);
    let top = (f64::from(rect.y) * s).floor().max(0.0);
    let right = ((f64::from(rect.x) + f64::from(rect.width)) * s)
        .ceil()
        .min(page(scene.0));
    let bottom = ((f64::from(rect.y) + f64::from(rect.height)) * s)
        .ceil()
        .min(page(scene.1));
    if left >= right || top >= bottom {
        return None;
    }
    // Both corners are within the coordinate limit, so the grid coordinates fit an i32.
    let origin = DeviceOrigin {
        x: left as i32,
        y: top as i32,
    };
    Some((origin, (right - left, bottom - top)))
}

/// The image of the items of an [`Item::Raster`] covering `rect`, on a scene of `scene` (width
/// and height) points, at `dpi`, for a render whose errors have `context`. `depth` is the number
/// of groups around the items, the item itself counted as one, as the PNG backend counts it.
/// `Ok(None)` when there is nothing to draw: `rect` cannot be drawn or leaves nothing of the
/// scene, or no pixel of the image is visible.
///
/// # Errors
///
/// `Error::Style` under `context`, with the parameter `raster_dpi`, when the image would have
/// more than 32,767 pixels on a side or 2^26 pixels in all, or cannot be allocated.
pub(super) fn raster_image(
    rect: &Rect,
    items: &[Item],
    scene: (f32, f32),
    dpi: u32,
    context: &'static str,
    depth: usize,
) -> Result<Option<RasterImage>> {
    let Some((origin, (width, height))) = pixel_region(rect, scene, dpi) else {
        return Ok(None);
    };
    let too_large = |message: String| {
        Error::style(context, message)
            .with_parameter("raster_dpi")
            .with_hint("use a lower raster_dpi, a smaller figure, or render=ub.Render.VECTOR.")
    };
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(too_large(format!(
            "a mark drawn as an image would be {width} by {height} pixels at raster_dpi {dpi}, \
             over {MAX_SIDE} on one side."
        )));
    }
    if width * height > MAX_PIXELS {
        return Err(too_large(format!(
            "a mark drawn as an image would have {} pixels at raster_dpi {dpi}, over \
             {MAX_PIXELS}.",
            width * height
        )));
    }
    // Both are whole numbers in 1..=32767 here.
    let size = (width as u32, height as u32);
    let Some(pixmap) = draw_region(items, dpi, origin, size, depth) else {
        return Err(too_large(format!(
            "a mark drawn as an image of {width} by {height} pixels at raster_dpi {dpi} cannot \
             be allocated."
        )));
    };
    let rgba = pixmap.take_demultiplied();
    if rgba.chunks_exact(4).all(|pixel| pixel[3] == 0) {
        return Ok(None);
    }
    let s = f64::from(dpi as f32 / 72.0);
    let (left, top) = (f64::from(origin.x), f64::from(origin.y));
    let rect = Rect {
        x: (left / s) as f32,
        y: (top / s) as f32,
        width: (width / s) as f32,
        height: (height / s) as f32,
    };
    Ok(Some(RasterImage { rect, size, rgba }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;
    use crate::geometry::Path;
    use crate::render::png::render_png;
    use crate::scene::{FillRule, LineCap, LineJoin, Paint, Scene, StrokeStyle};
    use crate::test_support::image::decode_rgba8;

    const CLEAR: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Markers, a stroke and a fill that reach past `rect(10.3, 7.7, 50.0, 30.0)`.
    fn items() -> Vec<Item> {
        let mut shape = Path::new();
        shape.move_to(Point { x: -2.0, y: -2.0 });
        shape.line_to(Point { x: 2.0, y: -2.0 });
        shape.line_to(Point { x: 0.0, y: 2.5 });
        shape.close();
        let mut line = Path::new();
        line.move_to(Point { x: 0.0, y: 40.0 });
        line.line_to(Point { x: 70.0, y: 3.0 });
        let mut square = Path::new();
        square.move_to(Point { x: 20.0, y: 10.0 });
        square.line_to(Point { x: 45.5, y: 10.0 });
        square.line_to(Point { x: 45.5, y: 33.25 });
        square.close();
        let positions = (0..300)
            .map(|i| Point {
                x: 8.0 + (i * 37 % 563) as f32 / 10.0,
                y: 5.0 + (i * 53 % 347) as f32 / 10.0,
            })
            .collect();
        vec![
            Item::Fill {
                path: square,
                paint: Paint::Solid(Rgba {
                    r: 40,
                    g: 90,
                    b: 200,
                    a: 140,
                }),
                rule: FillRule::NonZero,
            },
            Item::Stroke {
                path: line,
                style: StrokeStyle {
                    color: Rgba::opaque(200, 60, 20),
                    width: 1.7,
                    dash: vec![3.0, 1.5],
                    cap: LineCap::Round,
                    join: LineJoin::Round,
                },
            },
            Item::Markers {
                shape,
                fill: Some(Rgba::opaque(20, 140, 60)),
                stroke: Some(StrokeStyle {
                    color: Rgba::opaque(250, 250, 250),
                    width: 0.6,
                    dash: Vec::new(),
                    cap: LineCap::Butt,
                    join: LineJoin::Miter,
                }),
                positions,
            },
        ]
    }

    #[test]
    fn the_image_is_the_png_of_the_items_on_its_pixels() {
        let items = items();
        for dpi in [72, 150, 300, 317] {
            let mut scene = Scene::new(80.0, 50.0, CLEAR);
            scene.items = items.clone();
            let png = decode_rgba8(&render_png(&scene, dpi).unwrap()).unwrap();
            let area = rect(10.3, 7.7, 50.0, 30.0);
            let image = raster_image(&area, &items, (80.0, 50.0), dpi, "test", 1)
                .unwrap()
                .unwrap();
            let s = f64::from(dpi as f32 / 72.0);
            let left = (f64::from(area.x) * s).floor() as u32;
            let top = (f64::from(area.y) * s).floor() as u32;
            let right = ((f64::from(area.x) + f64::from(area.width)) * s).ceil() as u32;
            let bottom = ((f64::from(area.y) + f64::from(area.height)) * s).ceil() as u32;
            assert_eq!(image.size, (right - left, bottom - top), "dpi {dpi}");
            for row in 0..image.size.1 {
                for column in 0..image.size.0 {
                    let at = ((row * image.size.0 + column) * 4) as usize;
                    assert_eq!(
                        image.rgba[at..at + 4],
                        png.at(left + column, top + row),
                        "dpi {dpi}, pixel {column}, {row}"
                    );
                }
            }
            // The rectangle in points is the pixels' own.
            assert!((f64::from(image.rect.x) * s - f64::from(left)).abs() < 1e-3);
            assert!((f64::from(image.rect.width) * s - f64::from(right - left)).abs() < 1e-3);
        }
    }

    #[test]
    fn the_image_is_cut_to_the_scene() {
        let image = raster_image(
            &rect(-10.0, 40.0, 200.0, 30.0),
            &items(),
            (80.0, 50.0),
            72,
            "t",
            1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(image.size, (80, 10));
        assert_eq!(image.rect, rect(0.0, 40.0, 80.0, 10.0));
    }

    #[test]
    fn nothing_to_draw_is_no_image() {
        let scene = (80.0, 50.0);
        for area in [
            rect(f32::NAN, 0.0, 10.0, 10.0),
            rect(0.0, 0.0, 0.0, 10.0),
            rect(2.0e6, 0.0, 10.0, 10.0),
            rect(100.0, 0.0, 10.0, 10.0),
        ] {
            assert_eq!(
                raster_image(&area, &items(), scene, 300, "t", 1).unwrap(),
                None
            );
        }
        // Nothing visible inside the rectangle.
        assert_eq!(
            raster_image(&rect(70.0, 45.0, 5.0, 4.0), &items(), scene, 300, "t", 1).unwrap(),
            None
        );
        assert_eq!(
            raster_image(&rect(0.0, 0.0, 80.0, 50.0), &[], scene, 300, "t", 1).unwrap(),
            None
        );
    }

    #[test]
    fn images_beyond_the_png_limits_are_refused() {
        // 1,000 pt at 2,400 dpi is 33,334 pixels.
        let wide = rect(0.0, 0.0, 1000.0, 10.0);
        let err =
            raster_image(&wide, &items(), (1000.0, 10.0), 2400, "Figure.to_svg()", 1).unwrap_err();
        assert_eq!(err.parameter(), Some("raster_dpi"));
        assert_eq!(
            err.to_string(),
            "Figure.to_svg(): a mark drawn as an image would be 33334 by 334 pixels at \
             raster_dpi 2400, over 32767 on one side."
        );
        let square = rect(0.0, 0.0, 540.0, 540.0);
        let err = raster_image(
            &square,
            &items(),
            (540.0, 540.0),
            1200,
            "Figure.to_pdf()",
            1,
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("at raster_dpi 1200, over 67108864."),
            "{err}"
        );
        assert_eq!(
            err.hint(),
            Some("use a lower raster_dpi, a smaller figure, or render=ub.Render.VECTOR.")
        );
    }
}
