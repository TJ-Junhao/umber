//! Heatmap, 2-D histogram and mesh cells drawn as one image ([`Render`]): which form each
//! setting draws, and that the image looks like the vector cells.
//!
//! The equivalence checks render a figure with [`Render::Vector`] and again with
//! [`Render::Raster`] and compare the PNGs pixel by pixel, except for the device pixels that
//! touch a cell edge or a plot edge: there the vector cells are antialiased and the image is
//! averaged, which differ by design. Everywhere else, cell interiors, empty cells, the cut
//! parts beyond explicit limits and everything around the plot, the two must agree.

use super::super::mapping::{AxisMaps, axis_time, axis_value};
use crate::model::{AxisData, Edges, Figure};
use crate::render::{SvgText, render_png, render_svg};
use crate::resolve::{AxisScale, ResolvedAxis, ResolvedMark};
use crate::scene::{Item, Paint, Scene};
use crate::test_support::image::{DecodedPng, decode_rgba8};
use crate::{
    Error, FontLibrary, HeatmapOptions, Hist2dOptions, Instant, LogBase, MeshOptions, Render, Rgba,
    ScaleKind, Theme, Transform,
};

/// The three settings, in a fixed order.
const RENDERS: [Render; 3] = [Render::Auto, Render::Vector, Render::Raster];

fn names(prefix: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}{i}")).collect()
}

/// A heatmap of `columns` by `rows` cells with distinct values, a NaN every seventh cell.
fn heatmap(theme: Theme, (columns, rows): (usize, usize), options: HeatmapOptions) -> Figure {
    let values: Vec<f64> = (0..columns * rows)
        .map(|i| {
            if i % 7 == 3 {
                f64::NAN
            } else {
                1.0 + (i * 37 % 101) as f64
            }
        })
        .collect();
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(&names("c", columns), &names("r", rows), &values, options)
        .unwrap();
    f
}

/// Values for a mesh of `columns` by `rows` cells: distinct, a NaN every fifth cell.
fn mesh_values(columns: usize, rows: usize) -> Vec<f64> {
    (0..columns * rows)
        .map(|i| {
            if i % 5 == 2 {
                f64::NAN
            } else {
                ((i * 29) % 83) as f64 - 20.0
            }
        })
        .collect()
}

/// `n + 1` evenly spaced edges from `start` by `step`.
fn even(start: f64, step: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| start + step * i as f64).collect()
}

/// A mesh of 8 by 6 cells on linear axes, with limits that cut the outer cells.
fn linear_mesh(theme: Theme, render: Render) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .mesh(
            &even(0.0, 1.5, 8),
            &even(-3.0, 1.0, 6),
            &mesh_values(8, 6),
            MeshOptions {
                render,
                ..MeshOptions::default()
            },
        )
        .unwrap()
        .xlim(0.7, 11.3)
        .unwrap()
        .ylim(-2.6, 2.2)
        .unwrap();
    f
}

/// A mesh of 6 by 5 cells on log axes, edges evenly spaced in their logarithms.
fn log_mesh(theme: Theme, options: MeshOptions) -> Figure {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let decades = |n: usize| {
        (0..=n)
            .map(|i| 10f64.powf(i as f64 * 0.5))
            .collect::<Vec<_>>()
    };
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(log, log)
        .unwrap()
        .mesh(&decades(6), &decades(5), &mesh_values(6, 5), options)
        .unwrap();
    f
}

/// A day in microseconds since the epoch.
fn day(n: i64) -> Option<Instant> {
    Some(Instant::from_micros(n * 86_400_000_000).unwrap())
}

/// A mesh of 7 by 4 cells with daily edges on a time x axis.
fn time_mesh(theme: Theme, render: Render) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Time, ScaleKind::Linear)
        .unwrap()
        .mesh_time(
            AxisData::Times((20_000..=20_007).map(day).collect()),
            AxisData::Numbers(even(0.0, 2.0, 4)),
            &mesh_values(7, 4),
            MeshOptions {
                render,
                ..MeshOptions::default()
            },
        )
        .unwrap();
    f
}

/// Points spread over a square, denser near its center.
fn points() -> (Vec<f64>, Vec<f64>) {
    (0..4000)
        .map(|i| {
            let t = f64::from(i);
            let r = (t / 4000.0).sqrt() * 5.0;
            (r * (t * 2.399).cos(), r * (t * 2.399).sin())
        })
        .unzip()
}

