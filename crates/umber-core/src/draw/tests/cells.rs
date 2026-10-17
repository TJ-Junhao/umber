//! Mesh, 2-D histogram and hexbin cells: drawn neighbours meet without a seam, and nothing of
//! a cell covers an empty cell or grows past the outer edges of the drawn cells.
//!
//! Each figure draws its cells in one color, so any pixel well inside the drawn cells that is
//! not that color shows the background through a seam. The exact cell outlines come from the
//! mark's edges or lattice, mapped as the axes map them, independently of how the cells are
//! drawn.

use super::super::mapping::{AxisMaps, axis_value};
use super::subpaths;
use crate::bin2d::{HexBins, hex_neighbours};
use crate::layout::Layout;
use crate::model::Figure;
use crate::render::render_png;
use crate::resolve::ResolvedMark;
use crate::scene::{Item, Paint, Scene};
use crate::test_support::image::decode_rgba8;
use crate::test_support::polygon::{bounds, edges, inside, segment_distance};
use crate::{
    Cmap, CmapChoice, FontLibrary, HexbinOptions, Hist2dOptions, LogBase, MeshOptions, Rgba,
    ScaleKind, Theme,
};

/// A polygon in plot points.
type Polygon = Vec<(f64, f64)>;

/// The one color every cell is drawn in.
const CELL: Rgba = Rgba {
    r: 0x3C,
    g: 0x7A,
    b: 0xB4,
    a: 255,
};

/// A colormap that maps every value to [`CELL`].
fn one_color() -> CmapChoice {
    CmapChoice::Fixed(Cmap::from_colors(&[CELL, CELL]).unwrap())
}

/// The exact outlines of the cells of the one mark of `f`: those drawn, and those left empty
/// beside them, with the layout they are drawn on.
struct Exact {
    layout: Layout,
    drawn: Vec<Polygon>,
    empty: Vec<Polygon>,
}

fn rectangle([left, right]: [f64; 2], [top, bottom]: [f64; 2]) -> Polygon {
    vec![(left, top), (right, top), (right, bottom), (left, bottom)]
}

