//! The PNG backend: a [`Scene`] rasterized with `tiny-skia`.
//!
//! Every path, dash and gradient goes through [`normalize`](super::normalize) first, so the
//! backend draws exactly what the SVG backend draws. If `tiny-skia` still refuses a shape the
//! shared layer accepted, the item is skipped.

use std::ops::Range;

use tiny_skia as sk;

use super::normalize::{
    GradientKind, NormalizedPath, Segment, clip_is_valid, image_is_drawable, normalize_dash,
    normalize_gradient, normalize_path, stroke_is_visible, text_is_visible, valid_point,
};
use crate::color::Rgba;
use crate::error::{Error, Result};
use crate::geometry::Point;
use crate::scene::{
    FillRule, Item, LineCap, LineJoin, MAX_GROUP_DEPTH, Paint, Rect, Scene, StrokeStyle,
};

const CONTEXT: &str = "Figure.to_png()";
const MIN_DPI: u32 = 10;
const MAX_DPI: u32 = 2400;
const MAX_SIDE: f64 = 32_767.0;
const MAX_PIXELS: f64 = 67_108_864.0; // 2^26
/// A marker sprite larger than this, in pixels per side, is not cached.
const MAX_SPRITE_SIDE: i32 = 256;
/// Sub-pixel phases per axis for marker sprites (8 x 8 = 64 sprites at most per item).
const PHASES: usize = 8;

/// Renders `scene` to PNG bytes at `dpi` dots per inch.
///
/// The image is `round(width / 72 * dpi)` by `round(height / 72 * dpi)` pixels, at least 1 by 1,
/// filled with the scene background. The file records the resolution in a `pHYs` chunk. Values
/// the shared normalization rejects (non-finite or huge coordinates, zero-width strokes,
/// degenerate clips and gradients) are skipped; the output is always a valid PNG. The same scene
/// gives the same bytes.
///
/// Scenes with thousands of markers are composited on up to 8 threads, bounded by
/// [`std::thread::available_parallelism`]; the threads end before this returns, and the bytes
/// do not depend on how many there were.
///
/// # Errors
///
/// Returns [`Error::Style`] when `dpi` is outside `10..=2400`, when the scene size is not
/// finite, or when the image would exceed 32,767 pixels per side or 2^26 (67,108,864) pixels
/// in total.
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
    let pixmap = rasterize(scene, dpi)?;
    encode(pixmap, dpi)
}

/// Device pixels per scene point, `dpi / 72`: the scale from scene coordinates to the pixmap.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DeviceScale(f32);

impl DeviceScale {
    fn new(dpi: u32) -> DeviceScale {
        DeviceScale(dpi as f32 / 72.0)
    }

    /// Device pixels per point.
    fn get(self) -> f32 {
        self.0
    }

    /// The scene coordinate `points` in device pixels.
    fn to_device(self, points: f32) -> DevicePixels {
        DevicePixels(points * self.0)
    }
}

/// A coordinate in device pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DevicePixels(f32);

fn rasterize(scene: &Scene, dpi: u32) -> Result<sk::Pixmap> {
    let (width, height) = pixel_size(scene, dpi)?;
    let Some(mut pixmap) = sk::Pixmap::new(width, height) else {
        return Err(too_large("the image cannot be allocated"));
    };
    pixmap.fill(color(scene.background));
    let scale = DeviceScale::new(dpi);
    let mut canvas = Canvas {
        pixmap,
        scale,
        root: sk::Transform::from_scale(scale.get(), scale.get()),
        mask: None,
    };
    canvas.items(&scene.items, None, 0);
    Ok(canvas.pixmap)
}

fn too_large(what: &str) -> Error {
    Error::style(
        CONTEXT,
        format!("{what}; use a smaller figure size or a lower dpi."),
    )
}

/// `dpi` as the `u32` the renderer works in, or `Error::Style` under `context` when it is
/// outside `10..=2400`. The message names the value as given, however large or negative.
pub(crate) fn check_dpi(context: &'static str, dpi: i64) -> Result<u32> {
    match u32::try_from(dpi) {
        Ok(valid) if (MIN_DPI..=MAX_DPI).contains(&valid) => Ok(valid),
        _ => Err(Error::style(
            context,
            format!("dpi {dpi} is out of range; use a value from {MIN_DPI} to {MAX_DPI}."),
        )
        .with_parameter("dpi")),
    }
}

