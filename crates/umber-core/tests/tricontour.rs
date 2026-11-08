//! Contours on triangle meshes are the same drawing in PNG, SVG and PDF; their bands meet
//! without a seam in PNG at 144 dpi on a light and a dark theme, and the outer boundary of the
//! mesh does not grow; and a large mesh stays one item per band and per level.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::{
    ContourLevels, ContourOptions, ContourStyle, Figure, FontLibrary, Item, Rect, Rgba, ScaleKind,
    Scene, SvgText, Theme, Triangles, delaunay,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;
use pdf_check::image::{self, DecodedPng};

/// `count` deterministic values from 0 to 1.
fn uniform(count: usize, seed: u64) -> Vec<f64> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1_u64 << 53) as f64
        })
        .collect()
}

/// `count` scattered points in the square from -2 to 2, with its four corners, so that their
/// triangulation covers the whole square, and a field with many levels across it.
fn square(count: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (u, v) = (uniform(count, 31), uniform(count, 32));
    let mut x = vec![-2.0, 2.0, 2.0, -2.0];
    let mut y = vec![-2.0, -2.0, 2.0, 2.0];
    x.extend(u.iter().map(|u| 4.0 * u - 2.0));
    y.extend(v.iter().map(|v| 4.0 * v - 2.0));
    let z = x
        .iter()
        .zip(&y)
        .map(|(x, y)| (-(x * x) - y * y).exp() + 0.3 * (2.0 * x).sin() * y)
        .collect();
    (x, y, z)
}