fn exact(f: &Figure) -> Exact {
    let (resolved, layout, marks) =
        crate::pipeline::plan_panel_marks(f, FontLibrary::global()).swap_remove(0);
    let maps = AxisMaps::new(&resolved.x, &resolved.y, layout.plot);
    let xs = |edges: &[f64]| -> Vec<f64> {
        edges
            .iter()
            .map(|&v| maps.x.plot_at(axis_value(&resolved.x, v)))
            .collect()
    };
    let ys = |edges: &[f64]| -> Vec<f64> {
        edges
            .iter()
            .map(|&v| maps.y.plot_at(axis_value(&resolved.y, v)))
            .collect()
    };
    let (mut drawn, mut empty) = (Vec::new(), Vec::new());
    match &marks[..] {
        [ResolvedMark::Mesh { mark, .. }] => {
            let (x, y) = (xs(mark.x_edges.numbers()), ys(mark.y_edges.numbers()));
            let columns = x.len() - 1;
            for (i, value) in mark.colors.values.iter().enumerate() {
                let (c, r) = (i % columns, i / columns);
                let cell = rectangle([x[c], x[c + 1]], [y[r + 1], y[r]]);
                if value.is_nan() {
                    empty.push(cell);
                } else {
                    drawn.push(cell);
                }
            }
        }
        [ResolvedMark::Hist2d { mark, .. }] => {
            let (x, y) = (xs(&mark.bins.x_edges), ys(&mark.bins.y_edges));
            for r in 0..y.len() - 1 {
                for c in 0..x.len() - 1 {
                    let cell = rectangle([x[c], x[c + 1]], [y[r + 1], y[r]]);
                    if mark.bins.cells.contains(&(c, r)) {
                        drawn.push(cell);
                    } else {
                        empty.push(cell);
                    }
                }
            }
        }
        [
            ResolvedMark::Hexbin {
                bins: HexBins::Occupied(bins),
                ..
            },
        ] => {
            let hexagon = |position: [i64; 2]| -> Polygon {
                bins.vertices(position)
                    .iter()
                    .map(|&[x, y]| {
                        (
                            maps.x.plot_at(axis_value(&resolved.x, x)),
                            maps.y.plot_at(axis_value(&resolved.y, y)),
                        )
                    })
                    .collect()
            };
            for &position in &bins.positions {
                drawn.push(hexagon(position));
                for around in hex_neighbours(position) {
                    if !bins.positions.contains(&around) {
                        empty.push(hexagon(around));
                    }
                }
            }
        }
        other => panic!("expected one counted cell mark, got {other:?}"),
    }
    Exact {
        layout,
        drawn,
        empty,
    }
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

/// The outlines of the cell fills of `f`, each checked to be in [`CELL`].
fn fills(f: &Figure) -> Vec<Polygon> {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    mark_items(&scene)
        .iter()
        .map(|item| match item {
            Item::Fill { path, paint, .. } => {
                assert_eq!(*paint, Paint::Solid(CELL));
                let [outline] = &subpaths(path)[..] else {
                    panic!("expected one outline, got {path:?}");
                };
                outline
                    .iter()
                    .map(|p| (f64::from(p.x), f64::from(p.y)))
                    .collect()
            }
            other => panic!("expected a cell fill, got {other:?}"),
        })
        .collect()
}

/// Whether `p` lies inside `polygon`, whose bounds are `[x0, x1, y0, y1]`, by the nonzero
/// winding rule.
fn inside_bounded(polygon: &[(f64, f64)], [x0, x1, y0, y1]: [f64; 4], p: (f64, f64)) -> bool {
    p.0 >= x0 && p.0 <= x1 && p.1 >= y0 && p.1 <= y1 && inside(polygon, p)
}

/// The edges of the drawn cells that no other drawn cell shares: the outline of the drawn
/// region, facing the background.
fn outer_edges(drawn: &[Polygon]) -> Vec<((f64, f64), (f64, f64))> {
    let same = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
    let mut out = Vec::new();
    for (i, cell) in drawn.iter().enumerate() {
        for (a, b) in edges(cell) {
            let shared = drawn
                .iter()
                .enumerate()
                .any(|(j, other)| j != i && edges(other).any(|(c, d)| same(a, d) && same(b, c)));
            if !shared {
                out.push((a, b));
            }
        }
    }
    out
}

/// Checks that every pixel of `f` whose center lies inside a drawn cell, and whose square
/// keeps clear of the outline of the drawn region and of the plot's edges, is [`CELL`]: no
/// seam between two drawn cells shows the background.
fn assert_seamless(f: &Figure, exact: &Exact) {
    let outer = outer_edges(&exact.drawn);
    let drawn: Vec<_> = exact
        .drawn
        .iter()
        .map(|cell| (cell, bounds(cell)))
        .collect();
    let plot = exact.layout.plot;
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let mut checked = 0;
    for dpi in [72, 200] {
        let image = decode_rgba8(&render_png(&scene, dpi).unwrap()).unwrap();
        let scale = f64::from(dpi) / 72.0;
        // Half a pixel's diagonal, and a little more, in points.
        let clearance = 0.75 / scale;
        for y in 0..image.height {
            for x in 0..image.width {
                let center = ((f64::from(x) + 0.5) / scale, (f64::from(y) + 0.5) / scale);
                let in_plot = center.0 > plot.x + 3.0
                    && center.0 < plot.right() - 3.0
                    && center.1 > plot.y + 3.0
                    && center.1 < plot.bottom() - 3.0;
                if !in_plot
                    || !drawn
                        .iter()
                        .any(|&(cell, b)| inside_bounded(cell, b, center))
                    || outer
                        .iter()
                        .any(|&(a, b)| segment_distance(center, a, b) < clearance)
                {
                    continue;
                }
                checked += 1;
                let pixel = image.at(x, y);
                let off = [CELL.r, CELL.g, CELL.b, 255]
                    .iter()
                    .zip(pixel)
                    .map(|(&want, got)| want.abs_diff(got))
                    .max();
                assert!(
                    off <= Some(2),
                    "pixel ({x}, {y}) at {dpi} dpi is {pixel:?}, not {CELL:?}"
                );
            }
        }
    }
    assert!(checked > 1000, "only {checked} pixels checked");
}

/// Checks that no fill of `f` covers any point inside an empty cell, and that every corner of
/// every fill lies in a drawn cell or on its outline: no cell grows past the drawn region.
fn assert_exact_outline(f: &Figure, exact: &Exact) {
    assert!(!exact.empty.is_empty(), "the figure has no empty cell");
    let fills = fills(f);
    assert_eq!(fills.len(), exact.drawn.len());
    let bounded: Vec<_> = fills.iter().map(|fill| (fill, bounds(fill))).collect();
    // Finer than any reach, so a reach into a hole covers some of these points.
    let step = 0.2;
    for hole in &exact.empty {
        let [x0, x1, y0, y1] = bounds(hole);
        let steps = |lo: f64, hi: f64| ((hi - lo) / step).ceil() as usize;
        for i in 0..=steps(x0, x1) {
            for j in 0..=steps(y0, y1) {
                let p = (
                    (x0 + i as f64 * step).min(x1),
                    (y0 + j as f64 * step).min(y1),
                );
                // Points on or next to the hole's outline belong to its neighbours too.
                if !inside(hole, p) || edges(hole).any(|(a, b)| segment_distance(p, a, b) < 1e-3) {
                    continue;
                }
                if let Some((fill, _)) = bounded
                    .iter()
                    .find(|&&(fill, b)| inside_bounded(fill, b, p))
                {
                    panic!("{fill:?} covers {p:?} in the empty cell {hole:?}");
                }
            }
        }
    }
    for fill in &fills {
        for &corner in fill {
            let within = exact.drawn.iter().any(|cell| {
                inside(cell, corner)
                    || edges(cell).any(|(a, b)| segment_distance(corner, a, b) < 1e-3)
            });
            assert!(
                within,
                "{corner:?} of {fill:?} lies outside the drawn cells"
            );
        }
    }
}

fn themes() -> [Theme; 2] {
    [Theme::pigment(), Theme::midnight()]
}

/// A 4 by 3 mesh of uneven cells on `x` and `y` axes, one color, with the middle cell of the
/// second row and the top right cell empty.
fn mesh(theme: Theme, x: ScaleKind, y: ScaleKind) -> Figure {
    let mut f = Figure::new(5.0, 4.0, theme).unwrap();
    let mut values = [7.0; 12];
    values[5] = f64::NAN;
    values[11] = f64::NAN;
    f.add_axes(x, y)
        .unwrap()
        .mesh(
            &[1.0, 2.0, 3.5, 4.0, 9.0],
            &[1.0, 1.7, 4.0, 6.0],
            &values,
            MeshOptions {
                cmap: one_color(),
                ..MeshOptions::default()
            },
        )
        .unwrap();
    f
}

#[test]
fn mesh_cells_meet_without_seams() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    for theme in themes() {
        for (x, y) in [(ScaleKind::Linear, ScaleKind::Linear), (log, log)] {
            let f = mesh(theme.clone(), x, y);
            let exact = exact(&f);
            assert_seamless(&f, &exact);
            assert_exact_outline(&f, &exact);
        }
    }
}

