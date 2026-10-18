//! The page content and the resources it uses.
//!
//! [`Page`] writes the content stream in the scene's coordinates: the stream starts by turning
//! the page's y axis down, so points are written as the scene holds them. Each item is painted
//! inside its own `q`/`Q`, under the one clip in effect for it, which is opened and closed as
//! the clip changes (clips of nested groups are intersected first, so the state nesting never
//! goes deeper than two). The resources the content names (opacity states, soft masks,
//! shadings, marker forms, images and fonts) are recorded in first-use order and written by
//! [`Resources::write`].

use std::collections::HashMap;

use pdf_writer::types::{FunctionShadingType, LineCapStyle, LineJoinStyle, MaskType};
use pdf_writer::{Chunk, Content, Dict, Finish, Name, Rect as PdfRect, Ref};

use super::super::normalize::{
    GradientKind, NormalizedPath, Segment, image_is_drawable, normalize_dash, normalize_gradient,
    normalize_path, stroke_is_visible, text_is_visible, valid_point,
};
use super::Clip;
use super::image::{side, write_image};
use super::text::Fonts;
use super::write::{Refs, rounded, unit};
use crate::color::Rgba;
use crate::geometry::{Path, Point};
use crate::scene::{
    FillRule, GradientStop, Item, LineCap, LineJoin, Paint, Rect, StrokeStyle, TextItem,
};

/// The miter limit of every stroke, as in the PNG and SVG backends.
const MITER_LIMIT: f32 = 4.0;

/// The page content being written, with the resources it uses.
pub(super) struct Page<'f, 's> {
    content: Content,
    /// The clip whose `q` is open, if any.
    clip: Option<Clip>,
    resources: Resources<'s>,
    fonts: &'f Fonts<'s>,
}

impl<'f, 's> Page<'f, 's> {
    /// An empty page of `width × height` points whose text is set in `fonts`.
    pub(super) fn new(width: f32, height: f32, fonts: &'f Fonts<'s>) -> Page<'f, 's> {
        let mut content = Content::new();
        content.transform([1.0, 0.0, 0.0, -1.0, 0.0, height]);
        content.set_miter_limit(MITER_LIMIT);
        Page {
            content,
            clip: None,
            resources: Resources {
                width,
                height,
                states: Vec::new(),
                opacities: HashMap::new(),
                shadings: Vec::new(),
                xobjects: Vec::new(),
            },
            fonts,
        }
    }

    /// Paints the whole page in `color`, unless it is fully transparent.
    pub(super) fn background(&mut self, color: Rgba) {
        if color.a == 0 {
            return;
        }
        let (width, height) = (self.resources.width, self.resources.height);
        self.content.save_state();
        self.opacity(color.a, 255);
        fill_color(&mut self.content, color);
        self.content.rect(0.0, 0.0, width, height).fill_nonzero();
        self.content.restore_state();
    }

    /// Paints one leaf item under `clip`. Groups are resolved by the caller, which passes
    /// their leaves with the clip in effect for each.
    pub(super) fn item(&mut self, item: &'s Item, clip: Option<Clip>) {
        self.use_clip(clip);
        match item {
            Item::Fill { path, paint, rule } => self.fill(path, paint, *rule),
            Item::Stroke { path, style } => self.stroke(path, style),
            Item::Markers {
                shape,
                fill,
                stroke,
                positions,
            } => self.markers(shape, *fill, stroke.as_ref(), positions),
            Item::Text(text) => self.text(text),
            Item::Image {
                rect,
                width,
                height,
                pixels,
            } => self.image(rect, (*width, *height), pixels),
            Item::Group { .. } => {}
        }
    }

    /// The content stream and the resources it names.
    pub(super) fn finish(mut self) -> (Vec<u8>, Resources<'s>) {
        self.use_clip(None);
        (self.content.finish().into_vec(), self.resources)
    }