/// Filled contours of the values `z` at the points `(x, y)` under `theme`, with limits a unit
/// beyond the points when `margin`.
fn filled_figure(theme: Theme, (x, y, z): (&[f64], &[f64], &[f64]), margin: bool) -> Figure {
    let mut figure = Figure::new(5.0, 5.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    let options = ContourOptions {
        levels: ContourLevels::Count(14),
        style: ContourStyle::Filled,
        ..ContourOptions::default()
    };
    axes.tricontour(x, y, Triangles::Auto, z, options).unwrap();
    if margin {
        axes.xlim(-3.0, 3.0).unwrap().ylim(-3.0, 3.0).unwrap();
    }
    figure
}

/// A PNG of a figure, decoded, with the clip of its marks.
struct Shot {
    png: DecodedPng,
    clip: Rect,
    /// Pixels per point.
    scale: f64,
}

impl Shot {
    fn of(figure: &Figure, dpi: u32) -> Shot {
        let scene = figure.to_scene(FontLibrary::global()).unwrap();
        let clip = scene
            .items
            .iter()
            .find_map(|item| match item {
                Item::Group {
                    clip: Some(rect), ..
                } => Some(*rect),
                _ => None,
            })
            .unwrap();
        let png = umber_core::render::render_png(&scene, dpi).unwrap();
        Shot {
            png: image::decode_rgba8(&png).unwrap(),
            clip,
            scale: f64::from(dpi) / 72.0,
        }
    }

    /// The color of the pixel `(x, y)`.
    fn at(&self, x: usize, y: usize) -> [f64; 3] {
        let [r, g, b, _] = self
            .png
            .at(u32::try_from(x).unwrap(), u32::try_from(y).unwrap());
        [r, g, b].map(f64::from)
    }

    /// The pixels inside the clip, `inset` points away from its edges.
    fn inside(&self, inset: f64) -> impl Iterator<Item = (usize, usize)> + use<> {
        let clip = self.clip;
        let pixel = |v: f32, offset: f64| ((f64::from(v) + offset) * self.scale) as usize;
        let (left, top) = (pixel(clip.x, inset), pixel(clip.y, inset));
        let (right, bottom) = (
            pixel(clip.x + clip.width, -inset),
            pixel(clip.y + clip.height, -inset),
        );
        (top..bottom).flat_map(move |y| (left..right).map(move |x| (x, y)))
    }

    /// The pixel position of the data point `(x, y)` on axes whose limits are -3 to 3.
    fn pixel(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let clip = self.clip;
        (
            (f64::from(clip.x) + (x + 3.0) / 6.0 * f64::from(clip.width)) * self.scale,
            (f64::from(clip.y) + (3.0 - y) / 6.0 * f64::from(clip.height)) * self.scale,
        )
    }
}

/// The largest difference, in channel units, between the pixels inside the mesh of two shots
/// of the same mesh on different backgrounds: where the bands meet without a seam no pixel
/// lets the background through, so none differs.
fn background_through(a: &Shot, b: &Shot) -> f64 {
    a.inside(1.5)
        .map(|(x, y)| {
            let (p, q) = (a.at(x, y), b.at(x, y));
            (0..3).map(|i| (p[i] - q[i]).abs()).fold(0.0, f64::max)
        })
        .fold(0.0, f64::max)
}

/// How far the mesh reaches past its left boundary, in points: the coverage of a row of pixels
/// across the boundary, in the middle, against the fill 4 pt inside it, less the exact covered
/// length.
fn boundary_growth(theme: &Theme, shot: &Shot) -> f64 {
    let background = theme.background();
    let background = [background.r, background.g, background.b].map(f64::from);
    let (edge, middle) = shot.pixel((-2.0, 0.1));
    let row = middle as usize;
    let fill = shot.at((edge + 4.0 * shot.scale) as usize, row);
    let coverage = |p: [f64; 3]| {
        let d = [0, 1, 2].map(|i| fill[i] - background[i]);
        let q = [0, 1, 2].map(|i| p[i] - background[i]);
        let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
        (dot(d, q) / dot(d, d)).clamp(0.0, 1.0)
    };
    let window = (3.0 * shot.scale) as usize;
    let (start, end) = (edge as usize - window, edge as usize + window);
    let covered: f64 = (start..end).map(|x| coverage(shot.at(x, row))).sum();
    (covered - (end as f64 - edge)) / shot.scale
}

/// Filled contours of the values `-x` on a grid over the square from -2 to 2, under `theme`,
/// with limits a unit beyond it, as [`filled_figure`] draws them on a mesh.
fn grid_figure(theme: Theme) -> Figure {
    let axis: Vec<f64> = (0..=40).map(|i| f64::from(i) / 10.0 - 2.0).collect();
    let z: Vec<f64> = axis.iter().flat_map(|_| axis.iter().map(|x| -x)).collect();
    let mut figure = Figure::new(5.0, 5.0, theme).unwrap();
    let options = ContourOptions {
        levels: ContourLevels::Count(14),
        style: ContourStyle::Filled,
        ..ContourOptions::default()
    };
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .contour(&axis, &axis, &z, options)
        .unwrap()
        .xlim(-3.0, 3.0)
        .unwrap()
        .ylim(-3.0, 3.0)
        .unwrap();
    figure
}

#[test]
fn bands_meet_without_a_seam_and_the_outer_boundary_grows_no_more_than_on_a_grid() {
    let (red, green) = (Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0));
    for theme in [Theme::pigment(), Theme::midnight()] {
        // The mesh covers the whole plot: wherever a band edge lets the background through,
        // the two backgrounds show.
        let (x, y, z) = square(600);
        let shots = [red, green].map(|background| {
            let theme = theme.with_background(background).unwrap();
            Shot::of(&filled_figure(theme, (&x, &y, &z), false), 144)
        });
        let through = background_through(&shots[0], &shots[1]);
        assert!(through <= 1.0, "the background shows by {through}");
        // Values rising to the right: only the lowest band meets the left boundary, and the
        // boundary is where the geometry puts it. No hairline widens it.
        let rising: Vec<f64> = x.clone();
        let figure = filled_figure(theme.clone(), (&x, &y, &rising), true);
        let growth = boundary_growth(&theme, &Shot::of(&figure, 144));
        eprintln!("one band on the boundary: it grows by {growth:.3} pt");
        assert!(growth.abs() < 0.1, "the boundary grows by {growth} pt");
        // Values falling to the right: every band meets the boundary, each painted over the
        // ones before, so the antialiased pixels along it add up as they do for contour() on
        // a grid, whose bands are painted the same way.
        let falling: Vec<f64> = x.iter().map(|x| -x).collect();
        let figure = filled_figure(theme.clone(), (&x, &y, &falling), true);
        let growth = boundary_growth(&theme, &Shot::of(&figure, 144));
        let grid = boundary_growth(&theme, &Shot::of(&grid_figure(theme.clone()), 144));
        eprintln!("every band on the boundary: it grows by {growth:.3} pt, on a grid {grid:.3} pt");
        assert!(
            growth < grid + 0.05,
            "{growth} pt against {grid} pt on a grid"
        );
    }
}