fn pixel_size(scene: &Scene, dpi: u32) -> Result<(u32, u32)> {
    check_dpi(CONTEXT, i64::from(dpi))?;
    if !scene.width.is_finite() || !scene.height.is_finite() {
        return Err(Error::style(
            CONTEXT,
            "the figure size is not finite; use a finite width and height.",
        ));
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

// Counts items skipped because `tiny-skia` refused a shape the shared layer accepted.
#[cfg(test)]
thread_local! {
    static REFUSED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn refused() {
    #[cfg(test)]
    REFUSED.with(|c| c.set(c.get() + 1));
}

// Lets tests render markers path by path instead of through sprites.
#[cfg(test)]
thread_local! {
    static FORCE_PATHS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn sprites_disabled() -> bool {
    #[cfg(test)]
    return FORCE_PATHS.with(std::cell::Cell::get);
    #[cfg(not(test))]
    false
}

/// Converts normalized geometry to a `tiny-skia` path. `None` means there is nothing to draw.
fn convert_path(path: &NormalizedPath) -> Option<sk::Path> {
    let mut builder = sk::PathBuilder::new();
    for sub in path.subpaths() {
        let start = sub.start();
        builder.move_to(start.x, start.y);
        for seg in sub.segments() {
            match *seg {
                Segment::Line(p) => builder.line_to(p.x, p.y),
                Segment::Quad(c, p) => builder.quad_to(c.x, c.y, p.x, p.y),
                Segment::Cubic(c1, c2, p) => builder.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y),
            }
        }
        if sub.is_closed() {
            builder.close();
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

/// Builds the paint for a fill; `None` means the fill is skipped.
fn fill_paint(gradient: GradientKind) -> Option<sk::Paint<'static>> {
    match gradient {
        GradientKind::Skip => None,
        GradientKind::Solid(c) => (c.a > 0).then(|| solid_paint(c)),
        GradientKind::Linear { start, end, stops } => {
            let shader = sk::LinearGradient::new(
                sk::Point::from_xy(start.x, start.y),
                sk::Point::from_xy(end.x, end.y),
                stops
                    .iter()
                    .map(|s| sk::GradientStop::new(s.offset, color(s.color)))
                    .collect(),
                sk::SpreadMode::Pad,
                sk::Transform::identity(),
            );
            if shader.is_none() {
                refused();
            }
            shader.map(|shader| sk::Paint {
                shader,
                anti_alias: true,
                ..sk::Paint::default()
            })
        }
    }
}

/// A stroke ready to draw: the (dashed) path in scene space, the stroke without a dash and the
/// paint.
struct PreparedStroke {
    path: sk::Path,
    stroke: sk::Stroke,
    paint: sk::Paint<'static>,
    /// The widest the stroke can reach from the path, in points.
    reach: f32,
}

/// Prepares a stroke; `None` means the stroke is skipped.
fn prepare_stroke(
    normalized: &NormalizedPath,
    path: &sk::Path,
    style: &StrokeStyle,
    root: sk::Transform,
) -> Option<PreparedStroke> {
    if !stroke_is_visible(style) {
        return None;
    }
    let mut path = path.clone();
    if let Some(array) = normalize_dash(&style.dash, normalized.control_length()) {
        let Some(dash) = sk::StrokeDash::new(array, 0.0) else {
            refused();
            return None;
        };
        let scale = sk::PathStroker::compute_resolution_scale(&root);
        // `None` here means the dashes add up to nothing (a zero-length path, or only
        // zero-length dashes): there is nothing to draw, which is not a refusal. The dash
        // count budget cannot be hit because `normalize_dash` caps it far below tiny-skia's.
        let dashed = path.dash(&dash, scale)?;
        path = dashed;
    }
    let stroke = sk::Stroke {
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
        dash: None,
        ..sk::Stroke::default()
    };
    let half = style.width / 2.0;
    let reach = match style.join {
        LineJoin::Miter => half * stroke.miter_limit.max(core::f32::consts::SQRT_2),
        _ => half * core::f32::consts::SQRT_2,
    };
    Some(PreparedStroke {
        path,
        stroke,
        paint: solid_paint(style.color),
        reach,
    })
}

/// A clip rectangle in device pixels, already intersected with the pixmap bounds, with
/// `left < right` and `top < bottom`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ClipRect {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl ClipRect {
    /// Converts a scene rectangle to device pixels and intersects it with the pixmap bounds
    /// `(0, 0, width, height)`. `None` when the rectangle is invalid (the SVG rule: finite,
    /// positive size) or the intersection is empty.
    fn from_rect(r: &Rect, scale: DeviceScale, width: u32, height: u32) -> Option<ClipRect> {
        if !clip_is_valid(r) {
            return None;
        }
        let s = f64::from(scale.get());
        let (x, y) = (f64::from(r.x), f64::from(r.y));
        let (right, bottom) = (x + f64::from(r.width), y + f64::from(r.height));
        let (w, h) = (f64::from(width), f64::from(height));
        let out = ClipRect {
            left: (x * s).clamp(0.0, w) as f32,
            top: (y * s).clamp(0.0, h) as f32,
            right: (right * s).clamp(0.0, w) as f32,
            bottom: (bottom * s).clamp(0.0, h) as f32,
        };
        (out.left < out.right && out.top < out.bottom).then_some(out)
    }

    /// The geometric intersection; `None` when it is empty.
    fn intersect(self, other: ClipRect) -> Option<ClipRect> {
        let out = ClipRect {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (out.left < out.right && out.top < out.bottom).then_some(out)
    }

    /// The whole pixels `(left, top, right, bottom)` (right and bottom exclusive) that are
    /// certainly fully covered by the mask: the rectangle shrunk by one pixel on every side that
    /// is not the pixmap edge. Sides on the pixmap edge are unbounded.
    fn interior(self, width: u32, height: u32) -> (i32, i32, i32, i32) {
        let lo = |v: f32| {
            if v <= 0.0 {
                i32::MIN
            } else {
                v.ceil() as i32 + 1
            }
        };
        let hi = |v: f32, max: u32| {
            if v >= max as f32 {
                i32::MAX
            } else {
                v.floor() as i32 - 1
            }
        };
        (
            lo(self.left),
            lo(self.top),
            hi(self.right, width),
            hi(self.bottom, height),
        )
    }
}

/// Samples an opaque color gradient at device-pixel centers with `Cmap::at`. The temporary
/// texture covers only the visible path bounds; the normal fill path supplies antialiasing
/// and the current clip. Other gradients retain the common linear paint path.
fn sampled_fill(
    pixmap: &mut sk::Pixmap,
    path: &sk::Path,
    cmap: &crate::Cmap,
    endpoints: (Point, Point),
    rule: sk::FillRule,
    scale: DeviceScale,
    mask: Option<&sk::Mask>,
) {
    let gradient = normalize_gradient(&crate::LinearGradient::sampled(
        cmap,
        endpoints.0,
        endpoints.1,
    ));
    let (start, end) = match gradient {
        GradientKind::Linear { start, end, .. } => (start, end),
        other => {
            if let Some(paint) = fill_paint(other) {
                pixmap.fill_path(
                    path,
                    &paint,
                    rule,
                    sk::Transform::from_scale(scale.get(), scale.get()),
                    mask,
                );
            }
            return;
        }
    };
    let Some(path) = path
        .clone()
        .transform(sk::Transform::from_scale(scale.get(), scale.get()))
    else {
        refused();
        return;
    };
    let bounds = path.bounds();
    let left = bounds.left().floor().max(0.0) as u32;
    let top = bounds.top().floor().max(0.0) as u32;
    let right = bounds.right().ceil().min(pixmap.width() as f32).max(0.0) as u32;
    let bottom = bounds.bottom().ceil().min(pixmap.height() as f32).max(0.0) as u32;
    if left >= right || top >= bottom {
        return;
    }
    let Some(mut texture) = sk::Pixmap::new(right - left, bottom - top) else {
        refused();
        return;
    };
    let (dx, dy) = (f64::from(end.x - start.x), f64::from(end.y - start.y));
    let length2 = dx * dx + dy * dy;
    let factor = f64::from(scale.get());
    let width = texture.width() as usize;
    for (index, pixel) in texture.pixels_mut().iter_mut().enumerate() {
        let x = (f64::from(left) + (index % width) as f64 + 0.5) / factor;
        let y = (f64::from(top) + (index / width) as f64 + 0.5) / factor;
        let t = (((x - f64::from(start.x)) * dx + (y - f64::from(start.y)) * dy) / length2)
            .clamp(0.0, 1.0);
        let color = cmap.at(t);
        if let Some(color) = sk::PremultipliedColorU8::from_rgba(color.r, color.g, color.b, color.a)
        {
            *pixel = color;
        }
    }
    let paint = sk::Paint {
        shader: sk::Pattern::new(
            texture.as_ref(),
            sk::SpreadMode::Pad,
            sk::FilterQuality::Nearest,
            1.0,
            sk::Transform::from_translate(left as f32, top as f32),
        ),
        anti_alias: true,
        ..sk::Paint::default()
    };
    pixmap.fill_path(&path, &paint, rule, sk::Transform::identity(), mask);
}

struct Canvas {
    pixmap: sk::Pixmap,
    scale: DeviceScale,
    root: sk::Transform,
    /// The one clip mask that may exist, with the rectangle it was built from.
    mask: Option<(ClipRect, sk::Mask)>,
}

impl Canvas {
    /// Makes sure the live mask (if any) is the one for `clip`. Returns `false` when the mask
    /// cannot be built, which omits the item.
    fn use_clip(&mut self, clip: Option<ClipRect>) -> bool {
        let Some(clip) = clip else {
            self.mask = None;
            return true;
        };
        if self.mask.as_ref().is_some_and(|(r, _)| *r == clip) {
            return true;
        }
        // Drop the old mask before allocating the new one.
        self.mask = None;
        let Some(rect) = sk::NonZeroRect::from_ltrb(clip.left, clip.top, clip.right, clip.bottom)
        else {
            return false;
        };
        let Some(mut mask) = sk::Mask::new(self.pixmap.width(), self.pixmap.height()) else {
            return false;
        };
        let path = sk::PathBuilder::from_rect(rect.to_rect());
        mask.fill_path(
            &path,
            sk::FillRule::Winding,
            true,
            sk::Transform::identity(),
        );
        self.mask = Some((clip, mask));
        true
    }

    fn items(&mut self, items: &[Item], clip: Option<ClipRect>, depth: usize) {
        for item in items {
            match item {
                Item::Group { clip: own, items } => self.group(own.as_ref(), items, clip, depth),
                leaf => {
                    if self.use_clip(clip) {
                        self.leaf(leaf, clip);
                    }
                }
            }
        }
    }

    fn group(&mut self, own: Option<&Rect>, items: &[Item], clip: Option<ClipRect>, depth: usize) {
        if depth >= MAX_GROUP_DEPTH {
            return;
        }
        let effective = match own {
            None => clip,
            Some(rect) => {
                let Some(rect) = ClipRect::from_rect(
                    rect,
                    self.scale,
                    self.pixmap.width(),
                    self.pixmap.height(),
                ) else {
                    return;
                };
                match clip {
                    None => Some(rect),
                    Some(parent) => match parent.intersect(rect) {
                        Some(both) => Some(both),
                        None => return,
                    },
                }
            }
        };
        self.items(items, effective, depth + 1);
    }

    fn leaf(&mut self, item: &Item, clip: Option<ClipRect>) {
        let Canvas {
            pixmap,
            scale,
            root,
            mask,
            ..
        } = self;
        let mask = if clip.is_some() {
            mask.as_ref().map(|(_, m)| m)
        } else {
            None
        };
        let root = *root;
        match item {
            Item::Fill { path, paint, rule } => {
                let Some(normalized) = normalize_path(path) else {
                    return;
                };
                let Some(path) = convert_path(&normalized) else {
                    return;
                };
                let gradient = match paint {
                    Paint::Solid(c) => GradientKind::Solid(*c),
                    Paint::Linear(g) => normalize_gradient(g),
                    Paint::Sampled { cmap, from, to } => {
                        sampled_fill(
                            pixmap,
                            &path,
                            cmap,
                            (*from, *to),
                            fill_rule(*rule),
                            *scale,
                            mask,
                        );
                        return;
                    }
                };
                if let Some(paint) = fill_paint(gradient) {
                    pixmap.fill_path(&path, &paint, fill_rule(*rule), root, mask);
                }
            }
            Item::Stroke { path, style } => {
                let Some(normalized) = normalize_path(path) else {
                    return;
                };
                let Some(path) = convert_path(&normalized) else {
                    return;
                };
                if let Some(s) = prepare_stroke(&normalized, &path, style, root) {
                    pixmap.stroke_path(&s.path, &s.paint, &s.stroke, root, mask);
                }
            }
            Item::Markers {
                shape,
                fill,
                stroke,
                positions,
            } => draw_markers(
                pixmap,
                *scale,
                root,
                mask.zip(clip),
                shape,
                *fill,
                stroke.as_ref(),
                positions,
            ),
            Item::Text(text) => {
                if !text_is_visible(text.color, text.origin) {
                    return;
                }
                let Some(normalized) = normalize_path(&text.outline) else {
                    return;
                };
                if let Some(path) = convert_path(&normalized) {
                    pixmap.fill_path(
                        &path,
                        &solid_paint(text.color),
                        sk::FillRule::Winding,
                        root,
                        mask,
                    );
                }
            }
            Item::Image {
                rect,
                width,
                height,
                pixels,
            } => draw_image(pixmap, *scale, clip, rect, (*width, *height), pixels),
            Item::Group { .. } => {}
        }
    }
}

/// Draws the image of `width` by `height` `pixels` over `rect`, inside `clip` when there is
/// one: each device pixel the image reaches takes the area-weighted sum of the image pixels it
/// covers inside the clip ([`super::image::resample`]), composited source-over onto the pixmap
/// in integer arithmetic. Every clip of a scene is a rectangle, so the clip enters the area
/// weights exactly and no clip mask is applied on top. Only the device pixels inside both the
/// image and the clip are resampled.
fn draw_image(
    pixmap: &mut sk::Pixmap,
    scale: DeviceScale,
    clip: Option<ClipRect>,
    rect: &Rect,
    (width, height): (u32, u32),
    pixels: &[Rgba],
) {
    if !image_is_drawable(rect, width, height, pixels) {
        return;
    }
    let s = f64::from(scale.get());
    let (left, top) = (f64::from(rect.x) * s, f64::from(rect.y) * s);
    let right = (f64::from(rect.x) + f64::from(rect.width)) * s;
    let bottom = (f64::from(rect.y) + f64::from(rect.height)) * s;
    let (canvas_w, canvas_h) = (f64::from(pixmap.width()), f64::from(pixmap.height()));
    // The visible window: the clip, or the whole pixmap.
    let window = clip.map_or([0.0, 0.0, canvas_w, canvas_h], |c| {
        [c.left, c.top, c.right, c.bottom].map(f64::from)
    });
    let lo_x = left.max(window[0]).floor().max(0.0);
    let lo_y = top.max(window[1]).floor().max(0.0);
    let hi_x = right.min(window[2]).ceil().min(canvas_w);
    let hi_y = bottom.min(window[3]).ceil().min(canvas_h);
    if lo_x >= hi_x || lo_y >= hi_y {
        return;
    }
    // All four are whole numbers within the pixmap here.
    let columns = lo_x as usize..hi_x as usize;
    let rows = lo_y as usize..hi_y as usize;
    let device = super::image::resample(
        [left, top, right, bottom],
        (width as usize, height as usize),
        pixels,
        (columns.clone(), rows.clone()),
        window,
    );
    let stride = pixmap.width() as usize;
    let data = pixmap.data_mut();
    let channel = |v: f64| v.round().clamp(0.0, 255.0) as u32;
    for (row, line) in rows.zip(device.chunks(columns.len())) {
        for (column, &[r, g, b, a]) in columns.clone().zip(line) {
            let a = channel(a);
            if a == 0 {
                continue;
            }
            // A premultiplied channel never exceeds the alpha.
            let source = [channel(r).min(a), channel(g).min(a), channel(b).min(a), a];
            let at = (row * stride + column) * 4;
            let Some(target) = data.get_mut(at..at + 4) else {
                continue;
            };
            let keep = 255 - a;
            for (to, from) in target.iter_mut().zip(source) {
                // Source-over in premultiplied form: at most 255, so it fits a byte.
                *to = (from + div255(u32::from(*to) * keep)) as u8;
            }
        }
    }
}

/// One marker, ready to draw at any position.
struct Marker {
    path: sk::Path,
    fill: Option<sk::Paint<'static>>,
    stroke: Option<PreparedStroke>,
}

impl Marker {
    fn draw(&self, pixmap: &mut sk::Pixmap, at: sk::Transform, mask: Option<&sk::Mask>) {
        if let Some(paint) = &self.fill {
            pixmap.fill_path(&self.path, paint, sk::FillRule::Winding, at, mask);
        }
        if let Some(s) = &self.stroke {
            pixmap.stroke_path(&s.path, &s.paint, &s.stroke, at, mask);
        }
    }
}

/// One marker pre-rendered at one sub-pixel phase.
struct Sprite {
    pixmap: sk::Pixmap,
    /// For each row, the columns `start..end` outside which every pixel is fully transparent
    /// (an empty range for a transparent row).
    spans: Vec<Range<usize>>,
}

impl Sprite {
    fn new(pixmap: sk::Pixmap) -> Sprite {
        let width = pixmap.width() as usize;
        let spans = pixmap
            .data()
            .chunks_exact(width.max(1) * 4)
            .map(|row| {
                let ink = |pixel: &[u8]| pixel.get(3).is_some_and(|&a| a != 0);
                let start = row.chunks_exact(4).position(ink).unwrap_or(width);
                let end = row.chunks_exact(4).rposition(ink).map_or(start, |i| i + 1);
                start..end
            })
            .collect();
        Sprite { pixmap, spans }
    }
}

/// Pre-rendered copies of one marker at [`PHASES`] x [`PHASES`] sub-pixel phases.
struct Sprites {
    /// Where the marker origin sits inside each sprite, in pixels.
    origin: (i32, i32),
    size: (u32, u32),
    phases: Vec<Option<Sprite>>,
}

impl Sprites {
    /// `None` when the sprite would be larger than [`MAX_SPRITE_SIDE`] pixels per side.
    fn new(marker: &Marker, scale: DeviceScale) -> Option<Sprites> {
        let bounds = marker.path.bounds();
        let reach = marker
            .stroke
            .as_ref()
            .map_or(0.0, |s| s.reach.max(s.stroke.width));
        // All in f64, and checked before any integer cast: a huge stroke width must fall back
        // to path rendering, not overflow.
        let s = f64::from(scale.get());
        let reach = f64::from(reach);
        let edge = |v: f32, grow: f64| (f64::from(v) + grow) * s;
        let (l, r) = (edge(bounds.left(), -reach), edge(bounds.right(), reach));
        let (t, b) = (edge(bounds.top(), -reach), edge(bounds.bottom(), reach));
        // Two extra pixels each side for anti-aliasing and the sub-pixel shift.
        let (w, h) = (r - l + 6.0, b - t + 6.0);
        let limit = f64::from(MAX_SPRITE_SIDE);
        if !(w.is_finite() && h.is_finite() && w <= limit && h <= limit) {
            return None;
        }
        // Bounded by the check above: the extent is at most 256 px around the shape.
        let (left, top) = (l.floor() as i32 - 2, t.floor() as i32 - 2);
        let (w, h) = (w.ceil() as i32, h.ceil() as i32);
        if w <= 0 || h <= 0 {
            return None;
        }
        Some(Sprites {
            origin: (-left, -top),
            size: (w as u32, h as u32),
            phases: (0..PHASES * PHASES).map(|_| None).collect(),
        })
    }

    /// Renders the sprite at `index` in `phases` unless it exists: the phase
    /// (`index % PHASES`, `index / PHASES`). `None` when it cannot be allocated.
    fn prepare(&mut self, marker: &Marker, scale: DeviceScale, index: usize) -> Option<()> {
        let (px, py) = (index % PHASES, index / PHASES);
        let slot = self.phases.get_mut(index)?;
        if slot.is_none() {
            let mut pixmap = sk::Pixmap::new(self.size.0, self.size.1)?;
            let step = 1.0 / PHASES as f32;
            let at = sk::Transform::from_scale(scale.get(), scale.get()).post_translate(
                self.origin.0 as f32 + px as f32 * step,
                self.origin.1 as f32 + py as f32 * step,
            );
            marker.draw(&mut pixmap, at, None);
            *slot = Some(Sprite::new(pixmap));
        }
        Some(())
    }

    /// Composites the queued sprites over `pixmap` in queue order and empties the queue.
    ///
    /// With enough sprites queued, the image is cut into bands of [`BAND_ROWS`] rows, dealt out
    /// to up to [`MAX_THREADS`] threads; each band belongs to one thread, which composites the
    /// queued sprites that reach it in queue order. Every pixel therefore sees the same sprites
    /// in the same order whatever the number of threads, and the output is the same.
    fn flush(&self, pixmap: &mut sk::Pixmap, queue: &mut Vec<Placement>) {
        let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);
        let threads = compositing_threads(queue.len());
        let image = pixmap.data_mut();
        let visits = if threads <= 1 {
            self.composite(
                Band {
                    top: 0,
                    data: image,
                },
                width,
                queue.iter(),
            )
        } else {
            self.composite_in_bands(image, (width, height), queue, threads)
        };
        #[cfg(test)]
        VISITS.with(|v| v.set(v.get() + visits));
        #[cfg(not(test))]
        let _ = visits;
        queue.clear();
    }

    /// Composites `queue` over `image`, `width` x `height` pixels, in bands of [`BAND_ROWS`]
    /// rows on up to `threads` threads. Each band visits only the placements that reach it, in
    /// queue order, and bands that no placement reaches are skipped. Returns how many
    /// placements were visited, once per band they reach.
    fn composite_in_bands(
        &self,
        image: &mut [u8],
        (width, height): (usize, usize),
        queue: &[Placement],
        threads: usize,
    ) -> usize {
        let mut reaching: Vec<Vec<&Placement>> = (0..height.div_ceil(BAND_ROWS))
            .map(|_| Vec::new())
            .collect();
        let (band_rows, sprite_rows) = (BAND_ROWS as i64, i64::from(self.size.1));
        let last = reaching.len() as i64 - 1;
        for placement in queue {
            let top = i64::from(placement.top);
            let first = top.max(0) / band_rows;
            let end = ((top + sprite_rows - 1).max(0) / band_rows).min(last);
            for band in first..=end {
                if let Some(placements) = reaching.get_mut(band as usize) {
                    placements.push(placement);
                }
            }
        }
        let work: Vec<(Band<'_>, Vec<&Placement>)> = image
            .chunks_mut(BAND_ROWS * width * 4)
            .zip(reaching)
            .enumerate()
            .filter(|(_, (_, placements))| !placements.is_empty())
            .map(|(i, (data, placements))| {
                let band = Band {
                    top: i * BAND_ROWS,
                    data,
                };
                (band, placements)
            })
            .collect();
        let threads = threads.min(work.len());
        let run = |share: Vec<(Band<'_>, Vec<&Placement>)>| -> usize {
            share
                .into_iter()
                .map(|(band, placements)| self.composite(band, width, placements.into_iter()))
                .sum()
        };
        if threads <= 1 {
            return run(work);
        }
        let mut shares: Vec<Vec<(Band<'_>, Vec<&Placement>)>> =
            (0..threads).map(|_| Vec::new()).collect();
        for (i, item) in work.into_iter().enumerate() {
            if let Some(share) = shares.get_mut(i % threads) {
                share.push(item);
            }
        }
        std::thread::scope(|scope| {
            let workers: Vec<_> = shares
                .into_iter()
                .map(|share| scope.spawn(move || run(share)))
                .collect();
            workers
                .into_iter()
                .map(|worker| {
                    worker
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                })
                .sum()
        })
    }

    /// Composites `placements`, in order, over `band` of a `width`-pixel image. Returns how
    /// many placements were visited.
    fn composite<'q>(
        &self,
        mut band: Band<'_>,
        width: usize,
        placements: impl Iterator<Item = &'q Placement>,
    ) -> usize {
        let mut visits = 0;
        for placement in placements {
            visits += 1;
            if let Some(Some(sprite)) = self.phases.get(placement.phase) {
                blit_over(&mut band, width, sprite, placement.left, placement.top);
            }
        }
        visits
    }
}

/// Rows of the image per band when sprites are composited on several threads.
const BAND_ROWS: usize = 16;

/// The most threads that composite sprites.
const MAX_THREADS: usize = 8;

/// The fewest queued sprites per thread for which an extra thread pays for starting. Measured
/// with 6 x 4 in, 200 dpi scatters: threads make no difference below about 8,000 markers and
/// cut the time of 100,000 by a third.
const MIN_SPRITES_PER_THREAD: usize = 1024;

// Lets tests choose the number of compositing threads, and count the placements composited
// over each band.
#[cfg(test)]
thread_local! {
    static FORCE_THREADS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many threads composite `queued` sprites: one per [`MIN_SPRITES_PER_THREAD`] sprites, at
/// most the available parallelism and at most [`MAX_THREADS`], and at least one.
fn compositing_threads(queued: usize) -> usize {
    #[cfg(test)]
    if let Some(threads) = FORCE_THREADS.with(std::cell::Cell::get) {
        return threads;
    }
    static AVAILABLE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let available = *AVAILABLE.get_or_init(|| {
        std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
    });
    let most = available.clamp(1, MAX_THREADS);
    (queued / MIN_SPRITES_PER_THREAD).clamp(1, most)
}

/// The rows `top..` of an image, as many as `data` holds.
struct Band<'a> {
    top: usize,
    data: &'a mut [u8],
}

/// The most sprite placements queued before they are composited. Queuing separates finding
/// positions from compositing, which keeps the compositing loop tight; the limit bounds the
/// queue's memory for any number of markers (16 bytes a placement) and leaves enough sprites in
/// a full queue for [`MAX_THREADS`] threads.
const QUEUE_LIMIT: usize = 32_768;

/// A sprite at the whole pixel (`left`, `top`) that needs no clip mask: there is no clip, or
/// every pixel of the sprite is one the clip mask covers fully, so masking would change
/// nothing. [`Placement::unmasked`] is the only way to make one.
#[derive(Debug, Clone, Copy)]
struct Placement {
    /// The sprite's index in [`Sprites::phases`].
    phase: usize,
    left: i32,
    top: i32,
}

impl Placement {
    /// The placement of a `size` sprite at (`left`, `top`), or `None` when part of it lies
    /// outside `interior`, the whole pixels the clip mask covers fully (`None` for no clip).
    fn unmasked(
        interior: Option<(i32, i32, i32, i32)>,
        phase: usize,
        (left, top): (i32, i32),
        (w, h): (i32, i32),
    ) -> Option<Placement> {
        if let Some((il, it, ir, ib)) = interior
            && (left < il || top < it || left + w > ir || top + h > ib)
        {
            return None;
        }
        Some(Placement { phase, left, top })
    }
}

/// Whether `mask` is 255 at every pixel of the rectangle at (`left`, `top`) of size `w` x `h`
/// that lies inside the mask. Checks the invariant of [`Placement`] in debug builds.
fn mask_is_full(mask: &sk::Mask, (left, top): (i32, i32), (w, h): (i32, i32)) -> bool {
    let (mw, mh) = (mask.width() as i32, mask.height() as i32);
    let data = mask.data();
    (top.max(0)..(top + h).min(mh)).all(|y| {
        (left.max(0)..(left + w).min(mw))
            .all(|x| data.get((y * mw + x) as usize).copied() == Some(255))
    })
}

/// Splits a device coordinate into a whole pixel and the nearest 1/8-pixel phase.
fn split_phase(d: DevicePixels) -> (i32, usize) {
    let steps = (d.0 * PHASES as f32).round();
    let whole = (steps / PHASES as f32).floor();
    let phase = steps - whole * PHASES as f32;
    (whole as i32, phase as usize)
}

/// Draws `shape` at every one of `positions`, in order.
///
/// Markers are composited from cached sprites where the clip mask, if any, covers the whole
/// sprite. A marker that reaches the clip edge is drawn as paths through the mask, because a
/// sprite is a composited fill and stroke, which is only equal to masking fill and stroke
/// separately where the mask is 255. Sprite placements are queued and composited in batches;
/// the queue is flushed before every marker drawn as paths, so markers keep their order.
#[allow(clippy::too_many_arguments)]
fn draw_markers(
    pixmap: &mut sk::Pixmap,
    scale: DeviceScale,
    root: sk::Transform,
    clip: Option<(&sk::Mask, ClipRect)>,
    shape: &crate::geometry::Path,
    fill: Option<Rgba>,
    stroke: Option<&StrokeStyle>,
    positions: &[Point],
) {
    let Some(normalized) = normalize_path(shape) else {
        return;
    };
    let Some(path) = convert_path(&normalized) else {
        return;
    };
    let marker = Marker {
        fill: fill.filter(|c| c.a > 0).map(solid_paint),
        stroke: stroke.and_then(|s| prepare_stroke(&normalized, &path, s, root)),
        path,
    };
    if marker.fill.is_none() && marker.stroke.is_none() {
        return;
    }
    let mut sprites = if sprites_disabled() {
        None
    } else {
        Sprites::new(&marker, scale)
    };
    let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
    let mask = clip.map(|(m, _)| m);
    let interior = clip.map(|(_, c)| c.interior(pixmap.width(), pixmap.height()));
    let mut queue = Vec::new();
    for p in positions.iter().filter(|p| valid_point(**p)) {
        let Some(sprites) = sprites.as_mut() else {
            marker.draw(pixmap, root.pre_translate(p.x, p.y), mask);
            continue;
        };
        let (x, px) = split_phase(scale.to_device(p.x));
        let (y, py) = split_phase(scale.to_device(p.y));
        let (left, top) = (x - sprites.origin.0, y - sprites.origin.1);
        let (w, h) = (sprites.size.0 as i32, sprites.size.1 as i32);
        if left >= width || top >= height || left + w <= 0 || top + h <= 0 {
            continue;
        }
        let phase = py * PHASES + px;
        let Some(placement) = Placement::unmasked(interior, phase, (left, top), (w, h)) else {
            sprites.flush(pixmap, &mut queue);
            marker.draw(pixmap, root.pre_translate(p.x, p.y), mask);
            continue;
        };
        if sprites.prepare(&marker, scale, phase).is_none() {
            continue;
        }
        debug_assert!(mask.is_none_or(|m| mask_is_full(m, (left, top), (w, h))));
        queue.push(placement);
        if queue.len() == QUEUE_LIMIT {
            sprites.flush(pixmap, &mut queue);
        }
    }
    if let Some(sprites) = &sprites {
        sprites.flush(pixmap, &mut queue);
    }
}

fn div255(x: u32) -> u32 {
    let x = x + 128;
    (x + (x >> 8)) >> 8
}

/// Composites the premultiplied `sprite` over the rows of `band` of a `width`-pixel image, with
/// the sprite's top-left corner at (`left`, `top`) of the image, source-over, without a clip
/// mask (see [`Placement`]).
///
/// This is `draw_pixmap` without the per-call shader setup, which dominates when drawing
/// hundreds of thousands of small sprites. Each row visits only its span of non-transparent
/// pixels and copies opaque ones. All arithmetic is integer, so output is exact.
fn blit_over(band: &mut Band<'_>, width: usize, sprite: &Sprite, left: i32, top: i32) {
    let (width, sw) = (width as i64, i64::from(sprite.pixmap.width()));
    let (left, top) = (i64::from(left), i64::from(top));
    let band_top = band.top as i64;
    let band_bottom = band_top + (band.data.len() as i64) / (width * 4).max(1);
    let src = sprite.pixmap.data();
    let y0 = top.max(band_top);
    let y1 = (top + sprite.spans.len() as i64).min(band_bottom);
    for y in y0..y1 {
        let sy = y - top;
        let Some(span) = sprite.spans.get(sy as usize) else {
            continue;
        };
        let x0 = (left + span.start as i64).max(0);
        let x1 = (left + span.end as i64).min(width);
        if x0 >= x1 {
            continue;
        }
        let si = ((sy * sw + (x0 - left)) * 4) as usize;
        let di = (((y - band_top) * width + x0) * 4) as usize;
        let n = ((x1 - x0) * 4) as usize;
        // Both ranges are inside their images by the clamping above.
        let (Some(src_row), Some(dst_row)) = (src.get(si..si + n), band.data.get_mut(di..di + n))
        else {
            continue;
        };
        blend_row(dst_row, src_row);
    }
}

/// Composites the premultiplied pixels `src` over `dst`, source-over: transparent pixels are
/// skipped, opaque ones copied.
///
/// Kept out of line: inlined into [`blit_over`], the per-pixel blend is no longer vectorized,
/// which made compositing a 100,000-marker scatter about 25 ms slower.
#[inline(never)]
fn blend_row(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let alpha = u32::from(s[3]);
        if alpha == 0 {
            continue;
        }
        if alpha == 255 {
            d.copy_from_slice(s);
            continue;
        }
        let inv = 255 - alpha;
        for (dc, &sc) in d.iter_mut().zip(s) {
            *dc = (u32::from(sc) + div255(u32::from(*dc) * inv)).min(255) as u8;
        }
    }
}

fn encode(pixmap: sk::Pixmap, dpi: u32) -> Result<Vec<u8>> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let data = pixmap.take_demultiplied();
    let fail = |e: ::png::EncodingError| {
        Error::style(
            CONTEXT,
            format!("the PNG could not be encoded ({e}); try a smaller figure."),
        )
    };
    let mut out = Vec::new();
    let mut encoder = ::png::Encoder::new(&mut out, width, height);
    encoder.set_color(::png::ColorType::Rgba);
    encoder.set_depth(::png::BitDepth::Eight);
    // Deflate level 3 with adaptive row filters: on a dense 1200 x 800 plot about 40% of the
    // encoding time of level 6, for files about 2% larger.
    encoder.set_deflate_compression(::png::DeflateCompression::Level(3));
    encoder.set_filter(::png::Filter::Adaptive);
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
    use crate::geometry::Path;
    use crate::scene::{GradientStop, LinearGradient};
    use crate::test_support::image::{DecodedPng as Decoded, decode_rgba8};
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

    impl Decoded {
        fn is_ink(&self, x: u32, y: u32) -> bool {
            self.at(x, y)[0] < 128
        }
    }

    fn decode(bytes: &[u8]) -> Decoded {
        decode_rgba8(bytes).unwrap()
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
    fn sampled_paint_evaluates_original_cmap_at_pixel_centers() {
        let cmap = crate::Cmap::from_colors(&[BLACK, WHITE, Rgba::RED]).unwrap();
        for dpi in [72, 144] {
            let mut scene = Scene::new(200.0, 20.0, WHITE);
            scene.push(Item::Group {
                clip: Some(Rect {
                    x: 20.0,
                    y: 0.0,
                    width: 160.0,
                    height: 20.0,
                }),
                items: vec![Item::Fill {
                    path: rect_path(0.0, 0.0, 200.0, 20.0),
                    paint: Paint::Sampled {
                        cmap: cmap.clone(),
                        from: Point { x: 0.0, y: 0.0 },
                        to: Point { x: 200.0, y: 0.0 },
                    },
                    rule: FillRule::NonZero,
                }],
            });
            let image = decode(&render_png(&scene, dpi).unwrap());
            let scale = dpi / 72;
            assert_eq!(image.at(10 * scale, 10 * scale), [255; 4]);
            for x in 21 * scale..179 * scale {
                let color = cmap.at((f64::from(x) + 0.5) / f64::from(scale) / 200.0);
                assert_eq!(
                    image.at(x, 10 * scale),
                    [color.r, color.g, color.b, color.a]
                );
            }
        }
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
    fn dpi_beyond_u32_keeps_its_value() {
        for dpi in [i64::MIN, -72, 0, 9, 2401, i64::from(u32::MAX) + 1, i64::MAX] {
            let err = check_dpi("Figure.to_png()", dpi).unwrap_err();
            assert_eq!(
                err.to_string(),
                format!("Figure.to_png(): dpi {dpi} is out of range; use a value from 10 to 2400.")
            );
            assert_eq!(err.parameter(), Some("dpi"));
        }
        assert_eq!(check_dpi("Figure.to_png()", 10).unwrap(), 10);
        assert_eq!(check_dpi("Figure.to_png()", 2400).unwrap(), 2400);
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
        // 10,000 x 7,000 px is 7e7 pixels: over 2^26, under the old 2^28.
        check(2_400.0, 1_680.0, 300);
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

    fn pt(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn path_of(verbs: &[(char, &[f32])]) -> Path {
        let mut p = Path::new();
        for (v, c) in verbs {
            match (v, c) {
                ('M', [x, y]) => p.move_to(pt(*x, *y)),
                ('L', [x, y]) => p.line_to(pt(*x, *y)),
                ('Q', [a, b, x, y]) => p.quad_to(pt(*a, *b), pt(*x, *y)),
                ('C', [a, b, c, d, x, y]) => p.cubic_to(pt(*a, *b), pt(*c, *d), pt(*x, *y)),
                _ => p.close(),
            }
        }
        p
    }

    fn raster(scene: &Scene, dpi: u32) -> Decoded {
        decode(&render_png(scene, dpi).unwrap())
    }

    #[test]
    fn huge_control_points_are_skipped() {
        // Finite but huge control point: used to abort tiny-skia in release.
        let mut scene = Scene::new(40.0, 20.0, WHITE);
        scene.push(Item::Stroke {
            path: path_of(&[
                ('M', &[1.0, 1.0]),
                ('Q', &[1e10, -1e10, 30.0, 10.0]),
                ('L', &[20.0, 15.0]),
            ]),
            style: stroke_style(1.0, vec![]),
        });
        let before = REFUSED.with(std::cell::Cell::get);
        let image = raster(&scene, 200);
        assert!(image.rgba.chunks(4).all(|p| p == [255, 255, 255, 255]));
        assert_eq!(REFUSED.with(std::cell::Cell::get), before);
    }

    #[test]
    fn invalid_segment_breaks_the_subpath() {
        let mut scene = Scene::new(40.0, 20.0, WHITE);
        scene.push(Item::Stroke {
            path: path_of(&[
                ('M', &[2.0, 10.0]),
                ('L', &[9.0, 10.0]),
                ('L', &[f32::NAN, 10.0]),
                ('L', &[30.0, 10.0]),
                ('L', &[38.0, 10.0]),
            ]),
            style: stroke_style(2.0, vec![]),
        });
        // A close-only subpath draws nothing, even with round caps.
        scene.push(Item::Stroke {
            path: path_of(&[('M', &[20.0, 3.0]), ('Z', &[])]),
            style: StrokeStyle {
                cap: LineCap::Round,
                ..stroke_style(6.0, vec![])
            },
        });
        let image = raster(&scene, 72);
        assert!(image.is_ink(5, 10));
        assert!(!image.is_ink(18, 10), "the gap was bridged");
        assert!(image.is_ink(34, 10));
        assert!(!image.is_ink(20, 3), "a lone move and close drew a dot");
    }

    #[test]
    fn odd_dash_arrays_repeat_like_svg() {
        let mut scene = Scene::new(60.0, 10.0, WHITE);
        scene.push(Item::Stroke {
            path: line_path(0.0, 60.0, 5.0),
            style: stroke_style(2.0, vec![6.0]),
        });
        let image = raster(&scene, 72);
        let row: Vec<bool> = (0..60).map(|x| image.is_ink(x, 5)).collect();
        assert!(row.iter().any(|ink| *ink) && row.iter().any(|ink| !*ink));
    }

    #[test]
    fn non_finite_gradient_stops_are_dropped() {
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
                start: pt(0.0, 0.0),
                end: pt(100.0, 0.0),
                stops: vec![
                    GradientStop {
                        offset: f32::NAN,
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
        let image = raster(&scene, 72);
        assert_eq!(image.at(0, 10), [0, 0, 255, 255]);
        assert_eq!(image.at(99, 10), [0, 0, 255, 255]);
    }

    fn fractional_clip_scene(nested: bool) -> Scene {
        let clip = Rect {
            x: 3.4,
            y: 3.4,
            width: 20.3,
            height: 10.3,
        };
        let content = fill(rect_path(0.0, 0.0, 40.0, 20.0), BLACK);
        let mut scene = Scene::new(40.0, 20.0, WHITE);
        let inner = Item::Group {
            clip: Some(clip),
            items: vec![content],
        };
        scene.push(if nested {
            Item::Group {
                clip: Some(clip),
                items: vec![inner],
            }
        } else {
            inner
        });
        scene
    }

    #[test]
    fn same_fractional_clip_nested_twice_matches_one_clip() {
        let single = raster(&fractional_clip_scene(false), 72);
        let nested = raster(&fractional_clip_scene(true), 72);
        assert_eq!(single.rgba, nested.rgba);
        // The edge really is fractional.
        let edge = single.at(3, 8)[0];
        assert!(edge > 0 && edge < 255, "edge pixel {edge}");
    }

    #[test]
    fn disjoint_nested_clips_omit_the_group() {
        let mut scene = Scene::new(40.0, 20.0, WHITE);
        let inner = Item::Group {
            clip: Some(Rect {
                x: 25.0,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            }),
            items: vec![fill(rect_path(0.0, 0.0, 40.0, 20.0), BLACK)],
        };
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            }),
            items: vec![inner],
        });
        let image = raster(&scene, 72);
        assert!(image.rgba.chunks(4).all(|p| p == [255, 255, 255, 255]));
    }

    #[test]
    fn deep_group_chains_do_not_overflow_the_stack() {
        let handle = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let mut item = fill(rect_path(0.0, 0.0, 10.0, 10.0), BLACK);
                for _ in 0..100_000 {
                    item = Item::Group {
                        clip: None,
                        items: vec![item],
                    };
                }
                let mut scene = Scene::new(10.0, 10.0, WHITE);
                scene.push(item);
                let image = raster(&scene, 72);
                // The content sits deeper than MAX_GROUP_DEPTH, so it is omitted.
                assert_eq!(image.at(5, 5), [255, 255, 255, 255]);
            })
            .unwrap();
        handle.join().unwrap();
    }

    /// 50 circle markers with a white outline. With `snap`, positions sit exactly on the
    /// 1/8-pixel grid (at 100 dpi) that the sprite cache quantizes to.
    fn markers_scene(count: usize, snap: bool) -> Scene {
        let mut state: u32 = 7;
        let mut next = move || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
            ((state >> 8) & 0xffff) as f32 / 65535.0
        };
        let grid = |v: f32| {
            if snap {
                (v * 100.0 / 72.0 * PHASES as f32).round() / PHASES as f32 * 72.0 / 100.0
            } else {
                v
            }
        };
        let mut scene = Scene::new(120.0, 80.0, WHITE);
        let positions = (0..count)
            .map(|_| pt(grid(10.0 + next() * 100.0), grid(10.0 + next() * 60.0)))
            .collect();
        let mut circle = Path::new();
        circle.move_to(pt(4.0, 0.0));
        circle.cubic_to(pt(4.0, 2.2), pt(2.2, 4.0), pt(0.0, 4.0));
        circle.cubic_to(pt(-2.2, 4.0), pt(-4.0, 2.2), pt(-4.0, 0.0));
        circle.cubic_to(pt(-4.0, -2.2), pt(-2.2, -4.0), pt(0.0, -4.0));
        circle.cubic_to(pt(2.2, -4.0), pt(4.0, -2.2), pt(4.0, 0.0));
        circle.close();
        scene.push(Item::Markers {
            shape: circle,
            fill: Some(Rgba {
                r: 40,
                g: 120,
                b: 190,
                a: 255,
            }),
            stroke: Some(StrokeStyle {
                color: WHITE,
                ..stroke_style(0.8, vec![])
            }),
            positions,
        });
        scene
    }

    /// (fraction of pixels with a channel delta over 8, largest channel delta) between sprite
    /// and path rendering of `scene`.
    fn sprite_vs_path(scene: &Scene) -> (f64, u8) {
        FORCE_PATHS.with(|f| f.set(true));
        let paths = raster(scene, 100);
        FORCE_PATHS.with(|f| f.set(false));
        let sprites = raster(scene, 100);
        assert_eq!(paths.rgba.len(), sprites.rgba.len());
        let mut different = 0;
        let mut largest = 0;
        for (a, b) in paths.rgba.chunks(4).zip(sprites.rgba.chunks(4)) {
            let delta = a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            largest = largest.max(delta);
            if delta > 8 {
                different += 1;
            }
        }
        (different as f64 / (paths.rgba.len() / 4) as f64, largest)
    }

    #[test]
    fn marker_sprites_match_path_rendering_on_the_phase_grid() {
        let (fraction, largest) = sprite_vs_path(&markers_scene(50, true));
        eprintln!(
            "on-grid: {:.4}% over 8, largest {largest}",
            fraction * 100.0
        );
        assert!(fraction <= 0.005 && largest <= 64);
    }

    #[test]
    fn marker_sprites_respect_clip_masks() {
        let mut scene = Scene::new(120.0, 80.0, WHITE);
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 20.3,
                y: 15.7,
                width: 60.0,
                height: 30.0,
            }),
            items: std::mem::take(&mut markers_scene(50, true).items),
        });
        let (fraction, largest) = sprite_vs_path(&scene);
        assert!(fraction <= 0.005 && largest <= 64, "{fraction} {largest}");
        let image = raster(&scene, 100);
        // Nothing outside the clip: the first 28 px columns (20.3 pt = 28.2 px) stay white.
        assert!((0..27).all(|x| (0..111).all(|y| image.at(x, y) == [255, 255, 255, 255])));
    }

    #[test]
    fn marker_sprites_match_path_rendering_at_random_positions() {
        let (fraction, largest) = sprite_vs_path(&markers_scene(50, false));
        eprintln!("random: {:.4}% over 8, largest {largest}", fraction * 100.0);
        // Snapping to 1/8 pixel moves an edge by up to 1/16 px per axis (about 16 levels on a
        // vertical or horizontal edge, a little more on diagonals), so the golden fraction limit
        // (0.5%) cannot hold here. Measured: 3.06% over 8, largest 42; limits keep a margin.
        assert!(largest <= 48, "largest channel difference {largest}");
        assert!(
            fraction <= 0.04,
            "{:.2}% of pixels differ by more than 8",
            fraction * 100.0
        );
    }

    /// Renders `scene` with sprites and with path rendering; returns (largest channel delta,
    /// sprite image, path image).
    fn both_ways(scene: &Scene, dpi: u32) -> (u8, Decoded, Decoded) {
        FORCE_PATHS.with(|f| f.set(true));
        let paths = raster(scene, dpi);
        FORCE_PATHS.with(|f| f.set(false));
        let sprites = raster(scene, dpi);
        let largest = paths
            .rgba
            .iter()
            .zip(&sprites.rgba)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        (largest, sprites, paths)
    }

    #[test]
    fn huge_stroke_width_falls_back_to_paths_instead_of_overflowing() {
        let mut scene = Scene::new(20.0, 20.0, WHITE);
        scene.push(Item::Markers {
            shape: rect_path(-1.0, -1.0, 2.0, 2.0),
            fill: Some(BLACK),
            stroke: Some(StrokeStyle {
                color: BLACK,
                ..stroke_style(3.0e38, vec![])
            }),
            positions: vec![pt(10.0, 10.0)],
        });
        let (largest, sprites, paths) = both_ways(&scene, 72);
        assert_eq!(largest, 0);
        assert_eq!(sprites.at(10, 10), paths.at(10, 10));
        assert_eq!(sprites.at(10, 10), [0, 0, 0, 255]);
    }

    #[test]
    fn markers_on_a_fractional_clip_edge_match_path_rendering() {
        // Half-transparent fill under a half-transparent stroke, straddling a clip edge at
        // x = 10.5 px: masking the composite is not masking fill and stroke separately.
        let mut scene = Scene::new(30.0, 20.0, WHITE);
        let half = |r, g, b| Rgba { r, g, b, a: 128 };
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 10.5,
                y: 0.0,
                width: 15.0,
                height: 20.0,
            }),
            items: vec![Item::Markers {
                shape: rect_path(-4.0, -4.0, 8.0, 8.0),
                fill: Some(half(0, 0, 0)),
                stroke: Some(StrokeStyle {
                    color: half(0, 0, 0),
                    ..stroke_style(4.0, vec![])
                }),
                positions: vec![pt(10.0, 10.0), pt(20.0, 10.0)],
            }],
        });
        let (largest, _, _) = both_ways(&scene, 72);
        assert!(largest <= 2, "largest channel difference {largest}");
    }

    #[test]
    fn clips_use_device_bounds_not_the_logical_figure_size() {
        // 10.51 pt at 72 dpi is 11 px wide; the last column is only 0.51 pt inside the figure.
        let mut scene = Scene::new(10.51, 4.0, WHITE);
        scene.push(Item::Group {
            clip: Some(Rect {
                x: -1.0,
                y: -1.0,
                width: 30.0,
                height: 30.0,
            }),
            items: vec![fill(rect_path(-1.0, -1.0, 30.0, 30.0), BLACK)],
        });
        let image = raster(&scene, 72);
        assert_eq!(image.width, 11);
        assert_eq!(image.at(10, 2), [0, 0, 0, 255]);
        assert_eq!(image.at(0, 0), [0, 0, 0, 255]);
        assert_eq!(image.at(10, 3), [0, 0, 0, 255]);
    }

    #[test]
    fn zero_length_paths_draw_caps_even_when_dashed() {
        for dash in [vec![], vec![2.0, 2.0]] {
            let mut scene = Scene::new(20.0, 20.0, WHITE);
            scene.push(Item::Stroke {
                path: path_of(&[('M', &[10.0, 10.0]), ('L', &[10.0, 10.0])]),
                style: StrokeStyle {
                    cap: LineCap::Round,
                    ..stroke_style(4.0, dash.clone())
                },
            });
            let image = raster(&scene, 72);
            assert!(image.is_ink(10, 10), "no dot for dash {dash:?}");
            assert!(image.is_ink(11, 10) && image.is_ink(10, 11));
            assert!(!image.is_ink(13, 10) && !image.is_ink(10, 7));
        }
        // Butt caps draw nothing for a zero-length stroke.
        let mut scene = Scene::new(20.0, 20.0, WHITE);
        scene.push(Item::Stroke {
            path: path_of(&[('M', &[10.0, 10.0]), ('L', &[10.0, 10.0])]),
            style: stroke_style(4.0, vec![]),
        });
        assert!(!raster(&scene, 72).is_ink(10, 10));
    }

    #[test]
    fn marker_order_is_preserved_in_sprites() {
        let mut scene = Scene::new(40.0, 20.0, WHITE);
        let square = rect_path(-5.0, -5.0, 10.0, 10.0);
        for (c, x) in [
            (BLACK, 15.0),
            (
                Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                20.0,
            ),
        ] {
            scene.push(Item::Markers {
                shape: square.clone(),
                fill: Some(c),
                stroke: None,
                positions: vec![pt(x, 10.0)],
            });
        }
        let image = raster(&scene, 72);
        assert_eq!(image.at(17, 10), [255, 0, 0, 255]);
        assert_eq!(image.at(12, 10), [0, 0, 0, 255]);
    }

    /// The marker loop as it was before sprite placements were queued, kept as the reference the
    /// queued loop must match byte for byte: each marker is composited as soon as it is reached,
    /// and every sprite pixel goes through the clip mask. Returns how many markers were drawn
    /// as paths because they reach the clip edge.
    #[allow(clippy::too_many_arguments)]
    fn reference_draw_markers(
        pixmap: &mut sk::Pixmap,
        scale: DeviceScale,
        root: sk::Transform,
        clip: Option<(&sk::Mask, ClipRect)>,
        shape: &Path,
        fill: Option<Rgba>,
        stroke: Option<&StrokeStyle>,
        positions: &[Point],
    ) -> usize {
        let normalized = normalize_path(shape).unwrap();
        let path = convert_path(&normalized).unwrap();
        let marker = Marker {
            fill: fill.filter(|c| c.a > 0).map(solid_paint),
            stroke: stroke.and_then(|s| prepare_stroke(&normalized, &path, s, root)),
            path,
        };
        let mut sprites = Sprites::new(&marker, scale).unwrap();
        let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
        let mask = clip.map(|(m, _)| m);
        let interior = clip.map(|(_, c)| c.interior(pixmap.width(), pixmap.height()));
        let mut as_paths = 0;
        for p in positions.iter().filter(|p| valid_point(**p)) {
            let (x, px) = split_phase(scale.to_device(p.x));
            let (y, py) = split_phase(scale.to_device(p.y));
            let (left, top) = (x - sprites.origin.0, y - sprites.origin.1);
            let (w, h) = (sprites.size.0 as i32, sprites.size.1 as i32);
            if left >= width || top >= height || left + w <= 0 || top + h <= 0 {
                continue;
            }
            if let Some((il, it, ir, ib)) = interior
                && (left < il || top < it || left + w > ir || top + h > ib)
            {
                marker.draw(pixmap, root.pre_translate(p.x, p.y), mask);
                as_paths += 1;
                continue;
            }
            let phase = py * PHASES + px;
            sprites.prepare(&marker, scale, phase).unwrap();
            let sprite = sprites.phases[phase].as_ref().unwrap();
            reference_blit(pixmap, &sprite.pixmap, left, top, mask);
        }
        as_paths
    }

    /// The sprite compositing as it was before row spans: every sprite pixel, scaled by the
    /// mask coverage when there is a mask.
    fn reference_blit(
        pixmap: &mut sk::Pixmap,
        sprite: &sk::Pixmap,
        left: i32,
        top: i32,
        mask: Option<&sk::Mask>,
    ) {
        let (dw, dh) = (pixmap.width() as i32, pixmap.height() as i32);
        let (sw, sh) = (sprite.width() as i32, sprite.height() as i32);
        let (x0, x1) = (left.max(0), (left + sw).min(dw));
        let (y0, y1) = (top.max(0), (top + sh).min(dh));
        let mask_data = mask.map(sk::Mask::data);
        let src = sprite.data();
        let dst = pixmap.data_mut();
        for y in y0..y1 {
            for x in x0..x1 {
                let si = (((y - top) * sw + (x - left)) * 4) as usize;
                let di = ((y * dw + x) * 4) as usize;
                let s = &src[si..si + 4];
                let mut s = [s[0], s[1], s[2], s[3]].map(u32::from);
                if s[3] == 0 {
                    continue;
                }
                if let Some(data) = mask_data {
                    let m = u32::from(data[(y * dw + x) as usize]);
                    if m == 0 {
                        continue;
                    }
                    if m != 255 {
                        s = s.map(|c| div255(c * m));
                    }
                }
                let inv = 255 - s[3];
                for (dc, sc) in dst[di..di + 4].iter_mut().zip(s) {
                    *dc = (sc + div255(u32::from(*dc) * inv)).min(255) as u8;
                }
            }
        }
    }

    fn circle(r: f32) -> Path {
        let k = 0.552_284_8 * r;
        let mut path = Path::new();
        path.move_to(pt(r, 0.0));
        path.cubic_to(pt(r, k), pt(k, r), pt(0.0, r));
        path.cubic_to(pt(-k, r), pt(-r, k), pt(-r, 0.0));
        path.cubic_to(pt(-r, -k), pt(-k, -r), pt(0.0, -r));
        path.cubic_to(pt(k, -r), pt(r, -k), pt(r, 0.0));
        path.close();
        path
    }

    /// The numbers of compositing threads every byte-identity test runs with.
    const THREAD_COUNTS: [usize; 4] = [1, 2, 3, 8];

    /// Runs `f` with sprite compositing forced onto `threads` threads.
    fn on_threads<T>(threads: usize, f: impl FnOnce() -> T) -> T {
        FORCE_THREADS.with(|t| t.set(Some(threads)));
        let out = f();
        FORCE_THREADS.with(|t| t.set(None));
        out
    }

    /// Draws translucent outlined circles at `positions` with [`draw_markers`] on each of
    /// [`THREAD_COUNTS`] threads and with [`reference_draw_markers`], each onto the same
    /// non-uniform 200 x 140 px background at 100 dpi, clipped to `clip` (in points) when given.
    /// Returns the pixel buffers of `draw_markers`, one per thread count, the reference buffer
    /// and how many markers the reference drew as paths.
    fn queued_and_reference(
        clip: Option<Rect>,
        positions: &[Point],
    ) -> (Vec<Vec<u8>>, Vec<u8>, usize) {
        let (width, height) = (200, 140);
        let scale = DeviceScale::new(100);
        let root = sk::Transform::from_scale(scale.get(), scale.get());
        let mut background = sk::Pixmap::new(width, height).unwrap();
        for (i, pixel) in background.data_mut().chunks_exact_mut(4).enumerate() {
            let (x, y) = (i as u32 % width, i as u32 / width);
            pixel.copy_from_slice(&[
                (x * 255 / width) as u8,
                (y * 255 / height) as u8,
                ((x * 7 + y * 3) % 256) as u8,
                255,
            ]);
        }
        let clip = clip.map(|r| ClipRect::from_rect(&r, scale, width, height).unwrap());
        let mut canvas = Canvas {
            pixmap: background.clone(),
            scale,
            root,
            mask: None,
        };
        assert!(canvas.use_clip(clip));
        let clip = canvas.mask.as_ref().map(|(c, m)| (m, *c));
        let shape = circle(4.0);
        let fill = Rgba {
            r: 200,
            g: 40,
            b: 90,
            a: 140,
        };
        let stroke = StrokeStyle {
            color: Rgba {
                r: 20,
                g: 90,
                b: 200,
                a: 190,
            },
            ..stroke_style(1.2, vec![])
        };
        let queued = THREAD_COUNTS
            .iter()
            .map(|&threads| {
                let mut queued = background.clone();
                on_threads(threads, || {
                    draw_markers(
                        &mut queued,
                        scale,
                        root,
                        clip,
                        &shape,
                        Some(fill),
                        Some(&stroke),
                        positions,
                    );
                });
                queued.take()
            })
            .collect();
        let mut reference = background;
        let as_paths = reference_draw_markers(
            &mut reference,
            scale,
            root,
            clip,
            &shape,
            Some(fill),
            Some(&stroke),
            positions,
        );
        (queued, reference.take(), as_paths)
    }

    fn assert_same_pixels(queued: &[Vec<u8>], reference: &[u8]) {
        for (queued, threads) in queued.iter().zip(THREAD_COUNTS) {
            assert_eq!(queued.len(), reference.len());
            let first = queued.iter().zip(reference).position(|(a, b)| a != b);
            assert!(
                first.is_none(),
                "{threads} threads: first differing byte at {first:?}"
            );
        }
    }

    /// A clip with fractional edges, 30.3 to 110.3 pt across and 20.6 to 70.6 pt down.
    const FRACTIONAL_CLIP: Rect = Rect {
        x: 30.3,
        y: 20.6,
        width: 80.0,
        height: 50.0,
    };

    #[test]
    fn queued_markers_keep_their_order_across_clip_edges() {
        // Overlapping markers alternate between straddling the left or top clip edge (drawn as
        // paths through the mask) and lying just inside it (composited from sprites).
        let positions: Vec<Point> = (0..400)
            .map(|i| {
                let jitter = (i % 5) as f32 * 1.3 - 2.6;
                let along = 25.0 + (i * 37 % 400) as f32 * 0.1;
                match i % 4 {
                    0 => pt(30.3 + jitter, along),
                    1 => pt(30.3 + 10.0 + jitter, along),
                    2 => pt(along + 10.0, 20.6 + jitter),
                    _ => pt(along + 10.0, 20.6 + 10.0 + jitter),
                }
            })
            .collect();
        let (queued, reference, as_paths) = queued_and_reference(Some(FRACTIONAL_CLIP), &positions);
        assert!(
            as_paths > 100 && as_paths < 300,
            "{as_paths} markers drawn as paths"
        );
        assert_same_pixels(&queued, &reference);
    }

    #[test]
    fn queued_markers_match_the_reference_without_a_clip() {
        // Markers partly off every side of the image, interleaved with markers inside it.
        let positions: Vec<Point> = (0..300)
            .map(|i| {
                let t = (i * 53 % 300) as f32 / 300.0;
                match i % 3 {
                    0 => pt(t * 144.0, -1.0 + (i % 4) as f32 * 1.5),
                    1 => pt(143.0 + (i % 4) as f32, t * 100.8),
                    _ => pt(20.0 + t * 100.0, 20.0 + t * 60.0),
                }
            })
            .collect();
        let (queued, reference, as_paths) = queued_and_reference(None, &positions);
        assert_eq!(as_paths, 0);
        assert_same_pixels(&queued, &reference);
    }

    #[test]
    fn queued_markers_match_the_reference_past_the_queue_limit() {
        // A run of interior markers longer than the queue limit between two markers on the clip
        // edge.
        let count = QUEUE_LIMIT + 3000;
        let positions: Vec<Point> = (0..count)
            .map(|i| {
                if i == 1000 || i == QUEUE_LIMIT + 1500 {
                    // Reaches past the clip interior and overlaps the markers that follow.
                    pt(37.0, 45.0)
                } else {
                    let (u, v) = ((i * 7919 % 1000) as f32, (i * 104_729 % 1000) as f32);
                    pt(40.0 + u * 0.06, 30.0 + v * 0.03)
                }
            })
            .collect();
        let (queued, reference, as_paths) = queued_and_reference(Some(FRACTIONAL_CLIP), &positions);
        assert_eq!(as_paths, 2);
        assert_same_pixels(&queued, &reference);
    }

    #[test]
    fn each_band_composites_only_the_sprites_that_reach_it() {
        // A tall, narrow image with every marker in one place: one or two of its 2,048 bands
        // hold sprites, and only those visit the queue.
        let count = 2048;
        let mut scene = Scene::new(17.0, 32_767.0, WHITE);
        scene.push(Item::Markers {
            shape: rect_path(-0.5, -0.5, 1.0, 1.0),
            fill: Some(BLACK),
            stroke: None,
            positions: vec![pt(8.0, 16_384.0); count],
        });
        let render = |threads| {
            on_threads(threads, || {
                VISITS.with(|v| v.set(0));
                let png = render_png(&scene, 72).unwrap();
                (png, VISITS.with(std::cell::Cell::get))
            })
        };
        let (single, visits) = render(1);
        assert_eq!(visits, count);
        for threads in [2, 3, 8] {
            let (png, visits) = render(threads);
            assert!(png == single, "{threads} threads change the PNG");
            assert!(
                (count..=2 * count).contains(&visits),
                "{threads} threads visit {visits} placements"
            );
        }
    }

    #[test]
    fn png_bytes_do_not_depend_on_the_number_of_compositing_threads() {
        // Translucent markers inside a fractional clip, many of them on its edges and drawn as
        // paths between batches of sprites.
        let mut markers = markers_scene(3000, false);
        if let Some(Item::Markers { fill, .. }) = markers.items.first_mut() {
            *fill = Some(Rgba {
                r: 40,
                g: 120,
                b: 190,
                a: 150,
            });
        }
        let mut scene = Scene::new(120.0, 80.0, WHITE);
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 20.3,
                y: 15.7,
                width: 60.0,
                height: 30.0,
            }),
            items: std::mem::take(&mut markers.items),
        });
        let single = on_threads(1, || render_png(&scene, 150).unwrap());
        for threads in THREAD_COUNTS {
            let png = on_threads(threads, || render_png(&scene, 150).unwrap());
            assert!(png == single, "{threads} threads change the PNG");
        }
    }

    #[test]
    fn oversized_markers_fall_back_to_paths() {
        let mut scene = Scene::new(400.0, 400.0, WHITE);
        scene.push(Item::Markers {
            shape: rect_path(-150.0, -150.0, 300.0, 300.0),
            fill: Some(BLACK),
            stroke: None,
            positions: vec![pt(200.0, 200.0)],
        });
        let image = raster(&scene, 72);
        assert_eq!(image.at(200, 200), [0, 0, 0, 255]);
        assert_eq!(image.at(10, 10), [255, 255, 255, 255]);
    }

    // The shapes the property tests draw.
    use proptest::prelude::*;

    fn coord() -> impl Strategy<Value = f32> {
        prop_oneof![
            3 => -100.0f32..100.0,
            2 => -crate::render::normalize::MAX_COORD..=crate::render::normalize::MAX_COORD,
            1 => Just(crate::render::normalize::MAX_COORD),
            1 => Just(-crate::render::normalize::MAX_COORD),
        ]
    }

    fn point() -> impl Strategy<Value = Point> {
        (coord(), coord()).prop_map(|(x, y)| pt(x, y))
    }

    fn arb_path() -> impl Strategy<Value = Path> {
        let verb = prop_oneof![
            point().prop_map(|p| ('M', vec![p])),
            point().prop_map(|p| ('L', vec![p])),
            (point(), point()).prop_map(|(a, b)| ('Q', vec![a, b])),
            (point(), point(), point()).prop_map(|(a, b, c)| ('C', vec![a, b, c])),
            Just(('Z', vec![])),
        ];
        proptest::collection::vec(verb, 1..8).prop_map(|verbs| {
            let mut p = Path::new();
            for (v, c) in verbs {
                match (v, c.as_slice()) {
                    ('M', [a]) => p.move_to(*a),
                    ('L', [a]) => p.line_to(*a),
                    ('Q', [a, b]) => p.quad_to(*a, *b),
                    ('C', [a, b, c]) => p.cubic_to(*a, *b, *c),
                    _ => p.close(),
                }
            }
            p
        })
    }

    fn arb_dash() -> impl Strategy<Value = Vec<f32>> {
        proptest::collection::vec(
            prop_oneof![0.0f32..20.0, 0.0f32..0.5, Just(0.0), 0.0f32..1.0e4],
            0..5,
        )
    }

    fn arb_gradient() -> impl Strategy<Value = Paint> {
        let stop = (-0.5f32..1.5, any::<[u8; 4]>()).prop_map(|(offset, c)| GradientStop {
            offset,
            color: Rgba {
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
            },
        });
        (point(), point(), proptest::collection::vec(stop, 0..4))
            .prop_map(|(start, end, stops)| Paint::Linear(LinearGradient { start, end, stops }))
    }

    fn cases() -> u32 {
        std::env::var("UMBER_PROPTEST_CASES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256)
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: cases(), ..ProptestConfig::default() })]

        /// tiny-skia never panics on, or refuses, anything the shared layer accepts.
        #[test]
        fn accepted_geometry_never_panics_or_gets_refused(
            path in arb_path(),
            dash in arb_dash(),
            gradient in arb_gradient(),
            width in 0.001f32..=1000.0,
            dpi in prop_oneof![Just(10u32), Just(72), Just(300), Just(2400)],
            cap in 0u8..3,
            join in 0u8..3,
            marker_at in point(),
        ) {
            let mut scene = Scene::new(60.0, 40.0, WHITE);
            let style = StrokeStyle {
                color: BLACK,
                width,
                dash,
                cap: [LineCap::Butt, LineCap::Round, LineCap::Square][cap as usize],
                join: [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel][join as usize],
            };
            scene.push(Item::Fill { path: path.clone(), paint: Paint::Solid(BLACK), rule: FillRule::NonZero });
            scene.push(Item::Fill { path: path.clone(), paint: gradient, rule: FillRule::EvenOdd });
            scene.push(Item::Stroke { path: path.clone(), style: style.clone() });
            scene.push(Item::Markers {
                shape: path.clone(),
                fill: Some(BLACK),
                stroke: Some(style),
                positions: vec![marker_at, pt(30.0, 20.0)],
            });
            scene.push(Item::Group {
                clip: Some(Rect { x: 5.0, y: 5.0, width: 30.0, height: 20.0 }),
                items: vec![Item::Stroke { path, style: stroke_style(width, vec![1.0, 2.0]) }],
            });
            let before = REFUSED.with(std::cell::Cell::get);
            prop_assert!(rasterize(&scene, dpi).is_ok());
            prop_assert_eq!(REFUSED.with(std::cell::Cell::get), before);
        }
    }

    #[test]
    #[ignore = "release benchmark: cargo test -p umber-core --release -- --ignored --nocapture marker_benchmark"]
    fn marker_benchmark() {
        for count in [100_000usize, 1_000_000] {
            for radius in [2.0f32, 4.0] {
                let mut scene = Scene::new(432.0, 288.0, WHITE); // 6 x 4 in
                let mut state: u32 = 1;
                let mut next = move || {
                    state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    ((state >> 8) & 0xffff) as f32 / 65535.0
                };
                let positions = (0..count)
                    .map(|_| pt(next() * 432.0, next() * 288.0))
                    .collect();
                let k = radius * 0.552_284_8;
                let mut shape = Path::new();
                shape.move_to(pt(radius, 0.0));
                shape.cubic_to(pt(radius, k), pt(k, radius), pt(0.0, radius));
                shape.cubic_to(pt(-k, radius), pt(-radius, k), pt(-radius, 0.0));
                shape.cubic_to(pt(-radius, -k), pt(-k, -radius), pt(0.0, -radius));
                shape.cubic_to(pt(k, -radius), pt(radius, -k), pt(radius, 0.0));
                shape.close();
                scene.push(Item::Markers {
                    shape,
                    fill: Some(Rgba {
                        r: 40,
                        g: 120,
                        b: 190,
                        a: 255,
                    }),
                    stroke: Some(StrokeStyle {
                        color: WHITE,
                        ..stroke_style(0.8, vec![])
                    }),
                    positions,
                });
                let mut times = Vec::new();
                for round in 0..4 {
                    let start = std::time::Instant::now();
                    let pixmap = rasterize(&scene, 200).unwrap();
                    let elapsed = start.elapsed();
                    drop(pixmap);
                    if round > 0 {
                        times.push(elapsed);
                    }
                }
                times.sort();
                println!(
                    "markers={count} radius={radius}pt: median {:?} (rasterize only)",
                    times[1]
                );
                let start = std::time::Instant::now();
                let bytes = render_png(&scene, 200).unwrap();
                println!(
                    "  render_png incl. encode: {:?}, {} bytes",
                    start.elapsed(),
                    bytes.len()
                );
            }
        }
    }

    #[test]
    #[ignore = "memory probe: run alone under /usr/bin/time -l"]
    fn rss_probe() {
        // 8192 x 8192 px is exactly the 2^26 pixel cap at 72 dpi.
        let mut scene = Scene::new(8192.0, 8192.0, WHITE);
        let clip = |x: f32| {
            Some(Rect {
                x,
                y: 10.3,
                width: 8000.0,
                height: 8000.0,
            })
        };
        let inner = Item::Group {
            clip: clip(20.7),
            items: vec![fill(rect_path(0.0, 0.0, 8192.0, 8192.0), BLACK)],
        };
        scene.push(Item::Group {
            clip: clip(10.1),
            items: vec![inner],
        });
        let bytes = render_png(&scene, 72).unwrap();
        println!("rss_probe: {} bytes", bytes.len());
    }
}
