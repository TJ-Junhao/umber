//! The PNG backend: a [`Scene`] rasterized with `tiny-skia`.

use tiny_skia as sk;

use crate::color::Rgba;
use crate::error::{Error, Result};
use crate::geometry::{Path, PathVerb, Point};
use crate::scene::{
    FillRule, Item, LineCap, LineJoin, LinearGradient, Paint, Rect, Scene, StrokeStyle,
};

const CONTEXT: &str = "Figure.to_png()";
const MIN_DPI: u32 = 10;
const MAX_DPI: u32 = 2400;
const MAX_SIDE: f64 = 32_767.0;
const MAX_PIXELS: f64 = 268_435_456.0;

/// Renders `scene` to PNG bytes at `dpi` dots per inch.
///
/// The image is `round(width / 72 * dpi)` by `round(height / 72 * dpi)` pixels, at least 1 by 1,
/// filled with the scene background. The file records the resolution in a `pHYs` chunk. Values
/// the rasterizer cannot draw (non-finite coordinates, zero-width strokes, degenerate clips) are
/// skipped; the output is always a valid PNG. The same scene gives the same bytes.
///
/// # Errors
///
/// Returns [`Error::Style`] when `dpi` is outside `10..=2400`, when the scene size is not
/// finite, or when the image would exceed 32,767 pixels per side or 2^28 pixels in total.
///
/// # Examples
///
/// ```
/// use umber_core::render::render_png;
/// use umber_core::scene::Scene;
/// use umber_core::Rgba;
///
/// let scene = Scene::new(72.0, 36.0, Rgba { r: 255, g: 255, b: 255, a: 255 });
/// let bytes = render_png(&scene, 144)?;
/// assert_eq!(&bytes[1..4], b"PNG");
/// # Ok::<(), umber_core::Error>(())
/// ```
pub fn render_png(scene: &Scene, dpi: u32) -> Result<Vec<u8>> {
    let (width, height) = pixel_size(scene, dpi)?;
    let Some(mut pixmap) = sk::Pixmap::new(width, height) else {
        return Err(too_large("the image cannot be allocated"));
    };
    pixmap.fill(color(scene.background));
    let scale = dpi as f32 / 72.0;
    let root = sk::Transform::from_scale(scale, scale);
    draw_items(&mut pixmap, &scene.items, root, None);
    encode(pixmap, dpi)
}

fn too_large(what: &str) -> Error {
    Error::Style {
        context: CONTEXT,
        message: format!("{what}; use a smaller figure size or a lower dpi."),
    }
}

fn pixel_size(scene: &Scene, dpi: u32) -> Result<(u32, u32)> {
    if !(MIN_DPI..=MAX_DPI).contains(&dpi) {
        return Err(Error::Style {
            context: CONTEXT,
            message: format!("dpi {dpi} is out of range; use a value from {MIN_DPI} to {MAX_DPI}."),
        });
    }
    if !scene.width.is_finite() || !scene.height.is_finite() {
        return Err(Error::Style {
            context: CONTEXT,
            message: "the figure size is not finite; use a finite width and height.".into(),
        });
    }
    let px = |size: f32| (f64::from(size) / 72.0 * f64::from(dpi)).round().max(1.0);
    let (w, h) = (px(scene.width), px(scene.height));
    if w > MAX_SIDE || h > MAX_SIDE {
        return Err(too_large(&format!(
            "the image would be {w} by {h} pixels, over {MAX_SIDE} on one side"
        )));
    }
    if w * h > MAX_PIXELS {
        return Err(too_large(&format!(
            "the image would have {} pixels, over {MAX_PIXELS}",
            w * h
        )));
    }
    // Both values are integers in 1..=32767 here.
    Ok((w as u32, h as u32))
}

fn color(c: Rgba) -> sk::Color {
    sk::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

fn solid_paint(c: Rgba) -> sk::Paint<'static> {
    let mut paint = sk::Paint::default();
    paint.set_color(color(c));
    paint.anti_alias = true;
    paint
}