    /// Makes `clip` the clip in effect: closes the open clip's state and opens one for `clip`,
    /// unless it is already in effect.
    fn use_clip(&mut self, clip: Option<Clip>) {
        if self.clip == clip {
            return;
        }
        if self.clip.is_some() {
            self.content.restore_state();
        }
        if let Some(c) = clip {
            self.content.save_state();
            self.content
                .rect(
                    rounded(c.left),
                    rounded(c.top),
                    rounded(c.right - c.left),
                    rounded(c.bottom - c.top),
                )
                .clip_nonzero()
                .end_path();
        }
        self.clip = clip;
    }

    /// Sets the fill and stroke opacity, unless both are opaque.
    fn opacity(&mut self, fill: u8, stroke: u8) {
        if (fill, stroke) == (255, 255) {
            return;
        }
        let name = self.resources.opacity(fill, stroke);
        self.content.set_parameters(Name(name.as_bytes()));
    }

    fn fill(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        let Some(normalized) = normalize_path(path) else {
            return;
        };
        let gradient = match paint {
            Paint::Solid(c) => GradientKind::Solid(*c),
            Paint::Linear(g) => normalize_gradient(g),
            Paint::Sampled { cmap, from, to } => {
                normalize_gradient(&crate::LinearGradient::sampled(cmap, *from, *to))
            }
        };
        match gradient {
            GradientKind::Skip => {}
            GradientKind::Solid(c) => {
                if c.a == 0 {
                    return;
                }
                self.content.save_state();
                self.opacity(c.a, 255);
                fill_color(&mut self.content, c);
                write_path(&mut self.content, &normalized);
                match rule {
                    FillRule::NonZero => self.content.fill_nonzero(),
                    FillRule::EvenOdd => self.content.fill_even_odd(),
                };
                self.content.restore_state();
            }
            GradientKind::Linear { start, end, stops } => {
                self.content.save_state();
                write_path(&mut self.content, &normalized);
                match rule {
                    FillRule::NonZero => self.content.clip_nonzero(),
                    FillRule::EvenOdd => self.content.clip_even_odd(),
                };
                self.content.end_path();
                self.gradient_opacity(start, end, &stops);
                let colors = stops
                    .iter()
                    .map(|s| ShadingStop {
                        offset: s.offset,
                        components: vec![unit(s.color.r), unit(s.color.g), unit(s.color.b)],
                    })
                    .collect();
                let name = self.resources.shading(Shading {
                    start,
                    end,
                    space: Space::Rgb,
                    stops: colors,
                });
                self.content.shading(Name(name.as_bytes()));
                self.content.restore_state();
            }
        }
    }

    /// Sets the opacity of a gradient: none when every stop is opaque, one fill opacity when
    /// they share one, and otherwise a soft mask that shades the stops' opacities along the
    /// same line.
    fn gradient_opacity(&mut self, start: Point, end: Point, stops: &[GradientStop]) {
        let first = stops.first().map_or(255, |s| s.color.a);
        if stops.iter().all(|s| s.color.a == first) {
            self.opacity(first, 255);
            return;
        }
        let alphas = stops
            .iter()
            .map(|s| ShadingStop {
                offset: s.offset,
                components: vec![unit(s.color.a)],
            })
            .collect();
        let name = self.resources.soft_mask(Shading {
            start,
            end,
            space: Space::Gray,
            stops: alphas,
        });
        self.content.set_parameters(Name(name.as_bytes()));
    }

    fn stroke(&mut self, path: &Path, style: &StrokeStyle) {
        let Some(normalized) = normalize_path(path) else {
            return;
        };
        if !stroke_is_visible(style) {
            return;
        }
        self.content.save_state();
        self.opacity(255, style.color.a);
        stroke_style(&mut self.content, style, normalized.control_length());
        write_path(&mut self.content, &normalized);
        self.content.stroke();
        self.content.restore_state();
    }