/// A 2-D histogram of 9 by 7 bins with limits that cut the outer bins.
fn hist2d(theme: Theme, (columns, rows): (usize, usize), render: Render) -> Figure {
    let (x, y) = points();
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .hist2d(
            &x,
            &y,
            Hist2dOptions {
                bins: (columns, rows),
                render,
                ..Hist2dOptions::default()
            },
        )
        .unwrap();
    f
}

fn scene(f: &Figure) -> Scene {
    f.to_scene(FontLibrary::global()).unwrap()
}

/// The items of the marks group of `scene`: the first group with a clip.
fn mark_items(scene: &Scene) -> &[Item] {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(_),
                items,
            } => Some(&items[..]),
            _ => None,
        })
        .expect("a marks group")
}

/// How the cells of the one cell mark of `f` are drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drawn {
    /// As this many filled shapes.
    Fills(usize),
    /// As one image of this many columns and rows, clipped to its shown part when `clipped`.
    Image {
        width: u32,
        height: u32,
        clipped: bool,
    },
}

fn drawn(f: &Figure) -> Drawn {
    let scene = scene(f);
    match mark_items(&scene) {
        [
            Item::Image {
                width,
                height,
                pixels,
                ..
            },
        ] => {
            assert_eq!(pixels.len(), (*width * *height) as usize);
            Drawn::Image {
                width: *width,
                height: *height,
                clipped: false,
            }
        }
        [
            Item::Group {
                clip: Some(_),
                items,
            },
        ] => match &items[..] {
            [Item::Image { width, height, .. }] => Drawn::Image {
                width: *width,
                height: *height,
                clipped: true,
            },
            other => panic!("expected one image in the clip group, got {other:?}"),
        },
        items => {
            assert!(
                items.iter().all(|item| matches!(item, Item::Fill { .. })),
                "expected cell fills, got {items:?}"
            );
            Drawn::Fills(items.len())
        }
    }
}

fn is_image(d: Drawn) -> bool {
    matches!(d, Drawn::Image { .. })
}

#[test]
fn each_setting_draws_its_form() {
    for render in RENDERS {
        let wanted = render == Render::Raster;
        let figures = [
            heatmap(
                Theme::pigment(),
                (6, 4),
                HeatmapOptions {
                    render,
                    ..HeatmapOptions::default()
                },
            ),
            hist2d(Theme::pigment(), (9, 7), render),
            linear_mesh(Theme::pigment(), render),
            log_mesh(
                Theme::pigment(),
                MeshOptions {
                    render,
                    ..MeshOptions::default()
                },
            ),
            time_mesh(Theme::pigment(), render),
        ];
        for (i, f) in figures.iter().enumerate() {
            assert_eq!(is_image(drawn(f)), wanted, "{render:?}, figure {i}");
        }
    }
}

#[test]
fn image_has_one_pixel_per_shown_cell() {
    // 6 x 4 heatmap: the whole grid.
    let f = heatmap(
        Theme::pigment(),
        (6, 4),
        HeatmapOptions {
            render: Render::Raster,
            ..HeatmapOptions::default()
        },
    );
    assert_eq!(
        drawn(&f),
        Drawn::Image {
            width: 6,
            height: 4,
            clipped: false
        }
    );
    // The limits 0.7..11.3 cut x cells 0 (0..1.5) and 7 (10.5..12), and -2.6..2.2 cut y cells
    // 0 (-3..-2) and 5 (2..3): all stay, clipped.
    assert_eq!(
        drawn(&linear_mesh(Theme::pigment(), Render::Raster)),
        Drawn::Image {
            width: 8,
            height: 6,
            clipped: true
        }
    );
    assert_eq!(
        drawn(&time_mesh(Theme::pigment(), Render::Raster)),
        Drawn::Image {
            width: 7,
            height: 4,
            clipped: false
        }
    );
}

#[test]
fn auto_draws_an_image_from_ten_thousand_cells() {
    let auto = HeatmapOptions::default();
    assert_eq!(
        drawn(&heatmap(Theme::pigment(), (99, 101), auto.clone())),
        Drawn::Fills(99 * 101 - (0..99 * 101).filter(|i| i % 7 == 3).count())
    );
    assert!(is_image(drawn(&heatmap(
        Theme::pigment(),
        (100, 100),
        auto
    ))));
    assert!(!is_image(drawn(&hist2d(
        Theme::pigment(),
        (99, 101),
        Render::Auto
    ))));
    assert!(is_image(drawn(&hist2d(
        Theme::pigment(),
        (100, 100),
        Render::Auto
    ))));
    let mesh = |columns: usize, rows: usize| {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .mesh(
                &even(0.0, 1.0, columns),
                &even(0.0, 1.0, rows),
                &vec![1.0; columns * rows],
                MeshOptions::default(),
            )
            .unwrap();
        f
    };
    assert!(!is_image(drawn(&mesh(99, 101))));
    assert!(is_image(drawn(&mesh(100, 100))));
}

