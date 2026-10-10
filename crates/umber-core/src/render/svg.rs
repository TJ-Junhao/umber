//! The SVG backend.
//!
//! [`render_svg`] writes a [`Scene`] as an SVG document. The output is hand-written XML with
//! absolute path coordinates, at most three decimals per number, and ids drawn from a single
//! counter in document order, so the same scene always gives the same bytes. Geometry, dashes
//! and gradients go through the shared normalization in [`super::normalize`]; values that
//! cannot be drawn are skipped and the document stays well-formed.
//!
//! # Examples
//!
//! ```
//! use umber_core::render::{SvgText, render_svg};
//! use umber_core::{Rgba, Scene};
//!
//! let white = Rgba { r: 255, g: 255, b: 255, a: 255 };
//! let svg = render_svg(&Scene::new(100.0, 50.0, white), SvgText::Outlines);
//! assert!(svg.starts_with("<svg "));
//! ```

use std::fmt::Write as _;

use super::SvgText;
use super::normalize::{
    GradientKind, NormalizedPath, Segment, normalize_dash, normalize_gradient, normalize_path,
    valid_point,
};
use crate::color::Rgba;
use crate::fonts::Weight;
use crate::geometry::{Path, Point};
use crate::scene::{
    FillRule, Item, LineCap, LineJoin, MAX_GROUP_DEPTH, Paint, Rect, Scene, StrokeStyle, TextItem,
};

/// Writes `scene` as an SVG document.
///
/// With [`SvgText::Outlines`] text is drawn as filled glyph outlines. With
/// [`SvgText::Editable`] it is written as `<text>` elements that name the font family.
/// Everything that cannot be drawn is skipped, so the result always parses as XML. Content of
/// groups nested deeper than [`MAX_GROUP_DEPTH`](crate::scene::MAX_GROUP_DEPTH) is omitted.
///
/// # Examples
///
/// ```
/// use umber_core::render::{SvgText, render_svg};
/// use umber_core::{Rgba, Scene};
///
/// let white = Rgba { r: 255, g: 255, b: 255, a: 255 };
/// let svg = render_svg(&Scene::new(72.0, 36.0, white), SvgText::Editable);
/// assert!(svg.contains(r#"width="72pt" height="36pt" viewBox="0 0 72 36""#));
/// assert!(svg.contains(r##"fill="#FFFFFF""##));
/// ```
#[must_use]
pub fn render_svg(scene: &Scene, text: SvgText) -> String {
    let width = size_value(scene.width);
    let height = size_value(scene.height);
    let (w, h) = (num(width), num(height));

    let mut writer = Writer {
        defs: String::new(),
        body: String::new(),
        next_id: 0,
        text,
    };
    for item in &scene.items {
        writer.item(item, 0);
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}pt" height="{h}pt" viewBox="0 0 {w} {h}">"#
    );
    let _ = write!(
        out,
        r#"<rect width="{w}" height="{h}" fill="{}""#,
        hex(scene.background)
    );
    write_opacity(&mut out, "fill-opacity", scene.background.a);
    out.push_str("/>\n");
    if !writer.defs.is_empty() {
        out.push_str("<defs>\n");
        out.push_str(&writer.defs);
        out.push_str("</defs>\n");
    }
    out.push_str(&writer.body);
    out.push_str("</svg>\n");
    out
}

/// A non-finite or negative size becomes 0.
fn size_value(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 { v } else { 0.0 }
}

