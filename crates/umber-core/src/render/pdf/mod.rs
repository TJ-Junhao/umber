//! The PDF backend.
//!
//! [`render_pdf`] writes a [`Scene`] as a one-page PDF whose page is the scene's size in
//! points. Everything stays vector: paths are filled and stroked as paths, gradients are axial
//! shadings, clip groups are clip paths, and opacity is an `ExtGState` alpha (a soft mask for a
//! gradient whose opacity varies). Marker shapes are form XObjects drawn at each position, and
//! images are Flate-compressed image XObjects scaled without interpolation. Text is real text:
//! every font the scene's text runs use is embedded as a subset of exactly the glyphs drawn
//! (TrueType outlines as `CIDFontType2`, CFF outlines as `CIDFontType0`), with a `ToUnicode`
//! map, so it can be selected, searched and copied.
//!
//! Values that cannot be drawn are skipped by the same rules as the other backends (see
//! [`super::normalize`]). The output is deterministic: no dates, objects in a fixed order, and a
//! document ID derived from the content.
//!
//! # Examples
//!
//! ```
//! use umber_core::render::render_pdf;
//! use umber_core::{Rgba, Scene};
//!
//! let white = Rgba { r: 255, g: 255, b: 255, a: 255 };
//! let pdf = render_pdf(&Scene::new(100.0, 50.0, white))?;
//! assert!(pdf.starts_with(b"%PDF-"));
//! # Ok::<(), umber_core::Error>(())
//! ```

// Module layering, lowest first; each module uses only the ones listed before it:
// - `write` holds number formatting, object numbering, hashing and compression;
// - `text` collects the glyphs of the scene's text, subsets and embeds fonts, and shows text;
// - `image` writes image XObjects;
// - `paint` writes the page content (paths, paints, clips, markers, images, text) and the
//   resources it uses.
mod image;
mod paint;
mod text;
mod write;

use pdf_writer::{Chunk, Filter, Finish, Pdf, Rect as PdfRect, TextStr};

use super::normalize::clip_is_valid;
use crate::error::number;
use crate::scene::{Item, MAX_GROUP_DEPTH, Rect, Scene};
use crate::{Error, Result};
use paint::{Page, Resources};
use text::{FontSet, Fonts};
use write::{Refs, compress, document_id, rounded};

/// The context of the errors of [`render_pdf`].
const CONTEXT: &str = "Figure.to_pdf()";

/// The largest page side in points that PDF readers accept: 200 inches.
const MAX_PAGE_SIDE: f32 = 14_400.0;

/// The smallest page side in points, the precision the document writes.
const MIN_PAGE_SIDE: f32 = 0.001;

/// Writes `scene` as a one-page PDF document.
///
/// The page is the scene's size in points, painted with the scene background, then with every
/// item in order. Text items are written as text in subsets of the fonts their glyphs were
/// shaped with, and extract as their source text. A text item without glyphs (built by hand)
/// is filled from its outline instead, as PNG draws it; so are the glyphs of a run whose font
/// cannot be read or embedded, while the item's other runs stay text. Text smaller than
/// 0.001 pt, which no rasterizer draws, is left out. Content of groups nested deeper than
/// [`MAX_GROUP_DEPTH`] is omitted. The same scene always gives the same bytes: the fonts come
/// from the scene's text items, which hold them (see [`TextGlyphs`](crate::TextGlyphs)).
///
/// # Errors
///
/// [`Error::Style`] with context `"Figure.to_pdf()"` when the scene's width or height is not
/// finite, below 0.001 pt or above 14,400 pt (200 in), the largest page PDF readers accept.
///
/// # Examples
///
/// ```
/// use umber_core::render::render_pdf;
/// use umber_core::{Rgba, Scene};
///
/// let white = Rgba { r: 255, g: 255, b: 255, a: 255 };
/// let pdf = render_pdf(&Scene::new(432.0, 288.0, white))?;
/// assert!(String::from_utf8_lossy(&pdf).contains("/MediaBox [0 0 432 288]"));
/// assert!(render_pdf(&Scene::new(f32::NAN, 288.0, white)).is_err());
/// # Ok::<(), umber_core::Error>(())
/// ```
pub fn render_pdf(scene: &Scene) -> Result<Vec<u8>> {
    let (width, height) = page_size(scene)?;
    let fonts = fonts_of(scene, (width, height));
    let (content, resources) = page_content(scene, &fonts, (width, height));

    // Objects 1 to 5 are the catalog, the page tree, the page, its content and the document
    // information; the resources follow in the order the content first uses them.
    let mut refs = Refs::new();
    let (catalog, pages, page_ref, content_ref, info) = (
        refs.next(),
        refs.next(),
        refs.next(),
        refs.next(),
        refs.next(),
    );
    let mut chunk = Chunk::new();
    let names = resources.write(&mut chunk, &mut refs, &fonts);

    let mut pdf = Pdf::new();
    pdf.catalog(catalog).pages(pages);
    pdf.pages(pages).kids([page_ref]).count(1);
    let mut page_dict = pdf.page(page_ref);
    page_dict
        .parent(pages)
        .media_box(PdfRect::new(0.0, 0.0, width, height))
        .contents(content_ref);
    names.write_into(&mut page_dict.resources());
    page_dict.finish();
    let compressed = compress(&content);
    pdf.stream(content_ref, &compressed)
        .filter(Filter::FlateDecode);
    pdf.document_info(info)
        .producer(TextStr(&format!("umber {}", crate::VERSION)));
    pdf.extend(&chunk);
    let id = document_id(pdf.as_bytes());
    pdf.set_file_id((id.clone(), id));
    Ok(pdf.finish())
}