    /// Draws one marker form per valid position: the shape filled, then stroked, as each copy
    /// is in the other backends.
    fn markers(
        &mut self,
        shape: &Path,
        fill: Option<Rgba>,
        stroke: Option<&StrokeStyle>,
        positions: &[Point],
    ) {
        let Some(normalized) = normalize_path(shape) else {
            return;
        };
        let fill = fill.filter(|c| c.a > 0);
        let stroke = stroke.filter(|s| stroke_is_visible(s));
        if fill.is_none() && stroke.is_none() {
            return;
        }
        if !positions.iter().any(|p| valid_point(*p)) {
            return;
        }
        let mut form = Content::new();
        if fill.is_some() {
            write_path(&mut form, &normalized);
            form.fill_nonzero();
        }
        if stroke.is_some() {
            write_path(&mut form, &normalized);
            form.stroke();
        }
        // The form clips to its box, so the box reaches past every miter of the stroke.
        let reach = stroke.map_or(0.0, |s| s.width / 2.0 * MITER_LIMIT) + 1.0;
        let bbox = bounds(&normalized).map(|[left, top, right, bottom]| {
            [left - reach, top - reach, right + reach, bottom + reach].map(rounded)
        });
        let Some(bbox) = bbox else {
            return;
        };
        let name = self.resources.xobject(XObject::Marker {
            content: form.finish().into_vec(),
            bbox,
        });

        self.content.save_state();
        self.opacity(fill.map_or(255, |c| c.a), stroke.map_or(255, |s| s.color.a));
        if let Some(c) = fill {
            fill_color(&mut self.content, c);
        }
        if let Some(s) = stroke {
            stroke_style(&mut self.content, s, normalized.control_length());
        }
        for p in positions.iter().filter(|p| valid_point(**p)) {
            self.content.save_state();
            self.content
                .transform([1.0, 0.0, 0.0, 1.0, rounded(p.x), rounded(p.y)]);
            self.content.x_object(Name(name.as_bytes()));
            self.content.restore_state();
        }
        self.content.restore_state();
    }

    /// Writes `item` as text when its fonts were embedded, and otherwise fills its outline.
    fn text(&mut self, item: &TextItem) {
        if !text_is_visible(item.color, item.origin) {
            return;
        }
        if self.fonts.can_show(item) {
            self.content.save_state();
            self.opacity(item.color.a, 255);
            fill_color(&mut self.content, item.color);
            // Text space: the origin at the left end of the baseline, the y axis up.
            self.content.transform([
                1.0,
                0.0,
                0.0,
                -1.0,
                rounded(item.origin.x),
                rounded(item.origin.y),
            ]);
            self.fonts.show(&mut self.content, &item.glyphs);
            self.content.restore_state();
        } else if let Some(normalized) = normalize_path(&item.outline) {
            self.content.save_state();
            self.opacity(item.color.a, 255);
            fill_color(&mut self.content, item.color);
            write_path(&mut self.content, &normalized);
            self.content.fill_nonzero();
            self.content.restore_state();
        }
    }

    fn image(&mut self, rect: &Rect, (width, height): (u32, u32), pixels: &'s [Rgba]) {
        if !image_is_drawable(rect, width, height, pixels) || pixels.iter().all(|c| c.a == 0) {
            return;
        }
        let (Some(columns), Some(rows)) = (side(width), side(height)) else {
            return;
        };
        let name = self.resources.xobject(XObject::Image {
            size: (columns, rows),
            pixels,
        });
        self.content.save_state();
        // The image's unit square, its first row at the top of `rect`.
        self.content.transform([
            rounded(rect.width),
            0.0,
            0.0,
            -rounded(rect.height),
            rounded(rect.x),
            rounded(rect.y + rect.height),
        ]);
        self.content.x_object(Name(name.as_bytes()));
        self.content.restore_state();
    }
}

/// Sets the fill color to the color channels of `c`; its opacity is set separately.
fn fill_color(content: &mut Content, c: Rgba) {
    content.set_fill_rgb(unit(c.r), unit(c.g), unit(c.b));
}