/// Converts a path, dropping non-finite segments. `None` means there is nothing to draw.
fn convert_path(path: &Path) -> Option<sk::Path> {
    let mut builder = sk::PathBuilder::new();
    let mut open = false;
    let point = |p: &Point| p.x.is_finite() && p.y.is_finite();
    for verb in path.verbs() {
        match verb {
            PathVerb::MoveTo(p) => {
                if point(p) {
                    builder.move_to(p.x, p.y);
                    open = true;
                } else {
                    open = false;
                }
            }
            PathVerb::LineTo(p) => {
                if point(p) {
                    if open {
                        builder.line_to(p.x, p.y);
                    } else {
                        builder.move_to(p.x, p.y);
                        open = true;
                    }
                }
            }
            PathVerb::QuadTo(c, p) => {
                if point(c) && point(p) {
                    if open {
                        builder.quad_to(c.x, c.y, p.x, p.y);
                    } else {
                        builder.move_to(p.x, p.y);
                        open = true;
                    }
                }
            }
            PathVerb::CubicTo(c1, c2, p) => {
                if point(c1) && point(c2) && point(p) {
                    if open {
                        builder.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y);
                    } else {
                        builder.move_to(p.x, p.y);
                        open = true;
                    }
                }
            }
            PathVerb::Close => {
                if open {
                    builder.close();
                }
            }
        }
    }
    builder.finish()
}

fn fill_rule(rule: FillRule) -> sk::FillRule {
    match rule {
        FillRule::NonZero => sk::FillRule::Winding,
        FillRule::EvenOdd => sk::FillRule::EvenOdd,
    }
}

/// Builds the shader paint for a gradient; `None` means the fill is skipped.
fn gradient_paint(gradient: &LinearGradient) -> Option<sk::Paint<'static>> {
    let mut stops: Vec<(f32, Rgba)> = gradient
        .stops
        .iter()
        .map(|s| {
            let offset = if s.offset.is_nan() {
                0.0
            } else {
                s.offset.clamp(0.0, 1.0)
            };
            (offset, s.color)
        })
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let last = stops.last()?.1;
    let finite = [
        gradient.start.x,
        gradient.start.y,
        gradient.end.x,
        gradient.end.y,
    ]
    .iter()
    .all(|v| v.is_finite());
    if !finite {
        return None;
    }
    if stops.len() == 1 || gradient.start == gradient.end {
        return Some(solid_paint(last));
    }
    let shader = sk::LinearGradient::new(
        sk::Point::from_xy(gradient.start.x, gradient.start.y),
        sk::Point::from_xy(gradient.end.x, gradient.end.y),
        stops
            .iter()
            .map(|(offset, c)| sk::GradientStop::new(*offset, color(*c)))
            .collect(),
        sk::SpreadMode::Pad,
        sk::Transform::identity(),
    );
    match shader {
        Some(shader) => Some(sk::Paint {
            shader,
            anti_alias: true,
            ..sk::Paint::default()
        }),
        None => Some(solid_paint(last)),
    }
}

fn stroke_of(style: &StrokeStyle) -> Option<sk::Stroke> {
    if !style.width.is_finite() || style.width <= 0.0 {
        return None;
    }
    let dash = if !style.dash.is_empty() && style.dash.iter().all(|d| d.is_finite() && *d >= 0.0) {
        sk::StrokeDash::new(style.dash.clone(), 0.0)
    } else {
        None
    };
    Some(sk::Stroke {
        width: style.width,
        line_cap: match style.cap {
            LineCap::Butt => sk::LineCap::Butt,
            LineCap::Round => sk::LineCap::Round,
            LineCap::Square => sk::LineCap::Square,
        },
        line_join: match style.join {
            LineJoin::Miter => sk::LineJoin::Miter,
            LineJoin::Round => sk::LineJoin::Round,
            LineJoin::Bevel => sk::LineJoin::Bevel,
        },
        dash,
        ..sk::Stroke::default()
    })
}

fn stroke_path(
    pixmap: &mut sk::Pixmap,
    path: &sk::Path,
    style: &StrokeStyle,
    transform: sk::Transform,
    mask: Option<&sk::Mask>,
) {
    if let Some(stroke) = stroke_of(style) {
        pixmap.stroke_path(path, &solid_paint(style.color), &stroke, transform, mask);
    }
}