/// The fonts of the text that `scene` draws on a page of `size`, subset to the glyphs drawn.
fn fonts_of(scene: &Scene, (width, height): (f32, f32)) -> Fonts {
    let mut fonts = FontSet::default();
    walk_page(scene, (width, height), &mut |item, _| {
        if let Item::Text(text) | Item::RotatedText { text, .. } = item {
            fonts.collect(text);
        }
    });
    fonts.embed()
}

/// The content stream of `scene` on a page of `size` whose text is set in `fonts`, and the
/// resources it uses.
fn page_content<'s>(
    scene: &'s Scene,
    fonts: &Fonts,
    (width, height): (f32, f32),
) -> (Vec<u8>, Resources<'s>) {
    let mut page = Page::new(width, height, fonts);
    page.background(scene.background);
    walk_page(scene, (width, height), &mut |item, clip| {
        page.item(item, clip)
    });
    page.finish()
}

/// The page width and height in points, rounded as the document writes them.
fn page_size(scene: &Scene) -> Result<(f32, f32)> {
    let valid = |side: f32| (MIN_PAGE_SIDE..=MAX_PAGE_SIDE).contains(&side);
    if valid(scene.width) && valid(scene.height) {
        Ok((rounded(scene.width), rounded(scene.height)))
    } else {
        Err(Error::style(
            CONTEXT,
            format!(
                "the page is {} by {} pt; a PDF page must be at least {MIN_PAGE_SIDE} pt and \
                 at most {MAX_PAGE_SIDE} pt on each side.",
                number(f64::from(scene.width)),
                number(f64::from(scene.height))
            ),
        )
        .with_hint("use a width and height above 0 and at most 200 in."))
    }
}

/// A clip rectangle in points, intersected with the page, with `left < right` and
/// `top < bottom`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Clip {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Clip {
    /// `rect` intersected with `bounds`; `None` when the rectangle cannot clip (the shared
    /// rule) or the intersection is empty.
    fn from_rect(rect: &Rect, bounds: Clip) -> Option<Clip> {
        if !clip_is_valid(rect) {
            return None;
        }
        bounds.intersect(Clip {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        })
    }

    /// The geometric intersection; `None` when it is empty.
    fn intersect(self, other: Clip) -> Option<Clip> {
        let out = Clip {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (out.left < out.right && out.top < out.bottom).then_some(out)
    }
}

/// The whole page as a clip rectangle.
fn page_bounds(width: f32, height: f32) -> Clip {
    Clip {
        left: 0.0,
        top: 0.0,
        right: width,
        bottom: height,
    }
}