/// Sets the stroke color, width, cap, join and dash of `style`. `path_len` bounds the stroked
/// path's length, which decides whether the dash is drawn (see [`normalize_dash`]).
fn stroke_style(content: &mut Content, style: &StrokeStyle, path_len: f32) {
    content.set_stroke_rgb(
        unit(style.color.r),
        unit(style.color.g),
        unit(style.color.b),
    );
    content.set_line_width(rounded(style.width));
    content.set_line_cap(match style.cap {
        LineCap::Butt => LineCapStyle::ButtCap,
        LineCap::Round => LineCapStyle::RoundCap,
        LineCap::Square => LineCapStyle::ProjectingSquareCap,
    });
    content.set_line_join(match style.join {
        LineJoin::Miter => LineJoinStyle::MiterJoin,
        LineJoin::Round => LineJoinStyle::RoundJoin,
        LineJoin::Bevel => LineJoinStyle::BevelJoin,
    });
    if let Some(dash) = normalize_dash(&style.dash, path_len) {
        content.set_dash_pattern(dash.into_iter().map(rounded), 0.0);
    }
}

/// Writes the subpaths of `path`; quadratic segments become the equal cubic ones.
fn write_path(content: &mut Content, path: &NormalizedPath) {
    for sub in path.subpaths() {
        let start = sub.start();
        content.move_to(rounded(start.x), rounded(start.y));
        let mut at = start;
        for segment in sub.segments() {
            match *segment {
                Segment::Line(p) => {
                    content.line_to(rounded(p.x), rounded(p.y));
                    at = p;
                }
                Segment::Quad(c, p) => {
                    let (c1, c2) = quad_controls(at, c, p);
                    content.cubic_to(
                        rounded(c1.x),
                        rounded(c1.y),
                        rounded(c2.x),
                        rounded(c2.y),
                        rounded(p.x),
                        rounded(p.y),
                    );
                    at = p;
                }
                Segment::Cubic(c1, c2, p) => {
                    content.cubic_to(
                        rounded(c1.x),
                        rounded(c1.y),
                        rounded(c2.x),
                        rounded(c2.y),
                        rounded(p.x),
                        rounded(p.y),
                    );
                    at = p;
                }
            }
        }
        if sub.is_closed() {
            content.close_path();
        }
    }
}

/// The two cubic control points of the quadratic curve from `from` through control `c` to
/// `to`.
fn quad_controls(from: Point, c: Point, to: Point) -> (Point, Point) {
    let toward = |a: Point| Point {
        x: a.x + (c.x - a.x) * 2.0 / 3.0,
        y: a.y + (c.y - a.y) * 2.0 / 3.0,
    };
    (toward(from), toward(to))
}

/// The box `[left, top, right, bottom]` of every point of `path`, control points included.
fn bounds(path: &NormalizedPath) -> Option<[f32; 4]> {
    let mut points = path.subpaths().iter().flat_map(|sub| {
        std::iter::once(sub.start()).chain(sub.segments().iter().flat_map(
            |segment| match *segment {
                Segment::Line(p) => vec![p],
                Segment::Quad(c, p) => vec![c, p],
                Segment::Cubic(c1, c2, p) => vec![c1, c2, p],
            },
        ))
    });
    let first = points.next()?;
    Some(
        points.fold([first.x, first.y, first.x, first.y], |[l, t, r, b], p| {
            [l.min(p.x), t.min(p.y), r.max(p.x), b.max(p.y)]
        }),
    )
}

/// The color space of a shading.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Space {
    Rgb,
    Gray,
}

/// An axial shading from `start` to `end`, extended past both ends, through its stops in
/// increasing offset order.
#[derive(Debug, Clone, PartialEq)]
struct Shading {
    start: Point,
    end: Point,
    space: Space,
    stops: Vec<ShadingStop>,
}

/// A stop of a shading: its offset in `0..=1` and the color components there.
#[derive(Debug, Clone, PartialEq)]
struct ShadingStop {
    offset: f32,
    components: Vec<f32>,
}

/// An interpolation over one interval of a shading, between the components at its ends.
#[derive(Debug, Clone, PartialEq)]
struct Interval {
    from: Vec<f32>,
    to: Vec<f32>,
}

/// The intervals of a shading function over `0..=1`, and the offsets where they meet.
#[derive(Debug, Clone, PartialEq)]
struct Stitching {
    intervals: Vec<Interval>,
    bounds: Vec<f32>,
}

