//! Triangle meshes are the same drawing in PNG, SVG and PDF; the filled triangles of a mesh
//! meet without a seam in PNG at 144 dpi on a light and a dark theme, their outer boundary and
//! the edge of a blank triangle grow by half the hairline at most; and a large mesh stays two
//! scene items per color.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;

use umber_core::{
    Figure, FontLibrary, Item, Rect, ScaleKind, Scene, SvgText, Theme, TriValues, Triangles,
    TripcolorOptions, TriplotOptions, delaunay,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;
use pdf_check::image::{self, DecodedPng};

/// The width of the hairline that strokes each filled triangle, in points.
const HAIRLINE: f64 = 0.75;

/// The number of squares along each side of the checkered mesh.
const SIDE: usize = 12;

/// A mesh of `SIDE` by `SIDE` squares, each split into two triangles, its inner points moved off
/// the lattice so that the edges run at many angles, with values 0 and 1 alternating between
/// neighbouring triangles: the points, the triangles and one value per triangle.
fn checkered() -> (Vec<f64>, Vec<f64>, Vec<[i64; 3]>, Vec<f64>) {
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for j in 0..=SIDE {
        for i in 0..=SIDE {
            let (fi, fj) = (i as f64, j as f64);
            let inner = if (1..SIDE).contains(&i) && (1..SIDE).contains(&j) {
                0.3
            } else {
                0.0
            };
            x.push(fi + inner * (fi * 1.7 + fj * 2.9).sin());
            y.push(fj + inner * (fj * 2.3 - fi * 1.3).cos());
        }
    }
    let at = |i: usize, j: usize| i64::try_from(j * (SIDE + 1) + i).unwrap();
    let (mut triangles, mut values) = (Vec::new(), Vec::new());
    for j in 0..SIDE {
        for i in 0..SIDE {
            let k = ((i + j) % 2) as f64;
            triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            values.push(k);
            triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
            values.push(1.0 - k);
        }
    }
    (x, y, triangles, values)
}

/// The index of a triangle of [`checkered`] near the middle.
const MIDDLE: usize = (SIDE / 2) * SIDE * 2 + SIDE;

/// The checkered mesh under `theme`, with the triangle [`MIDDLE`] blank when `blank`, and with
/// limits a square beyond the points when `margin`.
fn checkered_figure(theme: Theme, blank: bool, margin: bool) -> Figure {
    let (x, y, triangles, mut values) = checkered();
    if blank {
        values[MIDDLE] = f64::NAN;
    }
    let mut figure = Figure::new(5.0, 5.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.tripcolor(
        &x,
        &y,
        Triangles::Given(&triangles),
        &values,
        TripcolorOptions {
            on: TriValues::Triangles,
            ..TripcolorOptions::default()
        },
    )
    .unwrap();
    if margin {
        let far = SIDE as f64 + 1.0;
        axes.xlim(-1.0, far).unwrap().ylim(-1.0, far).unwrap();
    }
    figure
}

/// The scene of `figure` and the clip of its marks, without the strokes of the marks when
/// `bare`.
fn scene_of(figure: &Figure, bare: bool) -> (Scene, Rect) {
    let mut scene = figure.to_scene(FontLibrary::global()).unwrap();
    let mut clip = None;
    for item in &mut scene.items {
        if let Item::Group {
            clip: Some(rect),
            items,
        } = item
            && clip.is_none()
        {
            clip = Some(*rect);
            if bare {
                items.retain(|item| !matches!(item, Item::Stroke { .. }));
            }
        }
    }
    (scene, clip.unwrap())
}

/// A PNG of a scene, decoded, with the clip of its marks.
struct Shot {
    png: DecodedPng,
    clip: Rect,
    /// Pixels per point.
    scale: f64,
}

impl Shot {
    fn of(figure: &Figure, bare: bool, dpi: u32) -> Shot {
        let (scene, clip) = scene_of(figure, bare);
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

    /// The pixel position of the data point `(x, y)` on axes whose limits are a square beyond
    /// the checkered mesh.
    fn pixel(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let span = SIDE as f64 + 2.0;
        let clip = self.clip;
        (
            (f64::from(clip.x) + (x + 1.0) / span * f64::from(clip.width)) * self.scale,
            (f64::from(clip.y) + (SIDE as f64 + 1.0 - y) / span * f64::from(clip.height))
                * self.scale,
        )
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
}

/// The distance from the color `p` to the segment from `a` to `b`, in channel units: how far
/// `p` is from every blend of the two.
fn off_blend(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let t = (dot(ab, ap) / dot(ab, ab)).clamp(0.0, 1.0);
    let gap = [
        a[0] + t * ab[0] - p[0],
        a[1] + t * ab[1] - p[1],
        a[2] + t * ab[2] - p[2],
    ];
    dot(gap, gap).sqrt()
}

/// How far each pixel inside the mesh, away from its outer boundary, lies off every blend of the
/// mesh's two colors: the largest distance, and the number of pixels more than 3 units off.
fn seams(shot: &Shot) -> (f64, usize) {
    let mut counts: HashMap<[u64; 3], usize> = HashMap::new();
    for (x, y) in shot.inside(10.0) {
        *counts.entry(shot.at(x, y).map(|c| c as u64)).or_default() += 1;
    }
    let mut common: Vec<([u64; 3], usize)> = counts.into_iter().collect();
    common.sort_by_key(|&(color, n)| (std::cmp::Reverse(n), color));
    let [a, b] = [common[0].0, common[1].0].map(|color| color.map(|c| c as f64));
    shot.inside(10.0)
        .map(|(x, y)| off_blend(shot.at(x, y), a, b))
        .fold((0.0_f64, 0_usize), |(worst, far), d| {
            (worst.max(d), far + usize::from(d > 3.0))
        })
}

/// How far the mesh reaches past its left boundary, in points: the coverage of a row of pixels
/// across the boundary, in the middle of a triangle, less the exact covered length.
fn boundary_growth(theme: &Theme, shot: &Shot) -> f64 {
    let background = theme.background();
    let background = [background.r, background.g, background.b].map(f64::from);
    let (edge, middle) = shot.pixel((0.0, 5.5));
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

/// How deep, in points, the colors of the neighbours reach into the blank triangle
/// [`MIDDLE`]: the largest distance from its edges of a pixel inside it that is not the
/// background.
fn reach_into_blank(theme: &Theme, shot: &Shot) -> f64 {
    let (x, y, triangles, _) = checkered();
    let corners = triangles[MIDDLE].map(|i| {
        let i = usize::try_from(i).unwrap();
        shot.pixel((x[i], y[i]))
    });
    let background = theme.background();
    let background = [background.r, background.g, background.b].map(f64::from);
    let side = |p: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
    };
    let distance = |p: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        (a.0 + t * dx - p.0).hypot(a.1 + t * dy - p.1)
    };
    let (low_x, high_x) = corners.iter().fold((f64::INFINITY, 0.0_f64), |(l, h), c| {
        (l.min(c.0), h.max(c.0))
    });
    let (low_y, high_y) = corners.iter().fold((f64::INFINITY, 0.0_f64), |(l, h), c| {
        (l.min(c.1), h.max(c.1))
    });
    let mut reach = 0.0_f64;
    for py in low_y as usize..=high_y as usize {
        for px in low_x as usize..=high_x as usize {
            let p = (px as f64 + 0.5, py as f64 + 0.5);
            let sides = [0, 1, 2].map(|e| side(p, corners[e], corners[(e + 1) % 3]));
            if !(sides.iter().all(|&s| s >= 0.0) || sides.iter().all(|&s| s <= 0.0)) {
                continue;
            }
            let color = shot.at(px, py);
            if (0..3).any(|i| (color[i] - background[i]).abs() > 2.0) {
                let depth = [0, 1, 2]
                    .map(|e| distance(p, corners[e], corners[(e + 1) % 3]))
                    .into_iter()
                    .fold(f64::INFINITY, f64::min);
                reach = reach.max(depth);
            }
        }
    }
    reach / shot.scale
}

#[test]
fn neighbouring_triangles_meet_without_a_seam_and_the_boundary_grows_by_half_the_hairline() {
    for theme in [Theme::pigment(), Theme::midnight()] {
        let figure = checkered_figure(theme.clone(), false, false);
        // Without the hairlines the background shows between the triangles.
        let (worst, far) = seams(&Shot::of(&figure, true, 144));
        assert!(worst > 7.0 && far > 1000, "{worst} {far}");
        let (worst, far) = seams(&Shot::of(&figure, false, 144));
        assert!(worst < 2.0 && far == 0, "{worst} {far}");
        let figure = checkered_figure(theme.clone(), false, true);
        let growth = boundary_growth(&theme, &Shot::of(&figure, false, 144));
        let bare = boundary_growth(&theme, &Shot::of(&figure, true, 144));
        assert!(bare.abs() < 0.1, "{bare}");
        assert!(
            (growth - HAIRLINE / 2.0).abs() < 0.1,
            "the boundary grows by {growth} pt"
        );
        // A blank triangle: the neighbours reach into it by half the hairline, and the edge's
        // antialiasing, at most.
        let figure = checkered_figure(theme.clone(), true, true);
        let reach = reach_into_blank(&theme, &Shot::of(&figure, false, 144));
        assert!(reach < HAIRLINE / 2.0 + 0.4, "{reach}");
    }
}

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

/// Scattered points in an annulus, joined by the Delaunay triangles outside the hole, with a
/// smooth field: the points, the triangles and one value per point.
fn annulus(count: usize) -> (Vec<f64>, Vec<f64>, Vec<[i64; 3]>, Vec<f64>) {
    let (u, v) = (uniform(count, 3), uniform(count, 4));
    let (x, y): (Vec<f64>, Vec<f64>) = u
        .iter()
        .zip(&v)
        .map(|(&u, &v)| {
            let radius = (0.16 + 0.84 * u).sqrt();
            let angle = std::f64::consts::TAU * v;
            (radius * angle.cos(), radius * angle.sin())
        })
        .unzip();
    let triangles: Vec<[i64; 3]> = delaunay(&x, &y)
        .unwrap()
        .into_iter()
        .filter(|row| {
            let [a, b, c] = row.map(|i| usize::try_from(i).unwrap());
            let (cx, cy) = ((x[a] + x[b] + x[c]) / 3.0, (y[a] + y[b] + y[c]) / 3.0);
            cx.hypot(cy) > 0.4
        })
        .collect();
    let values = x.iter().zip(&y).map(|(x, y)| x * x - y).collect();
    (x, y, triangles, values)
}

fn mesh_figure(theme: Theme, count: usize, plotted: bool) -> Figure {
    let (x, y, triangles, values) = annulus(count);
    let mut figure = Figure::new(5.0, 4.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.tripcolor(
        &x,
        &y,
        Triangles::Given(&triangles),
        &values,
        TripcolorOptions::default(),
    )
    .unwrap();
    if plotted {
        axes.triplot(
            &x,
            &y,
            Triangles::Given(&triangles),
            TriplotOptions::default(),
        )
        .unwrap();
    }
    axes.equal_aspect().unwrap();
    figure
}

#[test]
fn triangle_meshes_agree_in_png_svg_and_pdf() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!("skipped the PDF raster comparison: pdftoppm is not installed");
    }
    for (name, theme) in [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let figure = mesh_figure(theme, 300, true);
        let scene = figure.to_scene(FontLibrary::global()).unwrap();
        for text in [SvgText::Editable, SvgText::Outlines] {
            assert!(
                figure.to_svg(text, 300).unwrap().contains("<path"),
                "{name}"
            );
        }
        let dpi: u32 = 144;
        let png = figure.to_png(i64::from(dpi)).unwrap();
        assert!(image::decode_rgba8(&png).is_ok(), "{name}");
        let pdf = figure.to_pdf(300).unwrap();
        assert_eq!(pdf, umber_core::render::render_pdf(&scene, 300).unwrap());
        if poppler {
            let dir = pdf_check::scratch(&format!("triangles-{name}"));
            let path = pdf_check::write(&dir, "triangles.pdf", &pdf);
            pdf_check::assert_matches_png(&path, &png, dpi);
        }
    }
}

#[test]
fn a_large_mesh_is_two_items_per_color() {
    let figure = mesh_figure(Theme::pigment(), 20_000, false);
    let (scene, _) = scene_of(&figure, false);
    let Some(Item::Group { items, .. }) = scene
        .items
        .iter()
        .find(|item| matches!(item, Item::Group { clip: Some(_), .. }))
    else {
        panic!("a marks group");
    };
    let colors: Vec<_> = items
        .chunks(2)
        .map(|pair| match pair {
            [Item::Fill { paint, .. }, Item::Stroke { style, .. }] => {
                assert_eq!(*paint, umber_core::Paint::Solid(style.color));
                style.color
            }
            other => panic!("a fill and its stroke: {other:?}"),
        })
        .collect();
    let mut distinct = colors.clone();
    distinct.sort_by_key(|c| (c.r, c.g, c.b, c.a));
    distinct.dedup();
    assert_eq!(distinct.len(), colors.len(), "each color once");
    assert!(colors.len() < 2000, "{}", colors.len());
    for text in [SvgText::Editable, SvgText::Outlines] {
        assert!(figure.to_svg(text, 300).is_ok());
    }
    assert!(figure.to_png(72).is_ok());
    assert!(figure.to_pdf(300).is_ok());
}