/// The clip rectangle as a device-space mask intersected with `parent`; `None` omits the group.
fn clip_mask(
    pixmap: &sk::Pixmap,
    clip: &Rect,
    transform: sk::Transform,
    parent: Option<&sk::Mask>,
) -> Option<sk::Mask> {
    let rect = sk::NonZeroRect::from_xywh(clip.x, clip.y, clip.width, clip.height)?;
    let path = sk::PathBuilder::from_rect(rect.to_rect());
    match parent {
        Some(parent) => {
            let mut mask = parent.clone();
            mask.intersect_path(&path, sk::FillRule::Winding, true, transform);
            Some(mask)
        }
        None => {
            let mut mask = sk::Mask::new(pixmap.width(), pixmap.height())?;
            mask.fill_path(&path, sk::FillRule::Winding, true, transform);
            Some(mask)
        }
    }
}

fn draw_items(
    pixmap: &mut sk::Pixmap,
    items: &[Item],
    transform: sk::Transform,
    mask: Option<&sk::Mask>,
) {
    for item in items {
        match item {
            Item::Fill { path, paint, rule } => {
                let Some(path) = convert_path(path) else {
                    continue;
                };
                let paint = match paint {
                    Paint::Solid(c) => Some(solid_paint(*c)),
                    Paint::Linear(gradient) => gradient_paint(gradient),
                };
                if let Some(paint) = paint {
                    pixmap.fill_path(&path, &paint, fill_rule(*rule), transform, mask);
                }
            }
            Item::Stroke { path, style } => {
                if let Some(path) = convert_path(path) {
                    stroke_path(pixmap, &path, style, transform, mask);
                }
            }
            Item::Markers {
                shape,
                fill,
                stroke,
                positions,
            } => {
                let Some(shape) = convert_path(shape) else {
                    continue;
                };
                for position in positions {
                    if !position.x.is_finite() || !position.y.is_finite() {
                        continue;
                    }
                    let at = transform.pre_translate(position.x, position.y);
                    if let Some(fill) = fill {
                        pixmap.fill_path(
                            &shape,
                            &solid_paint(*fill),
                            sk::FillRule::Winding,
                            at,
                            mask,
                        );
                    }
                    if let Some(style) = stroke {
                        stroke_path(pixmap, &shape, style, at, mask);
                    }
                }
            }
            Item::Text(text) => {
                if let Some(path) = convert_path(&text.outline) {
                    pixmap.fill_path(
                        &path,
                        &solid_paint(text.color),
                        sk::FillRule::Winding,
                        transform,
                        mask,
                    );
                }
            }
            Item::Group { clip, items } => match clip {
                None => draw_items(pixmap, items, transform, mask),
                Some(clip) => {
                    if let Some(inner) = clip_mask(pixmap, clip, transform, mask) {
                        draw_items(pixmap, items, transform, Some(&inner));
                    }
                }
            },
        }
    }
}