/// Formats a finite number with at most three decimals, trimmed, never `-0`.
fn num(v: f32) -> String {
    let mut s = format!("{v:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

/// Formats a dash entry with Rust's shortest round-trip form, so a nonzero entry never turns
/// into 0. `Display` for floats never uses an exponent.
fn dash_entry(v: f32) -> String {
    format!("{v}")
}

fn hex(c: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

/// Appends ` {name}="a/255"` when the alpha is below 255.
fn write_opacity(out: &mut String, name: &str, alpha: u8) {
    if alpha < 255 {
        let _ = write!(out, r#" {name}="{}""#, num(f32::from(alpha) / 255.0));
    }
}

/// Appends ` {attr}="#RRGGBB"` plus the opacity attribute when needed.
fn write_color(out: &mut String, attr: &str, opacity_attr: &str, c: Rgba) {
    let _ = write!(out, r#" {attr}="{}""#, hex(c));
    write_opacity(out, opacity_attr, c.a);
}

/// Escapes text for use in element content or attribute values, dropping characters XML
/// cannot carry.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' => out.push(c),
            '\r' => out.push_str("&#13;"),
            c if (c as u32) < 0x20 => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// Builds the `d` string of a normalized path.
fn path_d(path: &NormalizedPath) -> String {
    fn pt(d: &mut String, p: Point) {
        let _ = write!(d, "{} {}", num(p.x), num(p.y));
    }
    let mut d = String::new();
    for sub in &path.subpaths {
        if !d.is_empty() {
            d.push(' ');
        }
        d.push('M');
        pt(&mut d, sub.start);
        for seg in &sub.segments {
            match *seg {
                Segment::Line(p) => {
                    d.push_str(" L");
                    pt(&mut d, p);
                }
                Segment::Quad(c, p) => {
                    d.push_str(" Q");
                    pt(&mut d, c);
                    d.push(' ');
                    pt(&mut d, p);
                }
                Segment::Cubic(c1, c2, p) => {
                    d.push_str(" C");
                    pt(&mut d, c1);
                    d.push(' ');
                    pt(&mut d, c2);
                    d.push(' ');
                    pt(&mut d, p);
                }
            }
        }
        if sub.closed {
            d.push_str(" Z");
        }
    }
    d
}

/// Writes the stroke attributes and returns whether the stroke is drawable. `path_len` is an
/// upper bound on the stroked path's length, used to classify the dash.
fn write_stroke(out: &mut String, style: &StrokeStyle, path_len: f32) -> bool {
    if style.color.a == 0 || !style.width.is_finite() || style.width <= 0.0 {
        return false;
    }
    write_color(out, "stroke", "stroke-opacity", style.color);
    let _ = write!(out, r#" stroke-width="{}""#, num(style.width));
    match style.cap {
        LineCap::Butt => {}
        LineCap::Round => out.push_str(r#" stroke-linecap="round""#),
        LineCap::Square => out.push_str(r#" stroke-linecap="square""#),
    }
    match style.join {
        LineJoin::Miter => {}
        LineJoin::Round => out.push_str(r#" stroke-linejoin="round""#),
        LineJoin::Bevel => out.push_str(r#" stroke-linejoin="bevel""#),
    }
    if let Some(dash) = normalize_dash(&style.dash, path_len) {
        let list: Vec<String> = dash.iter().map(|v| dash_entry(*v)).collect();
        let _ = write!(out, r#" stroke-dasharray="{}""#, list.join(" "));
    }
    true
}

fn rect_is_valid(r: Rect) -> bool {
    [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite()) && r.width > 0.0 && r.height > 0.0
}

struct Writer {
    defs: String,
    body: String,
    next_id: usize,
    text: SvgText,
}

impl Writer {
    fn id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Writes one item. `depth` is the number of groups enclosing it.
    fn item(&mut self, item: &Item, depth: usize) {
        match item {
            Item::Fill { path, paint, rule } => self.fill(path, paint, *rule),
            Item::Stroke { path, style } => self.stroke(path, style),
            Item::Markers {
                shape,
                fill,
                stroke,
                positions,
            } => {
                self.markers(shape, *fill, stroke.as_ref(), positions);
            }
            Item::Text(t) => self.text(t),
            Item::Group { clip, items } => self.group(*clip, items, depth),
        }
    }

    fn fill(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        let Some(normalized) = normalize_path(path) else {
            return;
        };
        let d = path_d(&normalized);
        let mut attrs = String::new();
        match paint {
            Paint::Solid(c) => {
                if c.a == 0 {
                    return;
                }
                write_color(&mut attrs, "fill", "fill-opacity", *c);
            }
            Paint::Linear(g) => match normalize_gradient(g) {
                GradientKind::Skip => return,
                GradientKind::Solid(c) => {
                    if c.a == 0 {
                        return;
                    }
                    write_color(&mut attrs, "fill", "fill-opacity", c);
                }
                GradientKind::Linear { start, end, stops } => {
                    let id = self.id();
                    let _ = writeln!(
                        self.defs,
                        r#"<linearGradient id="g{id}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}">"#,
                        num(start.x),
                        num(start.y),
                        num(end.x),
                        num(end.y)
                    );
                    for stop in stops {
                        let mut line = format!(r#"<stop offset="{}""#, num(stop.offset));
                        write_color(&mut line, "stop-color", "stop-opacity", stop.color);
                        line.push_str("/>\n");
                        self.defs.push_str(&line);
                    }
                    self.defs.push_str("</linearGradient>\n");
                    let _ = write!(attrs, r#" fill="url(#g{id})""#);
                }
            },
        }
        if rule == FillRule::EvenOdd {
            attrs.push_str(r#" fill-rule="evenodd""#);
        }
        let _ = writeln!(self.body, r#"<path d="{d}"{attrs}/>"#);
    }

    fn stroke(&mut self, path: &Path, style: &StrokeStyle) {
        let Some(normalized) = normalize_path(path) else {
            return;
        };
        let mut attrs = String::from(r#" fill="none""#);
        if !write_stroke(&mut attrs, style, normalized.control_length()) {
            return;
        }
        let _ = writeln!(self.body, r#"<path d="{}"{attrs}/>"#, path_d(&normalized));
    }

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
        let mut attrs = String::new();
        let fill = fill.filter(|c| c.a > 0);
        match fill {
            Some(c) => write_color(&mut attrs, "fill", "fill-opacity", c),
            None => attrs.push_str(r#" fill="none""#),
        }
        let mut stroke_attrs = String::new();
        let has_stroke =
            stroke.is_some_and(|s| write_stroke(&mut stroke_attrs, s, normalized.control_length()));
        if has_stroke {
            attrs.push_str(&stroke_attrs);
        }
        if fill.is_none() && !has_stroke {
            return;
        }
        let uses: Vec<&Point> = positions.iter().filter(|p| valid_point(**p)).collect();
        if uses.is_empty() {
            return;
        }
        let id = self.id();
        let _ = writeln!(
            self.defs,
            r#"<path id="m{id}" d="{}"/>"#,
            path_d(&normalized)
        );
        let _ = writeln!(self.body, "<g{attrs}>");
        for p in uses {
            let _ = writeln!(
                self.body,
                "<use href=\"#m{id}\" x=\"{}\" y=\"{}\"/>",
                num(p.x),
                num(p.y)
            );
        }
        self.body.push_str("</g>\n");
    }

    fn text(&mut self, t: &TextItem) {
        if t.color.a == 0 {
            return;
        }
        match self.text {
            SvgText::Outlines => {
                let Some(normalized) = normalize_path(&t.outline) else {
                    return;
                };
                let mut attrs = String::new();
                write_color(&mut attrs, "fill", "fill-opacity", t.color);
                let _ = writeln!(self.body, r#"<path d="{}"{attrs}/>"#, path_d(&normalized));
            }
            SvgText::Editable => {
                let content = escape(&t.text);
                if content.is_empty() || !valid_point(t.origin) {
                    return;
                }
                if !t.style.size.is_finite() || t.style.size <= 0.0 {
                    return;
                }
                let weight = match t.style.weight {
                    Weight::Regular => 400,
                    Weight::SemiBold => 600,
                };
                let mut attrs = String::new();
                let _ = write!(
                    attrs,
                    r#" x="{}" y="{}" font-family="'{}', {}" font-size="{}" font-weight="{weight}""#,
                    num(t.origin.x),
                    num(t.origin.y),
                    escape(t.style.font.family_name()),
                    t.style.font.generic_family(),
                    num(t.style.size)
                );
                if t.style.tabular_figures {
                    attrs.push_str(r#" font-variant-numeric="tabular-nums""#);
                }
                write_color(&mut attrs, "fill", "fill-opacity", t.color);
                let _ = writeln!(
                    self.body,
                    r#"<text xml:space="preserve"{attrs}>{content}</text>"#
                );
            }
        }
    }

    fn group(&mut self, clip: Option<Rect>, items: &[Item], depth: usize) {
        if depth >= MAX_GROUP_DEPTH {
            return;
        }
        let mut attrs = String::new();
        let mut clip_def = None;
        if let Some(r) = clip {
            if !rect_is_valid(r) {
                return;
            }
            let id = self.id();
            let _ = write!(attrs, r#" clip-path="url(#c{id})""#);
            clip_def = Some(format!(
                "<clipPath id=\"c{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>\n",
                num(r.x),
                num(r.y),
                num(r.width),
                num(r.height)
            ));
        }
        let open = format!("<g{attrs}>\n");
        let start = self.body.len();
        let defs_start = self.defs.len();
        self.body.push_str(&open);
        for item in items {
            self.item(item, depth + 1);
        }
        if self.body.len() == start + open.len() {
            self.body.truncate(start);
            return;
        }
        self.body.push_str("</g>\n");
        if let Some(def) = clip_def {
            // The clip was numbered before its children, so it is defined before them.
            self.defs.insert_str(defs_start, &def);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::Font;
    use crate::scene::{GradientStop, LinearGradient};
    use crate::text::{FontLibrary, TextStyle};

    const WHITE: Rgba = Rgba {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    const RED: Rgba = Rgba {
        r: 200,
        g: 40,
        b: 20,
        a: 255,
    };
    const BLUE: Rgba = Rgba {
        r: 20,
        g: 60,
        b: 200,
        a: 128,
    };

    fn p(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn triangle() -> Path {
        let mut path = Path::new();
        path.move_to(p(10.0, 10.0));
        path.line_to(p(50.0, 10.0));
        path.line_to(p(30.0, 40.0));
        path.close();
        path
    }

    fn line() -> Path {
        let mut path = Path::new();
        path.move_to(p(0.0, 0.0));
        path.line_to(p(100.0, 20.0));
        path
    }

    fn stroke_style(width: f32, dash: Vec<f32>) -> StrokeStyle {
        StrokeStyle {
            color: RED,
            width,
            dash,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }
    }

    fn gradient(stops: Vec<GradientStop>, start: Point, end: Point) -> Paint {
        Paint::Linear(LinearGradient { start, end, stops })
    }

    fn two_stops() -> Vec<GradientStop> {
        vec![
            GradientStop {
                offset: 1.0,
                color: BLUE,
            },
            GradientStop {
                offset: 0.0,
                color: RED,
            },
        ]
    }

    fn inter(size: f32) -> TextStyle {
        TextStyle {
            font: Font::Inter,
            weight: Weight::Regular,
            size,
            tabular_figures: false,
        }
    }

    fn parse(svg: &str) -> roxmltree::Document<'_> {
        match roxmltree::Document::parse(svg) {
            Ok(doc) => doc,
            Err(e) => panic!("not well-formed: {e}\n{svg}"),
        }
    }

    fn snapshot_scene() -> Scene {
        let mut scene = Scene::new(120.0, 80.0, WHITE);
        scene.push(Item::Fill {
            path: triangle(),
            paint: Paint::Solid(RED),
            rule: FillRule::NonZero,
        });
        scene.push(Item::Stroke {
            path: line(),
            style: StrokeStyle {
                color: BLUE,
                width: 1.5,
                dash: vec![4.0, 2.0],
                cap: LineCap::Round,
                join: LineJoin::Bevel,
            },
        });
        scene.push(Item::Fill {
            path: triangle().translated(60.0, 0.0),
            paint: gradient(two_stops(), p(60.0, 0.0), p(120.0, 0.0)),
            rule: FillRule::EvenOdd,
        });
        let mut dot = Path::new();
        dot.move_to(p(-2.0, 0.0));
        dot.line_to(p(2.0, 0.0));
        dot.line_to(p(0.0, 3.0));
        dot.close();
        scene.push(Item::Markers {
            shape: dot,
            fill: Some(RED),
            stroke: Some(stroke_style(0.5, vec![])),
            positions: vec![p(20.0, 60.0), p(40.0, 62.5), p(60.0, 58.25)],
        });
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 70.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            }),
            items: vec![Item::Fill {
                path: triangle().translated(60.0, 30.0),
                paint: Paint::Solid(BLUE),
                rule: FillRule::NonZero,
            }],
        });
        scene
    }

    #[test]
    fn empty_scene_is_valid_svg() {
        let svg = render_svg(&Scene::new(0.0, 0.0, WHITE), SvgText::Outlines);
        let doc = parse(&svg);
        let root = doc.root_element();
        assert_eq!(root.attribute("width"), Some("0pt"));
        assert_eq!(root.attribute("height"), Some("0pt"));
    }

    #[test]
    fn fill_stroke_and_markers_snapshot() {
        let svg = render_svg(&snapshot_scene(), SvgText::Outlines);
        parse(&svg);
        insta::assert_snapshot!(svg);
    }

    #[test]
    fn text_outlines_vs_editable() {
        let mut scene = Scene::new(100.0, 40.0, WHITE);
        scene.push_text(
            FontLibrary::global(),
            "Hello",
            inter(12.0),
            p(5.0, 20.0),
            RED,
        );

        let outlines = render_svg(&scene, SvgText::Outlines);
        assert!(!outlines.contains("<text"));
        assert!(outlines.contains("<path"));
        parse(&outlines);

        let editable = render_svg(&scene, SvgText::Editable);
        let doc = parse(&editable);
        let texts: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("text"))
            .collect();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].text(), Some("Hello"));
        assert_eq!(
            texts[0].attribute("font-family"),
            Some("'Inter', sans-serif")
        );
        assert_eq!(texts[0].attribute("font-weight"), Some("400"));
    }

    #[test]
    fn svg_escapes_text_and_attributes() {
        let mut scene = Scene::new(100.0, 40.0, WHITE);
        let source = "a<b & \"c\" 'd'\u{0007}e";
        scene.push_text(
            FontLibrary::global(),
            source,
            inter(12.0),
            p(5.0, 20.0),
            RED,
        );
        let svg = render_svg(&scene, SvgText::Editable);
        let doc = parse(&svg);
        let texts: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("text"))
            .collect();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].text(), Some("a<b & \"c\" 'd'e"));
        assert!(texts[0].children().all(|c| c.is_text()));
        let elements = doc.descendants().filter(|n| n.is_element()).count();
        assert_eq!(elements, 3, "svg, background rect and one text: {svg}");
        assert_eq!(escape("\"<>&'"), "&quot;&lt;&gt;&amp;&apos;");
    }

    #[test]
    fn hostile_scenes_render_without_panicking() {
        let nan = f32::NAN;
        let inf = f32::INFINITY;
        let mut scene = Scene::new(100.0, 100.0, WHITE);

        // Non-finite points in every verb position.
        for bad in [p(nan, 1.0), p(1.0, inf), p(-inf, nan)] {
            let mut paths = Vec::new();
            for slot in 0..3 {
                let mut path = Path::new();
                path.move_to(if slot == 0 { bad } else { p(0.0, 0.0) });
                path.line_to(if slot == 1 { bad } else { p(10.0, 0.0) });
                path.quad_to(if slot == 2 { bad } else { p(15.0, 5.0) }, p(20.0, 10.0));
                path.cubic_to(p(1.0, 1.0), if slot == 2 { bad } else { p(2.0, 2.0) }, bad);
                path.close();
                paths.push(path);
            }
            let mut only_bad = Path::new();
            only_bad.move_to(bad);
            only_bad.line_to(bad);
            paths.push(only_bad);
            for path in paths {
                scene.push(Item::Fill {
                    path: path.clone(),
                    paint: Paint::Solid(RED),
                    rule: FillRule::NonZero,
                });
                scene.push(Item::Stroke {
                    path,
                    style: stroke_style(1.0, vec![]),
                });
            }
        }

        // Empty path.
        scene.push(Item::Fill {
            path: Path::new(),
            paint: Paint::Solid(RED),
            rule: FillRule::NonZero,
        });
        scene.push(Item::Stroke {
            path: Path::new(),
            style: stroke_style(1.0, vec![]),
        });

        // Odd stroke widths and dashes.
        for width in [0.0, -1.0, nan, inf] {
            scene.push(Item::Stroke {
                path: line(),
                style: stroke_style(width, vec![]),
            });
        }
        for dash in [vec![0.0, 0.0], vec![-1.0, 2.0], vec![nan], vec![inf, 1.0]] {
            scene.push(Item::Stroke {
                path: line(),
                style: stroke_style(1.0, dash),
            });
        }

        // Degenerate gradients.
        scene.push(Item::Fill {
            path: triangle(),
            paint: gradient(vec![], p(0.0, 0.0), p(1.0, 0.0)),
            rule: FillRule::NonZero,
        });
        scene.push(Item::Fill {
            path: triangle(),
            paint: gradient(
                vec![GradientStop {
                    offset: 0.5,
                    color: RED,
                }],
                p(0.0, 0.0),
                p(1.0, 0.0),
            ),
            rule: FillRule::NonZero,
        });
        scene.push(Item::Fill {
            path: triangle(),
            paint: gradient(two_stops(), p(3.0, 3.0), p(3.0, 3.0)),
            rule: FillRule::NonZero,
        });
        scene.push(Item::Fill {
            path: triangle(),
            paint: gradient(two_stops(), p(nan, 0.0), p(1.0, 0.0)),
            rule: FillRule::NonZero,
        });
        scene.push(Item::Fill {
            path: triangle(),
            paint: gradient(
                vec![
                    GradientStop {
                        offset: nan,
                        color: RED,
                    },
                    GradientStop {
                        offset: inf,
                        color: BLUE,
                    },
                    GradientStop {
                        offset: -5.0,
                        color: RED,
                    },
                ],
                p(0.0, 0.0),
                p(1.0, 0.0),
            ),
            rule: FillRule::NonZero,
        });

        // Clips, groups and markers.
        for clip in [
            Rect {
                x: 0.0,
                y: 0.0,
                width: -1.0,
                height: 5.0,
            },
            Rect {
                x: nan,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            },
            Rect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: inf,
            },
        ] {
            scene.push(Item::Group {
                clip: Some(clip),
                items: vec![Item::Fill {
                    path: triangle(),
                    paint: Paint::Solid(RED),
                    rule: FillRule::NonZero,
                }],
            });
        }
        scene.push(Item::Group {
            clip: None,
            items: vec![],
        });
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            }),
            items: vec![],
        });
        scene.push(Item::Markers {
            shape: triangle(),
            fill: Some(RED),
            stroke: None,
            positions: vec![p(nan, 1.0), p(1.0, inf)],
        });
        scene.push(Item::Markers {
            shape: Path::new(),
            fill: Some(RED),
            stroke: None,
            positions: vec![p(1.0, 1.0)],
        });
        scene.push(Item::Markers {
            shape: triangle(),
            fill: None,
            stroke: None,
            positions: vec![p(1.0, 1.0)],
        });

        // Text with hostile geometry.
        scene.push(Item::Text(TextItem {
            text: "x".to_string(),
            style: inter(nan),
            origin: p(nan, nan),
            color: RED,
            outline: Path::new(),
            width: nan,
        }));

        for mode in [SvgText::Outlines, SvgText::Editable] {
            let svg = render_svg(&scene, mode);
            parse(&svg);
            assert!(!svg.contains("NaN") && !svg.contains("inf"), "{svg}");
        }

        for (w, h) in [(nan, 10.0), (10.0, inf), (-5.0, 10.0)] {
            let svg = render_svg(&Scene::new(w, h, WHITE), SvgText::Outlines);
            parse(&svg);
            assert!(!svg.contains("NaN") && !svg.contains("inf"), "{svg}");
        }
    }

    #[test]
    fn dropped_move_to_never_leaves_a_leading_line() {
        let mut path = Path::new();
        path.move_to(p(f32::NAN, 0.0));
        path.line_to(p(1.0, 2.0));
        path.line_to(p(3.0, 4.0));
        let normalized = normalize_path(&path).expect("drawable");
        assert_eq!(path_d(&normalized), "M1 2 L3 4");
    }

    #[test]
    fn numbers_are_trimmed_and_never_negative_zero() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(0.1234), "0.123");
        assert_eq!(num(-0.0004), "0");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(-12.5), "-12.5");
        assert_eq!(num(100.0), "100");
    }

    #[test]
    fn svg_output_is_deterministic() {
        let scene = snapshot_scene();
        let first = render_svg(&scene, SvgText::Outlines);
        for _ in 0..10 {
            assert_eq!(
                render_svg(&scene, SvgText::Outlines).as_bytes(),
                first.as_bytes()
            );
        }
    }

    #[test]
    fn drops_fully_transparent_items() {
        let clear = Rgba {
            r: 1,
            g: 2,
            b: 3,
            a: 0,
        };
        let mut scene = Scene::new(50.0, 50.0, WHITE);
        scene.push(Item::Fill {
            path: triangle(),
            paint: Paint::Solid(clear),
            rule: FillRule::NonZero,
        });
        scene.push(Item::Stroke {
            path: line(),
            style: StrokeStyle {
                color: clear,
                ..stroke_style(1.0, vec![])
            },
        });
        scene.push(Item::Markers {
            shape: triangle(),
            fill: Some(clear),
            stroke: None,
            positions: vec![p(1.0, 1.0)],
        });
        scene.push_text(
            FontLibrary::global(),
            "Hi",
            inter(12.0),
            p(5.0, 20.0),
            clear,
        );
        for mode in [SvgText::Outlines, SvgText::Editable] {
            let svg = render_svg(&scene, mode);
            let doc = parse(&svg);
            assert_eq!(
                doc.descendants().filter(|n| n.is_element()).count(),
                2,
                "{svg}"
            );
        }
    }

    #[test]
    fn editable_text_preserves_spaces() {
        let mut scene = Scene::new(200.0, 40.0, WHITE);
        let source = "  A     B  ";
        scene.push_text(
            FontLibrary::global(),
            source,
            inter(12.0),
            p(5.0, 20.0),
            RED,
        );
        let svg = render_svg(&scene, SvgText::Editable);
        let doc = parse(&svg);
        let text = doc
            .descendants()
            .find(|n| n.has_tag_name("text"))
            .expect("text element");
        assert_eq!(
            text.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
            Some("preserve")
        );
        assert_eq!(text.text(), Some(source));
    }

    #[test]
    fn signed_zero_gradient_offsets_keep_input_order() {
        let green = Rgba {
            r: 0,
            g: 255,
            b: 0,
            a: 255,
        };
        let mut scene = Scene::new(50.0, 50.0, WHITE);
        scene.push(Item::Fill {
            path: triangle(),
            paint: gradient(
                vec![
                    GradientStop {
                        offset: 0.0,
                        color: RED,
                    },
                    GradientStop {
                        offset: -0.0,
                        color: green,
                    },
                    GradientStop {
                        offset: 1.0,
                        color: WHITE,
                    },
                ],
                p(0.0, 0.0),
                p(50.0, 0.0),
            ),
            rule: FillRule::NonZero,
        });
        let svg = render_svg(&scene, SvgText::Outlines);
        let doc = parse(&svg);
        let colors: Vec<&str> = doc
            .descendants()
            .filter(|n| n.has_tag_name("stop"))
            .filter_map(|n| n.attribute("stop-color"))
            .collect();
        assert_eq!(colors, ["#C82814", "#00FF00", "#FFFFFF"]);
        let offsets: Vec<&str> = doc
            .descendants()
            .filter(|n| n.has_tag_name("stop"))
            .filter_map(|n| n.attribute("offset"))
            .collect();
        assert_eq!(offsets, ["0", "0", "1"]);
    }

    #[test]
    fn clip_is_defined_before_its_children_definitions() {
        let mut scene = Scene::new(50.0, 50.0, WHITE);
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            }),
            items: vec![Item::Fill {
                path: triangle(),
                paint: gradient(two_stops(), p(0.0, 0.0), p(20.0, 0.0)),
                rule: FillRule::NonZero,
            }],
        });
        let svg = render_svg(&scene, SvgText::Outlines);
        let clip = svg.find("<clipPath id=\"c0\"").expect("clip");
        let grad = svg.find("<linearGradient id=\"g1\"").expect("gradient");
        assert!(clip < grad, "{svg}");
    }

    #[test]
    fn dashes_round_trip_and_follow_the_shared_classification() {
        let mut scene = Scene::new(50.0, 50.0, WHITE);
        scene.push(Item::Stroke {
            path: line(),
            style: stroke_style(1.0, vec![0.0625, 3.0]),
        });
        scene.push(Item::Stroke {
            path: line(),
            style: stroke_style(1.0, vec![3.0]),
        });
        scene.push(Item::Stroke {
            path: line(),
            style: stroke_style(1.0, vec![0.00049, 0.00049]),
        });
        let svg = render_svg(&scene, SvgText::Outlines);
        assert!(svg.contains(r#"stroke-dasharray="0.0625 3""#), "{svg}");
        assert_eq!(dash_entry(0.1), "0.1");
        assert_eq!(dash_entry(0.00004), "0.00004");
        for v in [1.0e-20_f32, 3.0e38, f32::MIN_POSITIVE, 123456.79] {
            let text = dash_entry(v);
            assert!(!text.contains(['e', 'E']), "{text}");
            assert_eq!(text.parse::<f32>().ok(), Some(v));
        }
        assert!(svg.contains(r#"stroke-dasharray="3 3""#), "{svg}");
        assert_eq!(svg.matches("stroke-dasharray").count(), 2, "{svg}");
    }

    fn deep_chain(levels: usize) -> Scene {
        let mut item = Item::Fill {
            path: triangle(),
            paint: Paint::Solid(RED),
            rule: FillRule::NonZero,
        };
        for _ in 0..levels {
            item = Item::Group {
                clip: None,
                items: vec![item],
            };
        }
        let mut scene = Scene::new(50.0, 50.0, WHITE);
        scene.push(item);
        scene
    }

    #[test]
    fn group_depth_is_bounded() {
        use crate::scene::MAX_GROUP_DEPTH;
        let kept = render_svg(&deep_chain(MAX_GROUP_DEPTH), SvgText::Outlines);
        assert!(kept.contains("<path d="));
        assert_eq!(kept.matches("<g>").count(), MAX_GROUP_DEPTH);
        let omitted = render_svg(&deep_chain(MAX_GROUP_DEPTH + 1), SvgText::Outlines);
        assert!(!omitted.contains("<path d="));
        assert!(!omitted.contains("<g"));
    }

    #[test]
    fn very_deep_scenes_render_and_drop_without_overflow() {
        let handle = std::thread::spawn(|| {
            let scene = deep_chain(100_000);
            let svg = render_svg(&scene, SvgText::Outlines);
            drop(scene);
            svg
        });
        let svg = handle.join().expect("no stack overflow");
        parse(&svg);
        assert!(!svg.contains("<path d="));
    }
}