#[test]
fn auto_draws_uneven_meshes_as_vector_shapes() {
    let mut edges = even(0.0, 1.0, 120);
    edges[60] = 60.5;
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .mesh(
            &edges,
            &even(0.0, 1.0, 100),
            &vec![1.0; 120 * 100],
            MeshOptions::default(),
        )
        .unwrap();
    assert_eq!(drawn(&f), Drawn::Fills(120 * 100));
}

/// The error of a raster mesh with `x` and `y` edges on axes of `kinds`, after which the figure
/// must be unchanged.
fn raster_error(kinds: [ScaleKind; 2], x: AxisData, y: AxisData, values: &[f64]) -> Error {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(kinds[0], kinds[1]).unwrap();
    let before = f.clone();
    let err = f
        .axes_at_mut(crate::GridIndex::Auto, crate::GridIndex::Auto)
        .unwrap()
        .mesh_time(
            x,
            y,
            values,
            MeshOptions {
                render: Render::Raster,
                ..MeshOptions::default()
            },
        )
        .unwrap_err();
    assert_eq!(f, before);
    err
}

#[test]
fn raster_refuses_uneven_edges_naming_the_first_uneven_step() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let numbers = |v: &[f64]| AxisData::Numbers(v.to_vec());
    let cases = [
        (
            raster_error(
                [ScaleKind::Linear, ScaleKind::Linear],
                numbers(&[0.0, 1.0, 2.0, 4.0]),
                numbers(&[0.0, 1.0]),
                &[1.0; 3],
            ),
            "Axes.mesh(): the step from x_edges[2] = 2 to x_edges[3] = 4 is 2, not 1 as from \
             x_edges[0] to x_edges[1]; drawing the cells as one image needs evenly spaced edges \
             on the x axis.",
        ),
        (
            // Evenly spaced numbers are uneven on a log axis.
            raster_error(
                [ScaleKind::Linear, log],
                numbers(&[0.0, 1.0]),
                numbers(&[1.0, 2.0, 3.0]),
                &[1.0; 2],
            ),
            "Axes.mesh(): the step from y_edges[1] = 2 to y_edges[2] = 3 is a factor of 1.5, \
             not 2 as from y_edges[0] to y_edges[1]; drawing the cells as one image needs \
             evenly spaced edges on the y axis.",
        ),
        (
            raster_error(
                [ScaleKind::Time, ScaleKind::Linear],
                AxisData::Times(vec![day(0), day(31), day(59), day(90)]),
                numbers(&[0.0, 1.0]),
                &[1.0; 3],
            ),
            "Axes.mesh(): the step from x_edges[1] = 1970-02-01 to x_edges[2] = 1970-03-01 is \
             28 days, not 31 days as from x_edges[0] to x_edges[1]; drawing the cells as one \
             image needs evenly spaced edges on the x axis.",
        ),
    ];
    for (err, message) in cases {
        assert!(matches!(err, Error::Usage { .. }), "{err:?}");
        assert_eq!(err.to_string(), message);
        assert_eq!(err.parameter(), Some("render"));
        assert_eq!(
            err.hint(),
            Some("use render=ub.Render.VECTOR, or evenly spaced edges.")
        );
    }
}

#[test]
fn raster_accepts_edges_even_on_screen() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let numbers = |v: Vec<f64>| AxisData::Numbers(v);
    let raster = MeshOptions {
        render: Render::Raster,
        ..MeshOptions::default()
    };
    let cases = [
        // Evenly spaced logarithms on log axes, numbers and days on linear and time axes, and
        // edges far from zero, whose steps carry rounding error.
        (
            [log, log],
            numbers(vec![1.0, 10.0, 100.0, 1000.0]),
            numbers(vec![0.5, 5.0]),
        ),
        (
            [ScaleKind::Linear, ScaleKind::Linear],
            numbers((0..=300).map(|i| 1e6 + f64::from(i) * 1e-3).collect()),
            numbers(vec![0.0, 0.1, 0.2]),
        ),
        (
            [ScaleKind::Time, ScaleKind::Linear],
            AxisData::Times((0..=3).map(day).collect()),
            numbers(vec![0.0, 1.0]),
        ),
    ];
    for (kinds, x, y) in cases {
        let cells = (x.len() - 1) * (y.len() - 1);
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(kinds[0], kinds[1])
            .unwrap()
            .mesh_time(x, y, &vec![1.0; cells], raster.clone())
            .unwrap();
        assert!(is_image(drawn(&f)));
    }
}