/// A graphics state the content sets by name.
#[derive(Debug, Clone, PartialEq)]
enum State {
    /// Fill and stroke opacity, each in `0..=255`.
    Opacity { fill: u8, stroke: u8 },
    /// A luminosity soft mask that paints a grayscale shading of opacities over the page.
    SoftMask(Shading),
}

/// An XObject the content draws by name.
#[derive(Debug)]
enum XObject<'s> {
    /// A marker shape as a form: its content and its box, `[left, top, right, bottom]`.
    Marker { content: Vec<u8>, bbox: [f32; 4] },
    /// An image of `size` (columns, rows) of `pixels`.
    Image {
        size: (i32, i32),
        pixels: &'s [Rgba],
    },
}

/// The resources of a page, in first-use order within each kind.
#[derive(Debug)]
pub(super) struct Resources<'s> {
    /// The page size in points, the box of soft masks.
    width: f32,
    height: f32,
    states: Vec<State>,
    /// The index in `states` of each opacity pair used.
    opacities: HashMap<(u8, u8), usize>,
    shadings: Vec<Shading>,
    xobjects: Vec<XObject<'s>>,
}

impl<'s> Resources<'s> {
    /// The name of the state with fill opacity `fill` and stroke opacity `stroke`.
    fn opacity(&mut self, fill: u8, stroke: u8) -> String {
        let states = &mut self.states;
        let index = *self.opacities.entry((fill, stroke)).or_insert_with(|| {
            states.push(State::Opacity { fill, stroke });
            states.len() - 1
        });
        state_name(index)
    }

    /// The name of a new state with the soft mask of `alphas`.
    fn soft_mask(&mut self, alphas: Shading) -> String {
        self.states.push(State::SoftMask(alphas));
        state_name(self.states.len() - 1)
    }

    /// The name of a new shading.
    fn shading(&mut self, shading: Shading) -> String {
        self.shadings.push(shading);
        format!("S{}", self.shadings.len() - 1)
    }

    /// The name of a new XObject.
    fn xobject(&mut self, xobject: XObject<'s>) -> String {
        self.xobjects.push(xobject);
        format!("X{}", self.xobjects.len() - 1)
    }

    /// Writes every resource and the page's `fonts` into `chunk`, numbering objects with
    /// `refs`, and returns the names the content uses for them.
    pub(super) fn write(&self, chunk: &mut Chunk, refs: &mut Refs, fonts: &Fonts<'_>) -> Names {
        let states = self
            .states
            .iter()
            .enumerate()
            .map(|(index, state)| (state_name(index), self.write_state(chunk, refs, state)))
            .collect();
        let shadings = self
            .shadings
            .iter()
            .enumerate()
            .map(|(index, shading)| {
                let id = refs.next();
                write_shading(chunk, id, shading);
                (format!("S{index}"), id)
            })
            .collect();
        let xobjects = self
            .xobjects
            .iter()
            .enumerate()
            .map(|(index, xobject)| {
                let id = match xobject {
                    XObject::Marker { content, bbox } => {
                        let id = refs.next();
                        let mut form = chunk.form_xobject(id, content);
                        form.bbox(PdfRect::new(bbox[0], bbox[1], bbox[2], bbox[3]));
                        form.resources();
                        form.finish();
                        id
                    }
                    XObject::Image { size, pixels } => write_image(chunk, refs, *size, pixels),
                };
                (format!("X{index}"), id)
            })
            .collect();
        Names {
            states,
            shadings,
            xobjects,
            fonts: fonts.write(chunk, refs),
        }
    }