#[test]
fn hist2d_cells_meet_without_seams() {
    // One point in each bin of a 5 by 4 grid but two, which stay empty.
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for row in 0..4 {
        for column in 0..5 {
            if (column, row) != (2, 1) && (column, row) != (4, 2) {
                x.push(1.0 + f64::from(column));
                y.push(1.0 + f64::from(row));
            }
        }
    }
    for theme in themes() {
        let mut f = Figure::new(5.0, 4.0, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .hist2d(
                &x,
                &y,
                Hist2dOptions {
                    bins: (5, 4),
                    cmap: one_color(),
                    ..Hist2dOptions::default()
                },
            )
            .unwrap();
        let exact = exact(&f);
        assert_eq!(exact.drawn.len(), 18);
        assert_seamless(&f, &exact);
        assert_exact_outline(&f, &exact);
    }
}

#[test]
fn hexbin_cells_meet_without_seams() {
    // A dense field of points with a round hole in it, so that most hexagons have drawn
    // neighbours all around and some border empty cells.
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for i in 0..120 {
        for j in 0..90 {
            let (u, v) = (1.0 + f64::from(i) * 0.05, 1.0 + f64::from(j) * 0.05);
            if (u - 3.0).hypot(v - 3.2) > 0.8 {
                x.push(u);
                y.push(v);
            }
        }
    }
    for theme in themes() {
        let mut f = Figure::new(5.0, 4.0, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .hexbin(
                &x,
                &y,
                HexbinOptions {
                    gridsize: 9,
                    cmap: one_color(),
                    ..HexbinOptions::default()
                },
            )
            .unwrap();
        let exact = exact(&f);
        assert_seamless(&f, &exact);
        assert_exact_outline(&f, &exact);
    }
}

#[test]
fn a_lone_hexagon_keeps_its_exact_outline() {
    // Two points far apart: two hexagons with no drawn neighbour, drawn exactly.
    let mut f = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .hexbin(
            &[0.0, 10.0],
            &[0.0, 10.0],
            HexbinOptions {
                gridsize: 5,
                cmap: one_color(),
                ..HexbinOptions::default()
            },
        )
        .unwrap();
    let exact = exact(&f);
    let fills = fills(&f);
    assert_eq!(fills.len(), 2);
    for (fill, cell) in fills.iter().zip(&exact.drawn) {
        assert_eq!(fill.len(), 6, "{fill:?}");
        for (got, want) in fill.iter().zip(cell) {
            assert!(
                (got.0 - want.0).abs() < 1e-3 && (got.1 - want.1).abs() < 1e-3,
                "{fill:?} is not {cell:?}"
            );
        }
    }
}