/// The x and y positions, in points, of every cell edge of the cell marks of `f`, and of its
/// plot's edges.
fn edge_lines(f: &Figure) -> (Vec<f64>, Vec<f64>) {
    let (resolved, layout, marks) =
        crate::pipeline::plan_panel_marks(f, FontLibrary::global()).swap_remove(0);
    let maps = AxisMaps::new(&resolved.x, &resolved.y, layout.plot);
    let place = |axis: &ResolvedAxis, edges: &Edges| -> Vec<f64> {
        match edges {
            Edges::Numbers(v) => v.iter().map(|&v| axis_value(axis, v)).collect(),
            Edges::Times(t) => t.iter().map(|&t| axis_time(axis, t)).collect(),
        }
    };
    let bands = |axis: &ResolvedAxis| -> Vec<f64> {
        match &axis.scale {
            AxisScale::Category(scale) => {
                let n = scale.categories().len();
                (0..=n).map(|i| i as f64 / n as f64).collect()
            }
            _ => Vec::new(),
        }
    };
    let plot = layout.plot;
    let (mut xs, mut ys) = (vec![plot.x, plot.right()], vec![plot.y, plot.bottom()]);
    for mark in &marks {
        let (x, y) = match mark {
            ResolvedMark::Mesh { mark, .. } => (
                place(&resolved.x, &mark.x_edges),
                place(&resolved.y, &mark.y_edges),
            ),
            ResolvedMark::Hist2d { mark, .. } => (
                mark.bins
                    .x_edges
                    .iter()
                    .map(|&v| axis_value(&resolved.x, v))
                    .collect(),
                mark.bins
                    .y_edges
                    .iter()
                    .map(|&v| axis_value(&resolved.y, v))
                    .collect(),
            ),
            ResolvedMark::Heatmap { .. } => (bands(&resolved.x), bands(&resolved.y)),
            _ => continue,
        };
        xs.extend(x.iter().map(|&u| maps.x.plot_at(u)));
        ys.extend(y.iter().map(|&u| maps.y.plot_at(u)));
    }
    (xs, ys)
}

/// Whether the device pixel `i` (covering `i..i + 1`) at `scale` pixels per point touches a
/// line at one of `lines` points, or lies within one pixel of it.
fn near(i: u32, scale: f64, lines: &[f64]) -> bool {
    let (lo, hi) = (f64::from(i), f64::from(i) + 1.0);
    lines
        .iter()
        .any(|&at| hi > at * scale - 1.0 && lo < at * scale + 1.0)
}

fn png(f: &Figure, dpi: u32) -> DecodedPng {
    decode_rgba8(&render_png(&scene(f), dpi).unwrap()).unwrap()
}

/// Checks that `raster` and `vector`, the same figure drawn as an image and as vector shapes,
/// give PNGs that agree within `tolerance` per channel at 72 and 200 dpi on every pixel away
/// from a cell or plot edge, and that most pixels of the plot were compared.
fn assert_equivalent(vector: &Figure, raster: &Figure, what: &str) {
    assert!(is_image(drawn(raster)), "{what}: not an image");
    assert!(!is_image(drawn(vector)), "{what}: not vector shapes");
    let (xs, ys) = edge_lines(vector);
    for dpi in [72, 200] {
        let (a, b) = (png(vector, dpi), png(raster, dpi));
        assert_eq!((a.width, a.height), (b.width, b.height));
        let scale = f64::from(dpi) / 72.0;
        let mut compared = 0_u64;
        for y in 0..a.height {
            if near(y, scale, &ys) {
                continue;
            }
            for x in 0..a.width {
                if near(x, scale, &xs) {
                    continue;
                }
                let (p, q) = (a.at(x, y), b.at(x, y));
                let off = p.iter().zip(q).map(|(u, v)| u.abs_diff(v)).max();
                assert!(
                    off <= Some(1),
                    "{what} at {dpi} dpi: pixel ({x}, {y}) is {q:?} as an image, {p:?} as \
                     vector shapes"
                );
                compared += 1;
            }
        }
        let total = u64::from(a.width) * u64::from(a.height);
        assert!(
            compared * 2 > total,
            "{what}: only {compared} of {total} compared"
        );
    }
}