    /// Writes `state` and what it refers to, and returns its object.
    fn write_state(&self, chunk: &mut Chunk, refs: &mut Refs, state: &State) -> Ref {
        let id = refs.next();
        match state {
            State::Opacity { fill, stroke } => {
                chunk
                    .ext_graphics(id)
                    .non_stroking_alpha(unit(*fill))
                    .stroking_alpha(unit(*stroke));
            }
            State::SoftMask(alphas) => {
                let (group, shading) = (refs.next(), refs.next());
                chunk
                    .ext_graphics(id)
                    .soft_mask()
                    .subtype(MaskType::Luminosity)
                    .group(group);
                let mut content = Content::new();
                content.shading(Name(b"S0"));
                let content = content.finish().into_vec();
                let mut form = chunk.form_xobject(group, &content);
                form.bbox(PdfRect::new(0.0, 0.0, self.width, self.height));
                form.group().transparency().color_space().device_gray();
                form.resources().shadings().pair(Name(b"S0"), shading);
                form.finish();
                write_shading(chunk, shading, alphas);
            }
        }
        id
    }
}

/// The resource name of state `index`.
fn state_name(index: usize) -> String {
    format!("G{index}")
}

/// The names the page content uses for its resources, with their objects.
#[derive(Debug)]
pub(super) struct Names {
    states: Vec<(String, Ref)>,
    shadings: Vec<(String, Ref)>,
    xobjects: Vec<(String, Ref)>,
    fonts: Vec<(String, Ref)>,
}

impl Names {
    /// Writes the names into a resource dictionary, leaving out kinds the page does not use.
    pub(super) fn write_into(&self, resources: &mut pdf_writer::writers::Resources<'_>) {
        let entries = |dict: &mut Dict<'_>, names: &[(String, Ref)]| {
            for (name, id) in names {
                dict.pair(Name(name.as_bytes()), *id);
            }
        };
        if !self.states.is_empty() {
            entries(&mut resources.ext_g_states(), &self.states);
        }
        if !self.shadings.is_empty() {
            entries(&mut resources.shadings(), &self.shadings);
        }
        if !self.xobjects.is_empty() {
            entries(&mut resources.x_objects(), &self.xobjects);
        }
        if !self.fonts.is_empty() {
            entries(&mut resources.fonts(), &self.fonts);
        }
    }
}

/// Writes `shading` as an axial shading dictionary with an inline function: one exponential
/// interpolation between two stops, or a stitching of one per interval between stops.
fn write_shading(chunk: &mut Chunk, id: Ref, shading: &Shading) {
    let mut dict = chunk.function_shading(id);
    dict.shading_type(FunctionShadingType::Axial);
    match shading.space {
        Space::Rgb => dict.color_space().device_rgb(),
        Space::Gray => dict.color_space().device_gray(),
    }
    dict.coords(
        [
            shading.start.x,
            shading.start.y,
            shading.end.x,
            shading.end.y,
        ]
        .map(rounded),
    );
    dict.extend([true, true]);
    let Stitching { intervals, bounds } = stitching(&shading.stops);
    let mut function = dict.insert(Name(b"Function")).dict();
    match intervals.as_slice() {
        [only] => interpolation(&mut function, only),
        _ => {
            function.pair(Name(b"FunctionType"), 3);
            function
                .insert(Name(b"Domain"))
                .array()
                .items([0.0_f32, 1.0]);
            let mut functions = function.insert(Name(b"Functions")).array();
            for interval in &intervals {
                interpolation(&mut functions.push().dict(), interval);
            }
            functions.finish();
            function.insert(Name(b"Bounds")).array().items(bounds);
            function
                .insert(Name(b"Encode"))
                .array()
                .items(intervals.iter().flat_map(|_| [0.0_f32, 1.0]));
        }
    }
    function.finish();
    dict.finish();
}

/// Writes `interval` as an exponential interpolation function over `0..=1`.
fn interpolation(dict: &mut Dict<'_>, interval: &Interval) {
    dict.pair(Name(b"FunctionType"), 2);
    dict.insert(Name(b"Domain")).array().items([0.0_f32, 1.0]);
    dict.insert(Name(b"C0"))
        .array()
        .items(interval.from.iter().copied());
    dict.insert(Name(b"C1"))
        .array()
        .items(interval.to.iter().copied());
    dict.pair(Name(b"N"), 1);
}