fn encode(pixmap: sk::Pixmap, dpi: u32) -> Result<Vec<u8>> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let data = pixmap.take_demultiplied();
    let fail = |e: ::png::EncodingError| Error::Style {
        context: CONTEXT,
        message: format!("the PNG could not be encoded ({e}); try a smaller figure."),
    };
    let mut out = Vec::new();
    let mut encoder = ::png::Encoder::new(&mut out, width, height);
    encoder.set_color(::png::ColorType::Rgba);
    encoder.set_depth(::png::BitDepth::Eight);
    encoder.set_compression(::png::Compression::default());
    let per_metre = (f64::from(dpi) / 0.0254).round() as u32;
    encoder.set_pixel_dims(Some(::png::PixelDimensions {
        xppu: per_metre,
        yppu: per_metre,
        unit: ::png::Unit::Meter,
    }));
    let mut writer = encoder.write_header().map_err(fail)?;
    writer.write_image_data(&data).map_err(fail)?;
    writer.finish().map_err(fail)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{Font, Weight};
    use crate::scene::{GradientStop, LinearGradient};
    use crate::text::{FontLibrary, TextStyle};

    const WHITE: Rgba = Rgba {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    const BLACK: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    struct Decoded {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        phys: Option<::png::PixelDimensions>,
    }

    impl Decoded {
        fn at(&self, x: u32, y: u32) -> [u8; 4] {
            let i = ((y * self.width + x) * 4) as usize;
            [
                self.rgba[i],
                self.rgba[i + 1],
                self.rgba[i + 2],
                self.rgba[i + 3],
            ]
        }

        fn is_ink(&self, x: u32, y: u32) -> bool {
            self.at(x, y)[0] < 128
        }
    }

    fn decode(bytes: &[u8]) -> Decoded {
        let mut reader = ::png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!(info.color_type, ::png::ColorType::Rgba);
        assert_eq!(info.bit_depth, ::png::BitDepth::Eight);
        buf.truncate(info.buffer_size());
        Decoded {
            width: info.width,
            height: info.height,
            rgba: buf,
            phys: reader.info().pixel_dims,
        }
    }

    fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
        let mut p = Path::new();
        p.move_to(Point { x, y });
        p.line_to(Point { x: x + w, y });
        p.line_to(Point { x: x + w, y: y + h });
        p.line_to(Point { x, y: y + h });
        p.close();
        p
    }

    fn line_path(x0: f32, x1: f32, y: f32) -> Path {
        let mut p = Path::new();
        p.move_to(Point { x: x0, y });
        p.line_to(Point { x: x1, y });
        p
    }

    fn fill(path: Path, color: Rgba) -> Item {
        Item::Fill {
            path,
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        }
    }

    fn stroke_style(width: f32, dash: Vec<f32>) -> StrokeStyle {
        StrokeStyle {
            color: BLACK,
            width,
            dash,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }
    }

    #[test]
    fn pixel_size_follows_dpi() {
        let scene = Scene::new(72.0, 36.0, WHITE);
        let image = decode(&render_png(&scene, 144).unwrap());
        assert_eq!((image.width, image.height), (144, 72));
    }

    #[test]
    fn phys_chunk_records_dpi() {
        let scene = Scene::new(72.0, 36.0, WHITE);
        let image = decode(&render_png(&scene, 200).unwrap());
        let phys = image.phys.unwrap();
        assert_eq!((phys.xppu, phys.yppu), (7874, 7874));
        assert_eq!(phys.unit, ::png::Unit::Meter);
    }

    #[test]
    fn opaque_square_covers_exact_pixels() {
        let mut scene = Scene::new(30.0, 30.0, WHITE);
        scene.push(fill(rect_path(5.0, 5.0, 10.0, 10.0), BLACK));
        let image = decode(&render_png(&scene, 72).unwrap());
        assert_eq!(image.at(10, 10), [0, 0, 0, 255]);
        assert_eq!(image.at(2, 2), [255, 255, 255, 255]);
        assert_eq!(image.at(15, 15), [255, 255, 255, 255]);
        assert_eq!(image.at(14, 14), [0, 0, 0, 255]);
    }

    #[test]
    fn alpha_blends_over_background() {
        let mut scene = Scene::new(20.0, 20.0, WHITE);
        let half = Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 128,
        };
        scene.push(fill(rect_path(0.0, 0.0, 20.0, 20.0), half));
        let image = decode(&render_png(&scene, 72).unwrap());
        let value = image.at(10, 10)[0];
        assert!((127..=129).contains(&value), "got {value}");
        assert_eq!(image.at(10, 10)[3], 255);
    }

    #[test]
    fn gradient_endpoints_have_stop_colors() {
        let red = Rgba {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        let blue = Rgba {
            r: 0,
            g: 0,
            b: 255,
            a: 255,
        };
        let mut scene = Scene::new(100.0, 20.0, WHITE);
        scene.push(Item::Fill {
            path: rect_path(0.0, 0.0, 100.0, 20.0),
            paint: Paint::Linear(LinearGradient {
                start: Point { x: 0.0, y: 0.0 },
                end: Point { x: 100.0, y: 0.0 },
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: red,
                    },
                    GradientStop {
                        offset: 1.0,
                        color: blue,
                    },
                ],
            }),
            rule: FillRule::NonZero,
        });
        let image = decode(&render_png(&scene, 72).unwrap());
        let left = image.at(0, 10);
        let right = image.at(99, 10);
        assert!(left[0] >= 247 && left[2] <= 8, "left {left:?}");
        assert!(right[2] >= 247 && right[0] <= 8, "right {right:?}");
    }

    #[test]
    fn clip_hides_outside_content() {
        let mut scene = Scene::new(40.0, 20.0, WHITE);
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            }),
            items: vec![fill(rect_path(0.0, 0.0, 40.0, 20.0), BLACK)],
        });
        let image = decode(&render_png(&scene, 72).unwrap());
        assert_eq!(image.at(10, 10), [0, 0, 0, 255]);
        assert_eq!(image.at(30, 10), [255, 255, 255, 255]);
    }

    #[test]
    fn nested_clips_intersect() {
        let mut scene = Scene::new(40.0, 20.0, WHITE);
        let inner = Item::Group {
            clip: Some(Rect {
                x: 10.0,
                y: 0.0,
                width: 30.0,
                height: 20.0,
            }),
            items: vec![fill(rect_path(0.0, 0.0, 40.0, 20.0), BLACK)],
        };
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            }),
            items: vec![inner],
        });
        let image = decode(&render_png(&scene, 72).unwrap());
        assert_eq!(image.at(5, 10), [255, 255, 255, 255]);
        assert_eq!(image.at(15, 10), [0, 0, 0, 255]);
        assert_eq!(image.at(30, 10), [255, 255, 255, 255]);
    }

    #[test]
    fn dashed_stroke_has_gaps() {
        let mut scene = Scene::new(60.0, 10.0, WHITE);
        scene.push(Item::Stroke {
            path: line_path(0.0, 60.0, 5.0),
            style: stroke_style(2.0, vec![6.0, 4.0]),
        });
        let image = decode(&render_png(&scene, 72).unwrap());
        let row: Vec<bool> = (0..60).map(|x| image.is_ink(x, 5)).collect();
        assert!(row.iter().any(|ink| *ink));
        assert!(row.iter().any(|ink| !*ink));
        let solid = {
            let mut s = Scene::new(60.0, 10.0, WHITE);
            s.push(Item::Stroke {
                path: line_path(0.0, 60.0, 5.0),
                style: stroke_style(2.0, vec![]),
            });
            decode(&render_png(&s, 72).unwrap())
        };
        assert!((0..60).all(|x| solid.is_ink(x, 5)));
    }

    #[test]
    fn markers_draw_at_every_position() {
        let mut scene = Scene::new(60.0, 20.0, WHITE);
        scene.push(Item::Markers {
            shape: rect_path(-3.0, -3.0, 6.0, 6.0),
            fill: Some(BLACK),
            stroke: None,
            positions: vec![
                Point { x: 10.0, y: 10.0 },
                Point { x: 30.0, y: 10.0 },
                Point { x: 50.0, y: 10.0 },
            ],
        });
        let image = decode(&render_png(&scene, 72).unwrap());
        let mut blobs = 0;
        let mut previous = false;
        for x in 0..60 {
            let ink = image.is_ink(x, 10);
            if ink && !previous {
                blobs += 1;
            }
            previous = ink;
        }
        assert_eq!(blobs, 3);
    }

    #[test]
    fn text_renders_ink() {
        let mut scene = Scene::new(60.0, 40.0, WHITE);
        let style = TextStyle {
            font: Font::Inter,
            weight: Weight::Regular,
            size: 24.0,
            tabular_figures: false,
        };
        scene.push_text(
            FontLibrary::global(),
            "Hg",
            style,
            Point { x: 4.0, y: 24.0 },
            BLACK,
        );
        let image = decode(&render_png(&scene, 72).unwrap());
        let ink_in_rows = |mut rows: std::ops::Range<u32>| {
            rows.any(|y| (0..image.width).any(|x| image.is_ink(x, y)))
        };
        assert!(ink_in_rows(4..24), "no ink above the baseline");
        assert!(ink_in_rows(25..34), "no ink below the baseline");
    }

    #[test]
    fn png_rejects_oversized_output() {
        let check = |width: f32, height: f32, dpi: u32| {
            let scene = Scene::new(width, height, WHITE);
            match render_png(&scene, dpi) {
                Err(Error::Style { context, .. }) => assert_eq!(context, "Figure.to_png()"),
                other => panic!("expected a style error, got {:?}", other.map(|b| b.len())),
            }
        };
        check(100.0, 100.0, 9);
        check(100.0, 100.0, 2401);
        check(100_000.0, 10.0, 72);
        check(6_000.0, 6_000.0, 300);
        check(f32::NAN, 10.0, 72);
        check(10.0, f32::INFINITY, 72);
    }

    fn hostile_scene() -> Scene {
        let nan = f32::NAN;
        let inf = f32::INFINITY;
        let pt = |x: f32, y: f32| Point { x, y };
        let mut bad = Path::new();
        bad.move_to(pt(nan, 0.0));
        bad.line_to(pt(1.0, inf));
        bad.quad_to(pt(nan, nan), pt(2.0, 2.0));
        bad.cubic_to(pt(0.0, 0.0), pt(inf, 0.0), pt(3.0, 3.0));
        bad.line_to(pt(5.0, 5.0));
        bad.close();
        let mut huge = Path::new();
        huge.move_to(pt(-3.0e38, -3.0e38));
        huge.line_to(pt(3.0e38, 3.0e38));
        huge.line_to(pt(3.0e38, -3.0e38));
        let gradient = |stops: Vec<GradientStop>, end: Point| {
            Paint::Linear(LinearGradient {
                start: pt(0.0, 0.0),
                end,
                stops,
            })
        };
        let stop = |offset: f32| GradientStop {
            offset,
            color: BLACK,
        };
        let mut scene = Scene::new(50.0, 50.0, WHITE);
        let square = || rect_path(0.0, 0.0, 10.0, 10.0);
        scene.push(fill(bad.clone(), BLACK));
        scene.push(fill(huge.clone(), BLACK));
        scene.push(fill(Path::new(), BLACK));
        scene.push(Item::Stroke {
            path: bad.clone(),
            style: stroke_style(1.0, vec![]),
        });
        scene.push(Item::Stroke {
            path: huge,
            style: stroke_style(1.0, vec![2.0, 2.0]),
        });
        for width in [0.0, -1.0, nan, inf] {
            scene.push(Item::Stroke {
                path: square(),
                style: stroke_style(width, vec![]),
            });
        }
        for dash in [
            vec![0.0, 0.0],
            vec![-1.0, 2.0],
            vec![nan],
            vec![inf, 1.0],
            vec![3.0],
        ] {
            scene.push(Item::Stroke {
                path: square(),
                style: stroke_style(1.0, dash),
            });
        }
        for paint in [
            gradient(vec![], pt(10.0, 0.0)),
            gradient(vec![stop(0.5)], pt(10.0, 0.0)),
            gradient(vec![stop(0.0), stop(1.0)], pt(0.0, 0.0)),
            gradient(vec![stop(nan), stop(5.0), stop(-2.0)], pt(nan, 0.0)),
            gradient(vec![stop(nan), stop(5.0), stop(-2.0)], pt(10.0, 0.0)),
        ] {
            scene.push(Item::Fill {
                path: square(),
                paint,
                rule: FillRule::EvenOdd,
            });
        }
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: -1.0,
                height: 10.0,
            }),
            items: vec![fill(square(), BLACK)],
        });
        scene.push(Item::Group {
            clip: Some(Rect {
                x: nan,
                y: 0.0,
                width: 10.0,
                height: inf,
            }),
            items: vec![fill(square(), BLACK)],
        });
        scene.push(Item::Group {
            clip: None,
            items: vec![],
        });
        scene.push(Item::Markers {
            shape: square(),
            fill: Some(BLACK),
            stroke: Some(stroke_style(nan, vec![])),
            positions: vec![pt(nan, 1.0), pt(1.0, inf), pt(20.0, 20.0)],
        });
        scene.push(Item::Markers {
            shape: bad,
            fill: Some(BLACK),
            stroke: None,
            positions: vec![pt(5.0, 5.0)],
        });
        scene
    }

    #[test]
    fn hostile_scenes_render_without_panicking() {
        let image = decode(&render_png(&hostile_scene(), 72).unwrap());
        assert_eq!((image.width, image.height), (50, 50));
        // A scene with a zero size still renders a 1 x 1 image.
        let tiny = decode(&render_png(&Scene::new(0.0, -5.0, WHITE), 72).unwrap());
        assert_eq!((tiny.width, tiny.height), (1, 1));
    }

    #[test]
    fn png_output_is_deterministic() {
        let scene = hostile_scene();
        let first = render_png(&scene, 150).unwrap();
        for _ in 0..5 {
            assert_eq!(render_png(&scene, 150).unwrap(), first);
        }
    }
}