/// `make` drawn as vector shapes and as an image.
fn both(make: impl Fn(Render) -> Figure) -> (Figure, Figure) {
    (make(Render::Vector), make(Render::Raster))
}

#[test]
fn heatmap_images_look_like_vector_cells() {
    for theme in [Theme::pigment(), Theme::midnight()] {
        let plain = |render| {
            heatmap(
                theme.clone(),
                (6, 4),
                HeatmapOptions {
                    render,
                    ..HeatmapOptions::default()
                },
            )
        };
        let log = |render| {
            heatmap(
                theme.clone(),
                (7, 5),
                HeatmapOptions {
                    scale: Transform::Log(LogBase::TEN),
                    render,
                    ..HeatmapOptions::default()
                },
            )
        };
        let centered = |render| {
            heatmap(
                theme.clone(),
                (5, 5),
                HeatmapOptions {
                    center: Some(40.0),
                    annotate: true,
                    render,
                    ..HeatmapOptions::default()
                },
            )
        };
        let (v, r) = both(plain);
        assert_equivalent(&v, &r, "heatmap");
        let (v, r) = both(log);
        assert_equivalent(&v, &r, "log heatmap");
        let (v, r) = both(centered);
        assert_equivalent(&v, &r, "centered annotated heatmap");
    }
}

#[test]
fn mesh_and_hist2d_images_look_like_vector_cells() {
    for theme in [Theme::pigment(), Theme::midnight()] {
        let (v, r) = both(|render| linear_mesh(theme.clone(), render));
        assert_equivalent(&v, &r, "mesh cut by limits");
        let (v, r) = both(|render| {
            log_mesh(
                theme.clone(),
                MeshOptions {
                    center: Some(0.0),
                    render,
                    ..MeshOptions::default()
                },
            )
        });
        assert_equivalent(&v, &r, "centered mesh on log axes");
        let (v, r) = both(|render| time_mesh(theme.clone(), render));
        assert_equivalent(&v, &r, "time mesh");
        let (v, r) = both(|render| {
            let mut f = hist2d(theme.clone(), (9, 7), render);
            f.axes_at_mut(crate::GridIndex::Auto, crate::GridIndex::Auto)
                .unwrap()
                .xlim(-3.3, 4.1)
                .unwrap()
                .ylim(-4.4, 2.9)
                .unwrap();
            f
        });
        assert_equivalent(&v, &r, "hist2d cut by limits");
    }
}

#[test]
fn image_and_vector_marks_keep_their_drawing_order() {
    // Two overlapping meshes on one axes: the second covers part of the first.
    let pair = |first: Render, second: Render| {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.mesh(
            &even(0.0, 1.0, 8),
            &even(0.0, 1.0, 6),
            &mesh_values(8, 6),
            MeshOptions {
                render: first,
                ..MeshOptions::default()
            },
        )
        .unwrap()
        .mesh(
            &even(2.5, 0.5, 6),
            &even(1.5, 0.5, 4),
            &mesh_values(6, 4),
            MeshOptions {
                render: second,
                ..MeshOptions::default()
            },
        )
        .unwrap();
        f
    };
    let vector = pair(Render::Vector, Render::Vector);
    let (xs, ys) = edge_lines(&vector);
    for (first, second) in [
        (Render::Raster, Render::Vector),
        (Render::Vector, Render::Raster),
        (Render::Raster, Render::Raster),
    ] {
        let mixed = pair(first, second);
        let items = scene(&mixed);
        let kinds: Vec<bool> = mark_items(&items)
            .iter()
            .map(|item| matches!(item, Item::Image { .. }))
            .collect();
        assert!(kinds.contains(&true));
        for dpi in [72, 200] {
            let (a, b) = (png(&vector, dpi), png(&mixed, dpi));
            let scale = f64::from(dpi) / 72.0;
            for y in (0..a.height).filter(|&y| !near(y, scale, &ys)) {
                for x in (0..a.width).filter(|&x| !near(x, scale, &xs)) {
                    let off = a
                        .at(x, y)
                        .iter()
                        .zip(b.at(x, y))
                        .map(|(u, v)| u.abs_diff(v))
                        .max();
                    assert!(off <= Some(1), "{first:?} then {second:?} at ({x}, {y})");
                }
            }
        }
    }
}