/// The intervals between consecutive stops that have any width, each from one stop's
/// components to the next one's, over the whole `0..=1`, and the offsets where they meet. The
/// first and last stops are repeated at 0 and 1 when they lie inside, so the shading keeps
/// their colors beyond them, and stops at one offset make a sharp step there. `stops` holds
/// at least two stops, in increasing offset order within `0..=1`.
fn stitching(stops: &[ShadingStop]) -> Stitching {
    let mut padded: Vec<(f32, &[f32])> = stops
        .iter()
        .map(|stop| (stop.offset, stop.components.as_slice()))
        .collect();
    if let Some(&(offset, components)) = padded.first()
        && offset > 0.0
    {
        padded.insert(0, (0.0, components));
    }
    if let Some(&(offset, components)) = padded.last()
        && offset < 1.0
    {
        padded.push((1.0, components));
    }
    let mut intervals = Vec::new();
    let mut bounds = Vec::new();
    for pair in padded.windows(2) {
        let [(from_offset, from), (to_offset, to)] = pair else {
            continue;
        };
        if to_offset <= from_offset {
            continue;
        }
        if !intervals.is_empty() {
            bounds.push(*from_offset);
        }
        intervals.push(Interval {
            from: from.to_vec(),
            to: to.to_vec(),
        });
    }
    Stitching { intervals, bounds }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stops(list: &[(f32, f32)]) -> Vec<ShadingStop> {
        list.iter()
            .map(|&(offset, v)| ShadingStop {
                offset,
                components: vec![v],
            })
            .collect()
    }

    /// The intervals of `stitching` as `(from, to)` pairs of one component.
    fn pairs(stitching: &Stitching) -> Vec<(f32, f32)> {
        stitching
            .intervals
            .iter()
            .map(|interval| (interval.from[0], interval.to[0]))
            .collect()
    }

    #[test]
    fn two_stops_over_the_whole_line_are_one_interpolation() {
        let result = stitching(&stops(&[(0.0, 0.2), (1.0, 0.8)]));
        assert_eq!(pairs(&result), [(0.2, 0.8)]);
        assert!(result.bounds.is_empty());
    }

    #[test]
    fn inner_stops_keep_their_colors_to_the_ends() {
        let result = stitching(&stops(&[(0.25, 0.2), (0.75, 0.8)]));
        assert_eq!(pairs(&result), [(0.2, 0.2), (0.2, 0.8), (0.8, 0.8)]);
        assert_eq!(result.bounds, [0.25, 0.75]);
    }

    #[test]
    fn stops_at_one_offset_make_a_step() {
        let result = stitching(&stops(&[(0.0, 0.0), (0.5, 0.0), (0.5, 1.0), (1.0, 1.0)]));
        assert_eq!(pairs(&result), [(0.0, 0.0), (1.0, 1.0)]);
        assert_eq!(result.bounds, [0.5]);
        // Every stop at one end still covers the whole line.
        let result = stitching(&stops(&[(1.0, 0.3), (1.0, 0.6)]));
        assert_eq!(pairs(&result), [(0.3, 0.3)]);
        assert!(result.bounds.is_empty());
    }

    #[test]
    fn quadratic_controls_lie_two_thirds_toward_the_control_point() {
        let (c1, c2) = quad_controls(
            Point { x: 0.0, y: 0.0 },
            Point { x: 3.0, y: 6.0 },
            Point { x: 6.0, y: 0.0 },
        );
        assert_eq!((c1.x, c1.y), (2.0, 4.0));
        assert_eq!((c2.x, c2.y), (4.0, 4.0));
    }

    #[test]
    fn bounds_include_control_points() {
        let mut path = Path::new();
        path.move_to(Point { x: 1.0, y: 1.0 });
        path.quad_to(Point { x: 5.0, y: -4.0 }, Point { x: 9.0, y: 1.0 });
        path.line_to(Point { x: 2.0, y: 3.0 });
        let normalized = normalize_path(&path).unwrap();
        assert_eq!(bounds(&normalized), Some([1.0, -4.0, 9.0, 3.0]));
    }
}