#[test]
fn tricontours_agree_in_png_svg_and_pdf() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!("skipped the PDF raster comparison: pdftoppm is not installed");
    }
    let (x, y, z) = square(300);
    let rows = delaunay(&x, &y).unwrap();
    let holed: Vec<[i64; 3]> = rows
        .into_iter()
        .filter(|row| {
            let [a, b, c] = row.map(|i| usize::try_from(i).unwrap());
            ((x[a] + x[b] + x[c]) / 3.0).hypot((y[a] + y[b] + y[c]) / 3.0) > 0.7
        })
        .collect();
    for (name, theme) in [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let mut figure = Figure::new(5.0, 4.0, theme).unwrap();
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        let options = ContourOptions {
            style: ContourStyle::Both,
            annotate: true,
            ..ContourOptions::default()
        };
        axes.tricontour(&x, &y, Triangles::Given(&holed), &z, options)
            .unwrap();
        axes.equal_aspect().unwrap();
        let scene = figure.to_scene(FontLibrary::global()).unwrap();
        for text in [SvgText::Editable, SvgText::Outlines] {
            let svg = figure.to_svg(text).unwrap();
            assert!(svg.contains("evenodd"), "{name}");
        }
        let dpi: u32 = 144;
        let png = figure.to_png(i64::from(dpi)).unwrap();
        assert!(image::decode_rgba8(&png).is_ok(), "{name}");
        let pdf = figure.to_pdf().unwrap();
        assert_eq!(pdf, umber_core::render::render_pdf(&scene).unwrap());
        if poppler {
            let dir = pdf_check::scratch(&format!("tricontour-{name}"));
            let path = pdf_check::write(&dir, "tricontour.pdf", &pdf);
            pdf_check::assert_matches_png(&path, &png, dpi);
        }
    }
}

/// The items of the marks group of `scene`.
fn marks(scene: &Scene) -> &[Item] {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(_),
                items,
            } => Some(items.as_slice()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn a_large_mesh_is_one_fill_per_band_and_one_stroke_per_level() {
    let (x, y, z) = square(20_000);
    let mut figure = Figure::new(5.0, 5.0, Theme::pigment()).unwrap();
    let options = ContourOptions {
        style: ContourStyle::Both,
        ..ContourOptions::default()
    };
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .tricontour(&x, &y, Triangles::Auto, &z, options)
        .unwrap();
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    let items = marks(&scene);
    let fills = items
        .iter()
        .filter(|item| matches!(item, Item::Fill { .. }))
        .count();
    let strokes = items
        .iter()
        .filter(|item| matches!(item, Item::Stroke { .. }))
        .count();
    assert_eq!(items.len(), fills + strokes);
    assert_eq!(fills, strokes + 1, "{fills} bands and {strokes} levels");
    assert!((5..=12).contains(&strokes), "{strokes}");
    for text in [SvgText::Editable, SvgText::Outlines] {
        assert!(figure.to_svg(text).is_ok());
    }
    assert!(figure.to_png(72).is_ok());
    assert!(figure.to_pdf().is_ok());
}