/// Calls `f` on every leaf item of `scene` drawn on a page of `size`, as [`walk`] does.
fn walk_page<'s>(
    scene: &'s Scene,
    (width, height): (f32, f32),
    f: &mut impl FnMut(&'s Item, Option<Clip>),
) {
    walk(&scene.items, None, 0, page_bounds(width, height), f);
}

/// Calls `f` on every leaf item of `items` that the renderers draw, in order, with the clip in
/// effect for it: the intersection of the clips of its groups and the page, `None` when no
/// group clips it. A group whose clip cannot clip, or whose clip leaves nothing of the page or
/// of the clip around it, draws nothing, as in the other backends; so does the content of
/// groups nested [`MAX_GROUP_DEPTH`] deep.
fn walk<'s>(
    items: &'s [Item],
    clip: Option<Clip>,
    depth: usize,
    page: Clip,
    f: &mut impl FnMut(&'s Item, Option<Clip>),
) {
    for item in items {
        let Item::Group { clip: own, items } = item else {
            f(item, clip);
            continue;
        };
        if depth >= MAX_GROUP_DEPTH {
            continue;
        }
        let effective = match own {
            None => clip,
            Some(rect) => {
                let Some(rect) = Clip::from_rect(rect, page) else {
                    continue;
                };
                match clip {
                    None => Some(rect),
                    Some(parent) => match parent.intersect(rect) {
                        Some(both) => Some(both),
                        None => continue,
                    },
                }
            }
        };
        walk(items, effective, depth + 1, page, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;
    use crate::fonts::{Font, Weight};
    use crate::geometry::{Path, Point};
    use crate::scene::{
        FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint, StrokeStyle, TextItem,
    };
    use crate::text::{FaceId, FontLibrary, TextGlyphs, TextStyle};

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

    fn p(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn square(x: f32, y: f32, side: f32) -> Path {
        let mut path = Path::new();
        path.move_to(p(x, y));
        path.line_to(p(x + side, y));
        path.line_to(p(x + side, y + side));
        path.line_to(p(x, y + side));
        path.close();
        path
    }

    fn fill(path: Path, color: Rgba) -> Item {
        Item::Fill {
            path,
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        }
    }

    /// The content stream of `scene`, as text.
    fn content(scene: &Scene) -> String {
        let size = page_size(scene).unwrap();
        let fonts = fonts_of(scene, size);
        let (content, _) = page_content(scene, &fonts, size);
        String::from_utf8(content).unwrap()
    }

    /// The operators of `content`, one per line as the writer puts them.
    fn operators(content: &str) -> Vec<&str> {
        content
            .lines()
            .filter_map(|line| line.split_whitespace().last())
            .collect()
    }

    /// The deepest `q` nesting of `content`, asserting that every `q` is closed.
    fn state_depth(content: &str) -> usize {
        let (mut depth, mut deepest) = (0_usize, 0_usize);
        for op in operators(content) {
            match op {
                "q" => {
                    depth += 1;
                    deepest = deepest.max(depth);
                }
                "Q" => depth = depth.checked_sub(1).expect("Q without q"),
                _ => {}
            }
        }
        assert_eq!(depth, 0, "unclosed q");
        deepest
    }

    #[test]
    fn nested_clips_are_intersected_into_one_state() {
        let mut scene = Scene::new(400.0, 400.0, WHITE);
        // Groups nested past the depth limit, each clipped and holding one square.
        let mut group = Item::Group {
            clip: None,
            items: Vec::new(),
        };
        for level in (0..MAX_GROUP_DEPTH + 10).rev() {
            let inset = level as f32 * 0.5;
            group = Item::Group {
                clip: Some(Rect {
                    x: inset,
                    y: inset,
                    width: 400.0 - 2.0 * inset,
                    height: 400.0 - 2.0 * inset,
                }),
                items: vec![fill(square(150.0, 150.0, 10.0), RED), group],
            };
        }
        scene.push(group);
        let text = content(&scene);
        assert!(state_depth(&text) <= 2, "{text}");
        let fills = operators(&text).iter().filter(|op| **op == "f").count();
        // The background, then one square per drawn level.
        assert_eq!(fills, 1 + MAX_GROUP_DEPTH);
    }

    #[test]
    fn groups_whose_clip_leaves_nothing_draw_nothing() {
        let mut scene = Scene::new(100.0, 100.0, WHITE);
        let bad = |clip: Rect| Item::Group {
            clip: Some(clip),
            items: vec![fill(square(1.0, 1.0, 5.0), RED)],
        };
        for clip in [
            Rect {
                x: f32::NAN,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 10.0,
            },
            Rect {
                x: 200.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        ] {
            scene.push(bad(clip));
        }
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            items: vec![bad(Rect {
                x: 20.0,
                y: 20.0,
                width: 10.0,
                height: 10.0,
            })],
        });
        let text = content(&scene);
        assert_eq!(
            operators(&text).iter().filter(|op| **op == "f").count(),
            1,
            "{text}"
        );
    }

    #[test]
    fn hostile_values_write_no_invalid_numbers() {
        let nan = f32::NAN;
        let mut scene = Scene::new(120.0, 80.0, WHITE);
        let mut path = Path::new();
        path.move_to(p(nan, 1.0));
        path.line_to(p(10.0, f32::INFINITY));
        path.quad_to(p(1.0e30, 2.0), p(3.0, 4.0));
        path.line_to(p(20.0, 20.0));
        path.line_to(p(30.0, 25.0));
        scene.push(fill(path.clone(), RED));
        scene.push(Item::Stroke {
            path: path.clone(),
            style: StrokeStyle {
                color: RED,
                width: nan,
                dash: vec![nan, 1.0],
                cap: LineCap::Round,
                join: LineJoin::Bevel,
            },
        });
        scene.push(Item::Stroke {
            path: path.clone(),
            style: StrokeStyle {
                color: RED,
                width: 1.0,
                dash: vec![f32::INFINITY, 1.0],
                cap: LineCap::Square,
                join: LineJoin::Miter,
            },
        });
        scene.push(Item::Fill {
            path: square(5.0, 5.0, 20.0),
            paint: Paint::Linear(LinearGradient {
                start: p(nan, 0.0),
                end: p(10.0, 0.0),
                stops: vec![
                    GradientStop {
                        offset: nan,
                        color: RED,
                    },
                    GradientStop {
                        offset: 0.5,
                        color: WHITE,
                    },
                ],
            }),
            rule: FillRule::EvenOdd,
        });
        scene.push(Item::Markers {
            shape: square(-2.0, -2.0, 4.0),
            fill: Some(RED),
            stroke: None,
            positions: vec![p(nan, 1.0), p(1.0e30, 2.0), p(5.0, 5.0)],
        });
        scene.push(Item::Image {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: nan,
                height: 10.0,
            },
            width: 1,
            height: 1,
            pixels: vec![RED],
        });
        scene.push(Item::Image {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            width: 2,
            height: 2,
            pixels: vec![RED],
        });
        scene.push(Item::Text(TextItem {
            text: "x".into(),
            style: TextStyle {
                face: FaceId::new(0),
                weight: Weight::Regular,
                size: nan,
                tabular_figures: false,
            },
            origin: p(nan, 3.0),
            color: RED,
            outline: square(1.0, 1.0, 2.0),
            width: nan,
            glyphs: TextGlyphs::default(),
        }));
        let text = content(&scene);
        assert!(!text.contains("NaN") && !text.contains("inf"), "{text}");
        assert!(state_depth(&text) <= 2);
        // One marker of three is drawn; neither image is.
        assert_eq!(operators(&text).iter().filter(|op| **op == "Do").count(), 1);
        assert!(render_pdf(&scene).is_ok());
    }

    #[test]
    fn extreme_geometry_writes_only_reader_safe_numbers() {
        let mut scene = Scene::new(120.0, 80.0, WHITE);
        let mut path = Path::new();
        path.move_to(p(-1.0e6, 0.0));
        path.quad_to(p(1.0e6, -1.0e6), p(1.0e6, 1.0e6));
        path.line_to(p(1.0e30, -1.0e30));
        let style = StrokeStyle {
            color: RED,
            width: f32::MAX,
            dash: vec![1.0e30, 1.0e30],
            cap: LineCap::Round,
            join: LineJoin::Miter,
        };
        scene.push(Item::Stroke {
            path: path.clone(),
            style: style.clone(),
        });
        scene.push(Item::Markers {
            shape: path,
            fill: Some(RED),
            stroke: Some(style),
            positions: vec![p(-1.0e6, 1.0e6), p(f32::MAX, f32::MIN)],
        });
        scene.push(Item::Fill {
            path: square(0.0, 0.0, 50.0),
            paint: Paint::Linear(LinearGradient {
                start: p(-1.0e6, 0.0),
                end: p(1.0e6, 0.0),
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: RED,
                    },
                    GradientStop {
                        offset: f32::from_bits(1),
                        color: WHITE,
                    },
                    GradientStop {
                        offset: 1.0,
                        color: RED,
                    },
                ],
            }),
            rule: FillRule::NonZero,
        });
        let fonts = fonts_of(&scene, (120.0, 80.0));
        let (stream, resources) = page_content(&scene, &fonts, (120.0, 80.0));
        let mut chunk = Chunk::new();
        resources.write(&mut chunk, &mut Refs::new(), &fonts);
        for bytes in [stream.as_slice(), chunk.as_bytes()] {
            let text = String::from_utf8_lossy(bytes);
            for token in text.split(|c: char| c.is_whitespace() || c == '[' || c == ']') {
                if let Ok(number) = token.parse::<f64>() {
                    assert!(number.is_finite() && number.abs() <= 1.0e9, "{token}");
                    assert!(number == 0.0 || number.abs() >= 1.175e-38, "{token}");
                }
            }
        }
        // Text origins use the same numeric boundary as paths and marker transforms.
        let mut text = shaped(FontLibrary::global(), "edge");
        text.origin = p(-1.0e6, 1.0e6);
        scene.push(Item::RotatedText { text, angle: 45.0 });
        assert!(!content(&scene).contains("NaN"));
    }

    #[test]
    fn overflowing_text_positions_write_only_finite_numbers() {
        let fonts = FontLibrary::global()
            .with_figure_fonts(&[Font::INTER])
            .unwrap();
        let mut scene = Scene::new(120.0, 80.0, WHITE);
        let style = TextStyle {
            face: FaceId::new(0),
            weight: Weight::Regular,
            size: f32::MAX,
            tabular_figures: false,
        };
        scene
            .push_text(&fonts, "WWWWW", style, p(0.0, 0.0), RED)
            .unwrap();
        let stream = content(&scene);
        assert!(stream.contains("Tf"), "{stream}");
        for token in stream.split(|c: char| c.is_whitespace() || c == '[' || c == ']') {
            if let Ok(number) = token.parse::<f64>() {
                assert!(number.is_finite() && number.abs() <= 1.0e9, "{stream}");
            }
        }
    }

    #[test]
    fn hand_built_text_is_filled_from_its_outline() {
        let mut scene = Scene::new(60.0, 30.0, WHITE);
        scene.push(Item::Text(TextItem {
            text: "hand".into(),
            style: TextStyle {
                face: FaceId::new(0),
                weight: Weight::Regular,
                size: 12.0,
                tabular_figures: false,
            },
            origin: p(5.0, 20.0),
            color: RED,
            outline: square(5.0, 10.0, 8.0),
            width: 8.0,
            glyphs: TextGlyphs::default(),
        }));
        let text = content(&scene);
        assert!(!text.contains("BT"), "{text}");
        assert_eq!(operators(&text).iter().filter(|op| **op == "f").count(), 2);
        let pdf = String::from_utf8_lossy(&render_pdf(&scene).unwrap()).into_owned();
        assert!(!pdf.contains("/Font"), "no fonts are embedded");
    }

    #[test]
    fn shaped_text_is_written_as_text() {
        let fonts = FontLibrary::global()
            .with_figure_fonts(&[Font::INTER])
            .unwrap();
        let mut scene = Scene::new(100.0, 30.0, WHITE);
        let style = TextStyle {
            face: FaceId::new(0),
            weight: Weight::Regular,
            size: 12.0,
            tabular_figures: false,
        };
        scene
            .push_text(&fonts, "AVA", style, p(5.0, 20.0), RED)
            .unwrap();
        let text = content(&scene);
        let ops = operators(&text);
        for op in ["BT", "Tf", "Tm", "TJ", "ET"] {
            assert!(ops.contains(&op), "no {op} in {text}");
        }
        // The text space puts the origin at the left end of the baseline, the y axis up.
        assert!(text.contains("1 0 0 -1 5 20 cm"), "{text}");
        assert!(text.contains("/F0 12 Tf"), "{text}");
        assert_eq!(
            ops.iter().filter(|op| **op == "f").count(),
            1,
            "only the background"
        );
    }

    #[test]
    fn only_runs_whose_font_was_not_embedded_become_outlines() {
        let library = FontLibrary::with_fallbacks(vec![
            include_bytes!("../../../tests/fixtures/outline-cff.otf").to_vec(),
        ]);
        let fonts = library.with_figure_fonts(&[Font::INTER]).unwrap();
        let mut scene = Scene::new(100.0, 30.0, WHITE);
        let style = TextStyle {
            face: FaceId::new(0),
            weight: Weight::Regular,
            size: 12.0,
            tabular_figures: false,
        };
        scene
            .push_text(&fonts, "Ab\u{E001}", style, p(5.0, 20.0), RED)
            .unwrap();
        let size = page_size(&scene).unwrap();
        // Inter is font 0 and the fallback font 1.
        let fonts = fonts_of(&scene, size).without(1);
        let (content, _) = page_content(&scene, &fonts, size);
        let text = String::from_utf8(content).unwrap();
        let ops = operators(&text);
        assert!(ops.contains(&"TJ"), "Inter stays text: {text}");
        assert!(!text.contains("/F1"), "{text}");
        // The background, then the outline of the fallback run alone.
        assert_eq!(ops.iter().filter(|op| **op == "f").count(), 2, "{text}");
    }

    /// The text item of `text` in Inter at 12 pt from `library`, at (5, 20).
    fn shaped(library: &FontLibrary, text: &str) -> TextItem {
        let fonts = library.with_figure_fonts(&[Font::INTER]).unwrap();
        let style = TextStyle {
            face: FaceId::new(0),
            weight: Weight::Regular,
            size: 12.0,
            tabular_figures: false,
        };
        let mut scene = Scene::new(100.0, 30.0, WHITE);
        scene
            .push_text(&fonts, text, style, p(5.0, 20.0), RED)
            .unwrap();
        match scene.items.pop() {
            Some(Item::Text(item)) => item,
            other => panic!("expected text, got {other:?}"),
        }
    }

    /// The `cm` operators of `content`, as numbers.
    fn transforms(content: &str) -> Vec<Vec<f32>> {
        content
            .lines()
            .filter(|line| line.ends_with(" cm"))
            .map(|line| {
                line.split_whitespace()
                    .filter_map(|n| n.parse().ok())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn turned_text_is_written_in_a_turned_text_space() {
        let mut scene = Scene::new(100.0, 60.0, WHITE);
        scene.push(Item::RotatedText {
            text: shaped(FontLibrary::global(), "Turn"),
            angle: 30.0,
        });
        let text = content(&scene);
        let (cos, sin) = (30.0_f32.to_radians().cos(), 30.0_f32.to_radians().sin());
        let turned = transforms(&text)
            .into_iter()
            .find(|m| m.len() == 6 && (m[1] - sin).abs() < 1e-5)
            .unwrap_or_else(|| panic!("no turned text space in {text}"));
        assert!((turned[0] - cos).abs() < 1e-5 && (turned[2] - sin).abs() < 1e-5);
        assert!((turned[3] + cos).abs() < 1e-5);
        assert_eq!(&turned[4..], [5.0, 20.0]);
        assert!(operators(&text).contains(&"TJ"), "{text}");
        // A turn that is not finite writes the text unturned, as SVG does.
        let mut scene = Scene::new(100.0, 60.0, WHITE);
        scene.push(Item::RotatedText {
            text: shaped(FontLibrary::global(), "Turn"),
            angle: f32::NAN,
        });
        assert!(content(&scene).contains("1 0 0 -1 5 20 cm"));
    }

    #[test]
    fn turned_outlines_are_not_turned_again() {
        // A record built by hand holds its outline turned already: it is filled as it is.
        let mut scene = Scene::new(60.0, 30.0, WHITE);
        scene.push(Item::RotatedText {
            text: TextItem {
                text: "hand".into(),
                style: TextStyle {
                    face: FaceId::new(0),
                    weight: Weight::Regular,
                    size: 12.0,
                    tabular_figures: false,
                },
                origin: p(5.0, 20.0),
                color: RED,
                outline: square(5.0, 10.0, 8.0),
                width: 8.0,
                glyphs: TextGlyphs::default(),
            },
            angle: 45.0,
        });
        let text = content(&scene);
        assert_eq!(transforms(&text).len(), 1, "only the page's own: {text}");
        assert_eq!(operators(&text).iter().filter(|op| **op == "f").count(), 2);
        // A run whose font was not embedded is drawn from its unturned glyph outlines, turned
        // about the origin, while the other runs stay text.
        let library = FontLibrary::with_fallbacks(vec![
            include_bytes!("../../../tests/fixtures/outline-cff.otf").to_vec(),
        ]);
        let mut scene = Scene::new(100.0, 60.0, WHITE);
        scene.push(Item::RotatedText {
            text: shaped(&library, "Ab\u{E001}"),
            angle: 90.0,
        });
        let size = page_size(&scene).unwrap();
        let fonts = fonts_of(&scene, size).without(1);
        let (bytes, _) = page_content(&scene, &fonts, size);
        let text = String::from_utf8(bytes).unwrap();
        assert!(operators(&text).contains(&"TJ"), "{text}");
        let about = transforms(&text)
            .into_iter()
            .find(|m| m.len() == 6 && (m[1] - 1.0).abs() < 1e-5 && (m[2] + 1.0).abs() < 1e-5)
            .unwrap_or_else(|| panic!("no turn about the origin in {text}"));
        // Turning (5, 20) by a quarter turn about itself leaves it in place.
        assert!(
            (about[4] - 25.0).abs() < 1e-3 && (about[5] - 15.0).abs() < 1e-3,
            "{about:?}"
        );
    }

    #[test]
    fn translucent_paint_sets_an_opacity_state() {
        let mut scene = Scene::new(50.0, 50.0, Rgba { a: 0, ..WHITE });
        let half = Rgba { a: 128, ..RED };
        scene.push(fill(square(1.0, 1.0, 10.0), half));
        scene.push(fill(square(20.0, 1.0, 10.0), half));
        scene.push(fill(square(1.0, 20.0, 10.0), Rgba { a: 0, ..RED }));
        let text = content(&scene);
        // The transparent background and the transparent square paint nothing; both translucent
        // squares share one state.
        assert_eq!(text.matches("/G0 gs").count(), 2, "{text}");
        assert!(!text.contains("/G1"), "{text}");
        assert_eq!(operators(&text).iter().filter(|op| **op == "f").count(), 2);
    }

    #[test]
    fn gradients_with_varying_opacity_are_masked() {
        let mut scene = Scene::new(50.0, 50.0, WHITE);
        let gradient = |from: Rgba, to: Rgba| {
            Paint::Linear(LinearGradient {
                start: p(0.0, 0.0),
                end: p(0.0, 50.0),
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: from,
                    },
                    GradientStop {
                        offset: 1.0,
                        color: to,
                    },
                ],
            })
        };
        scene.push(Item::Fill {
            path: square(0.0, 0.0, 20.0),
            paint: gradient(RED, WHITE),
            rule: FillRule::NonZero,
        });
        scene.push(Item::Fill {
            path: square(25.0, 0.0, 20.0),
            paint: gradient(Rgba { a: 200, ..RED }, Rgba { a: 0, ..RED }),
            rule: FillRule::EvenOdd,
        });
        let text = content(&scene);
        assert!(text.contains("/S0 sh") && text.contains("/S1 sh"), "{text}");
        assert_eq!(
            text.matches(" gs").count(),
            1,
            "only the fading fill has a mask: {text}"
        );
        assert!(text.contains("W*"), "{text}");
        let pdf = String::from_utf8_lossy(&render_pdf(&scene).unwrap()).into_owned();
        assert!(pdf.contains("/S /Luminosity") && pdf.contains("/CS /DeviceGray"));
    }
}