/// `base64` decoded, for the data URI of an SVG image.
fn unbase64(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0_u32;
    let mut count = 0;
    let mut out = Vec::new();
    for byte in text.bytes().filter(|&b| b != b'=') {
        let value = ALPHABET.iter().position(|&c| c == byte).unwrap() as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out
}

/// The colors of the vector cell fills of `f`, in drawing order.
fn fill_colors(f: &Figure) -> Vec<Rgba> {
    mark_items(&scene(f))
        .iter()
        .map(|item| match item {
            Item::Fill {
                paint: Paint::Solid(color),
                ..
            } => *color,
            other => panic!("expected a solid fill, got {other:?}"),
        })
        .collect()
}

#[test]
fn svg_image_holds_one_pixel_per_cell_in_the_vector_colors() {
    let cases: [(&str, (Figure, Figure)); 3] = [
        (
            "heatmap",
            both(|render| {
                heatmap(
                    Theme::midnight(),
                    (6, 4),
                    HeatmapOptions {
                        render,
                        ..HeatmapOptions::default()
                    },
                )
            }),
        ),
        ("mesh", both(|render| time_mesh(Theme::pigment(), render))),
        (
            "hist2d",
            both(|render| hist2d(Theme::pigment(), (9, 7), render)),
        ),
    ];
    for (what, (vector, raster)) in cases {
        let svg = render_svg(&scene(&raster), SvgText::Outlines);
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let images: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("image"))
            .collect();
        let [image] = &images[..] else {
            panic!("{what}: expected one image, got {}", images.len());
        };
        assert_eq!(image.attribute("preserveAspectRatio"), Some("none"));
        assert_eq!(image.attribute("image-rendering"), Some("optimizeSpeed"));
        assert_eq!(image.attribute("class"), Some("umber-raster"));
        let styles: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("style"))
            .filter_map(|n| n.text())
            .collect();
        assert_eq!(
            styles,
            [
                "@supports (image-rendering: pixelated) { .umber-raster { image-rendering: pixelated } }"
            ]
        );
        let vector_svg = render_svg(&scene(&vector), SvgText::Outlines);
        assert!(!vector_svg.contains("<style"), "{what}");
        let href = image.attribute("href").unwrap();
        let data = href.strip_prefix("data:image/png;base64,").unwrap();
        let decoded = decode_rgba8(&unbase64(data)).unwrap();
        // The vector cells are drawn row by row from the top, each row from the left, and
        // skip empty cells, as the image's opaque pixels run.
        let mut pixels = Vec::new();
        for y in 0..decoded.height {
            for x in 0..decoded.width {
                let [r, g, b, a] = decoded.at(x, y);
                match a {
                    0 => {}
                    255 => pixels.push(Rgba { r, g, b, a }),
                    _ => panic!("{what}: pixel ({x}, {y}) is partly transparent"),
                }
            }
        }
        assert_eq!(pixels, fill_colors(&vector), "{what}");
        assert!(pixels.len() > 20, "{what}");
    }
}

#[test]
fn image_output_is_deterministic() {
    let make = || linear_mesh(Theme::pigment(), Render::Raster);
    let (a, b) = (scene(&make()), scene(&make()));
    assert_eq!(a, b);
    assert_eq!(render_png(&a, 200).unwrap(), render_png(&b, 200).unwrap());
    for text in [SvgText::Outlines, SvgText::Editable] {
        assert_eq!(render_svg(&a, text), render_svg(&b, text));
    }
}

#[test]
fn cells_far_wider_than_the_plot_fall_back_to_vector_shapes() {
    // Limits inside a sliver of two cells 1e9 wide on screen: an image would place their
    // shared edge in coordinates too large for points to hold exactly.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .mesh(
            &[0.0, 1.0, 2.0],
            &[0.0, 1.0],
            &[1.0, 2.0],
            MeshOptions {
                render: Render::Raster,
                ..MeshOptions::default()
            },
        )
        .unwrap()
        .xlim(1.0 - 1e-7, 1.0 + 1e-7)
        .unwrap();
    assert_eq!(drawn(&f), Drawn::Fills(2));
}
