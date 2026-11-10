//! Level labels of contours on grids and meshes as drawn: their color on the bands they lie
//! across, and their position inside the plot area and the field, clear of holes and edges.

use super::super::contour::label_color;
use super::super::contrast::contrast_ratio;
use crate::layout::Area;
use crate::pipeline::plan_figure;
use crate::scene::{Item, Scene};
use crate::{
    ContourLevels, ContourOptions, ContourStyle, Figure, FontLibrary, LogBase, PathVerb, Rgba,
    ScaleKind, SvgText, Theme, Triangles,
};

/// The gap kept clear around a label, in points, as the contour draws it.
const GAP: f64 = 3.0;

fn figure(theme: Theme) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    f
}

fn scene(f: &Figure) -> Scene {
    f.to_scene(FontLibrary::global()).unwrap()
}

/// The items of the clipped marks group.
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
        .expect("a marks group")
}

/// A level label as drawn: its text, color and the bounds of its outline on the page.
#[derive(Debug, Clone, PartialEq)]
struct Drawn {
    text: String,
    color: Rgba,
    bounds: [f64; 4],
}

/// The level labels among the marks of `scene`.
fn labels(scene: &Scene) -> Vec<Drawn> {
    marks(scene)
        .iter()
        .filter_map(|item| match item {
            Item::RotatedText { text, .. } => {
                let (low, high) = text.outline.bounds()?;
                Some(Drawn {
                    text: text.text.clone(),
                    color: text.color,
                    bounds: [low.x, low.y, high.x, high.y].map(f64::from),
                })
            }
            _ => None,
        })
        .collect()
}

/// The subpaths of every stroke among the marks of `scene`, as points.
fn subpaths(scene: &Scene) -> Vec<Vec<(f64, f64)>> {
    let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
    for item in marks(scene) {
        let Item::Stroke { path, .. } = item else {
            continue;
        };
        for verb in path.verbs() {
            match *verb {
                PathVerb::MoveTo(p) => out.push(vec![(f64::from(p.x), f64::from(p.y))]),
                PathVerb::LineTo(p) => {
                    if let Some(last) = out.last_mut() {
                        last.push((f64::from(p.x), f64::from(p.y)));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The plot area of the only axes of `f`.
fn plot(f: &Figure) -> Area {
    plan_figure(f, (0, 0), FontLibrary::global()).1.plot
}

/// The page rectangle `[left, top, right, bottom]` of the data rectangle from `(x0, y0)` to
/// `(x1, y1)` on a plot area showing `limits` (x from, x to, y from, y to) on linear axes.
fn on_page(plot: Area, limits: [f64; 4], [x0, y0, x1, y1]: [f64; 4]) -> [f64; 4] {
    let [left, right, bottom, top] = limits;
    let x = |v: f64| plot.x + (v - left) / (right - left) * plot.width;
    let y = |v: f64| plot.bottom() - (v - bottom) / (top - bottom) * plot.height;
    [x(x0), y(y1), x(x1), y(y0)]
}

/// Whether the rectangles `a` and `b` overlap.
fn overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// `bounds` widened by `by` on every side.
fn widened(bounds: [f64; 4], by: f64) -> [f64; 4] {
    [
        bounds[0] - by,
        bounds[1] - by,
        bounds[2] + by,
        bounds[3] + by,
    ]
}

/// The coordinates 0 to 20 in steps of 1.
fn coordinates() -> Vec<f64> {
    (0..=20).map(f64::from).collect()
}

/// A 21 × 21 grid of `z = y`, NaN at the grid points `holes` (column, row).
fn rising(holes: &[(usize, usize)]) -> Vec<f64> {
    let mut z: Vec<f64> = (0..21).flat_map(|j| [f64::from(j); 21]).collect();
    for &(i, j) in holes {
        z[j * 21 + i] = f64::NAN;
    }
    z
}

/// Lines of `levels` with labels.
fn labelled(levels: &[f64]) -> ContourOptions {
    ContourOptions {
        levels: ContourLevels::Values(levels.to_vec()),
        annotate: true,
        ..ContourOptions::default()
    }
}

/// The figure of a grid contour of `z` on [`coordinates`] with `options`.
fn grid_contour(z: &[f64], options: ContourOptions) -> Figure {
    let mut f = figure(Theme::pigment());
    let xy = coordinates();
    f.single_axes_mut().contour(&xy, &xy, z, options).unwrap();
    f
}

/// The points of a 21 × 21 lattice on [`coordinates`], and its unit squares cut into two
/// triangles each, those whose centre lies in one of the data rectangles `holes`
/// (`[x0, y0, x1, y1]`) left out.
fn lattice(holes: &[[f64; 4]]) -> (Vec<f64>, Vec<f64>, Vec<[i64; 3]>) {
    let x: Vec<f64> = (0..21).flat_map(|_| coordinates()).collect();
    let y: Vec<f64> = (0..21).flat_map(|j| [f64::from(j); 21]).collect();
    let mut triangles = Vec::new();
    for j in 0..20_i64 {
        for i in 0..20_i64 {
            let at = |i: i64, j: i64| j * 21 + i;
            for corners in [
                [at(i, j), at(i + 1, j), at(i + 1, j + 1)],
                [at(i, j), at(i + 1, j + 1), at(i, j + 1)],
            ] {
                let centre = corners.iter().fold((0.0, 0.0), |(cx, cy), &k| {
                    (cx + x[k as usize] / 3.0, cy + y[k as usize] / 3.0)
                });
                let inside = holes.iter().any(|&[x0, y0, x1, y1]| {
                    x0 < centre.0 && centre.0 < x1 && y0 < centre.1 && centre.1 < y1
                });
                if !inside {
                    triangles.push(corners);
                }
            }
        }
    }
    (x, y, triangles)
}

/// The figure of a mesh contour of `z = y` on [`lattice`] without the triangles in `holes`.
fn mesh_contour(holes: &[[f64; 4]], options: ContourOptions) -> Figure {
    let (x, y, triangles) = lattice(holes);
    let mut f = figure(Theme::pigment());
    f.single_axes_mut()
        .tricontour(&x, &y, Triangles::Given(&triangles), &y, options)
        .unwrap();
    f
}

/// Asserts that the one label of `f` lies clear of the data rectangle `hole` and away from the
/// middle of its line, which would put it over the hole.
fn assert_moved_clear_of(f: &Figure, hole: [f64; 4]) {
    let drawn = scene(f);
    let found = labels(&drawn);
    let [label] = found.as_slice() else {
        panic!("one label: {found:?}")
    };
    let plot = plot(f);
    let hole = on_page(plot, [0.0, 20.0, 0.0, 20.0], hole);
    assert!(
        !overlap(widened(label.bounds, GAP), hole),
        "{label:?} {hole:?}"
    );
    let centre = (label.bounds[0] + label.bounds[2]) / 2.0;
    let middle = plot.x + plot.width / 2.0;
    assert!((centre - middle).abs() > 10.0, "{label:?} at {middle}");
    // At the middle, the label would reach the hole.
    let half = (label.bounds[2] - label.bounds[0]) / 2.0;
    let there = [
        middle - half,
        label.bounds[1],
        middle + half,
        label.bounds[3],
    ];
    assert!(overlap(widened(there, GAP), hole), "{there:?} {hole:?}");
}

#[test]
fn a_grid_label_that_would_reach_a_hole_moves_along_its_line() {
    // NaN at (10, 12) cuts out the cells from x = 9 to 11 and y = 11 to 13, a quarter of a
    // cell above the line along y = 10.75.
    let f = grid_contour(&rising(&[(10, 12)]), labelled(&[10.75]));
    assert_moved_clear_of(&f, [9.0, 11.0, 11.0, 13.0]);
}

#[test]
fn a_mesh_label_that_would_reach_a_filtered_hole_moves_along_its_line() {
    let hole = [9.0, 11.0, 11.0, 13.0];
    let f = mesh_contour(&[hole], labelled(&[10.75]));
    assert_moved_clear_of(&f, hole);
}

/// Asserts that `f` draws no label and its one line whole, across the plot.
fn assert_dropped(f: &Figure) {
    let drawn = scene(f);
    assert!(labels(&drawn).is_empty(), "{:?}", labels(&drawn));
    let pieces = subpaths(&drawn);
    let [line] = pieces.as_slice() else {
        panic!("one piece: {pieces:?}")
    };
    let plot = plot(f);
    let xs: Vec<f64> = line.iter().map(|p| p.0).collect();
    let (low, high) = xs
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
            (lo.min(x), hi.max(x))
        });
    assert!((low - plot.x).abs() < 1e-3 && (high - plot.right()).abs() < 1e-3);
}

#[test]
fn a_grid_label_with_no_room_beside_its_holes_is_dropped() {
    // NaN along rows 9 and 12 leaves one row of cells, from y = 10 to 11, about 11 pt high:
    // too low for a label and its gap anywhere along the line through it.
    let holes: Vec<(usize, usize)> = (0..21).flat_map(|i| [(i, 9), (i, 12)]).collect();
    assert_dropped(&grid_contour(&rising(&holes), labelled(&[10.5])));
}

#[test]
fn a_mesh_label_with_no_room_beside_its_holes_is_dropped() {
    let holes = [[-1.0, 8.0, 21.0, 10.0], [-1.0, 11.0, 21.0, 13.0]];
    assert_dropped(&mesh_contour(&holes, labelled(&[10.5])));
}

#[test]
fn labels_keep_inside_the_plot_area_and_the_field() {
    // The field reaches past the top of the plot: the line 0.3 below the plot's top edge
    // (about 4 pt) has no room for its label; the one in the middle keeps it.
    let mut f = grid_contour(&rising(&[]), labelled(&[7.5, 14.7]));
    f.single_axes_mut().ylim(0.0, 15.0).unwrap();
    let drawn = scene(&f);
    let found: Vec<String> = labels(&drawn).into_iter().map(|label| label.text).collect();
    assert_eq!(found, ["7.5"], "{found:?}");
    // The plot reaches past the top of the field: the line 0.2 below the field's top edge has
    // no room either, while every label stays inside the field.
    let mut f = grid_contour(&rising(&[]), labelled(&[10.0, 19.8]));
    f.single_axes_mut().ylim(-5.0, 25.0).unwrap();
    let drawn = scene(&f);
    let found = labels(&drawn);
    assert_eq!(found.len(), 1, "{found:?}");
    let field = on_page(plot(&f), [0.0, 20.0, -5.0, 25.0], [0.0, 0.0, 20.0, 20.0]);
    for label in &found {
        let [left, top, right, bottom] = widened(label.bounds, GAP);
        assert!(
            field[0] < left && field[1] < top && right < field[2] && bottom < field[3],
            "{label:?} {field:?}"
        );
    }
    // The same on a mesh.
    let mut f = mesh_contour(&[], labelled(&[7.5, 14.7]));
    f.single_axes_mut().ylim(0.0, 15.0).unwrap();
    let found: Vec<String> = labels(&scene(&f))
        .into_iter()
        .map(|label| label.text)
        .collect();
    assert_eq!(found, ["7.5"], "{found:?}");
}

#[test]
fn labels_are_placed_the_same_every_time() {
    let holes: Vec<(usize, usize)> = [(10, 12), (4, 6), (15, 3)].to_vec();
    let grid = grid_contour(&rising(&holes), labelled(&[2.5, 5.5, 10.5, 13.5]));
    let mesh = mesh_contour(
        &[[9.0, 11.0, 11.0, 13.0], [3.0, 5.0, 5.0, 7.0]],
        labelled(&[2.5, 5.5, 10.5, 13.5]),
    );
    for f in [grid, mesh] {
        let first = scene(&f);
        assert!(!labels(&first).is_empty());
        assert_eq!(scene(&f), first);
        assert_eq!(
            f.to_svg(SvgText::Editable).unwrap(),
            f.to_svg(SvgText::Editable).unwrap()
        );
    }
}

/// A smooth bump on a 41 × 31 grid, filled and lined at nine levels across its range, with
/// labels, in `theme`.
fn filled(theme: Theme, style: ContourStyle) -> Figure {
    let x: Vec<f64> = (0..41).map(|i| f64::from(i) / 10.0 - 2.0).collect();
    let y: Vec<f64> = (0..31).map(|j| f64::from(j) / 10.0 - 1.5).collect();
    let z: Vec<f64> = y
        .iter()
        .flat_map(|&y| x.iter().map(move |&x| (-(x * x) / 1.5 - y * y).exp()))
        .collect();
    let mut f = figure(theme);
    f.single_axes_mut()
        .contour(
            &x,
            &y,
            &z,
            ContourOptions {
                levels: ContourLevels::Values((1..10).map(|k| f64::from(k) / 10.0).collect()),
                style,
                annotate: true,
                ..ContourOptions::default()
            },
        )
        .unwrap();
    f
}

#[test]
fn labels_on_bands_take_the_text_or_background_color_that_reads_on_both() {
    for theme in [Theme::pigment(), Theme::midnight()] {
        let f = filled(theme.clone(), ContourStyle::Both);
        let (resolved, _) = plan_figure(&f, (0, 0), FontLibrary::global());
        let scale = resolved.color.unwrap();
        let levels: Vec<f64> = (1..10).map(|k| f64::from(k) / 10.0).collect();
        let (text, background) = (theme.text_color(), theme.background());
        let drawn = scene(&f);
        let found = labels(&drawn);
        assert!(found.len() >= 5, "{found:?}");
        let mut colors = Vec::new();
        for label in &found {
            let index = levels
                .iter()
                .position(|&level| format!("{level:.1}") == label.text)
                .unwrap();
            let bands = [
                scale.band_color(index.checked_sub(1).map(|i| levels[i]), Some(levels[index])),
                scale.band_color(Some(levels[index]), levels.get(index + 1).copied()),
            ];
            let least = |color: Rgba| {
                bands
                    .iter()
                    .map(|&band| contrast_ratio(color, band))
                    .fold(f64::INFINITY, f64::min)
            };
            if label.color == text {
                assert!(least(text) >= least(background), "{label:?}");
            } else {
                assert_eq!(label.color, background, "{label:?}");
                assert!(least(background) > least(text), "{label:?}");
            }
            colors.push(label.color);
        }
        // The bump's top bands are light in one theme and dark in the other: both colors occur.
        assert!(colors.contains(&text), "{colors:?}");
        assert!(colors.contains(&background), "{colors:?}");
    }
    // On Midnight the lightest bands, at the top of the bump, take the dark background.
    let f = filled(Theme::midnight(), ContourStyle::Both);
    let top = labels(&scene(&f))
        .into_iter()
        .find(|label| label.text == "0.9")
        .unwrap();
    assert_eq!(top.color, Theme::midnight().background());
}

#[test]
fn labels_on_lines_keep_the_text_color() {
    for theme in [Theme::pigment(), Theme::midnight()] {
        let f = filled(theme.clone(), ContourStyle::Lines);
        let found = labels(&scene(&f));
        assert!(!found.is_empty());
        assert!(
            found.iter().all(|label| label.color == theme.text_color()),
            "{found:?}"
        );
    }
}

#[test]
fn a_tie_between_the_text_and_background_colors_takes_the_text_color() {
    let theme = Theme::pigment();
    let (text, background) = (theme.text_color(), theme.background());
    // Across a band in each color, both reach only 1:1 against one of them.
    assert_eq!(label_color(&theme, &[text, background]), text);
    assert_eq!(label_color(&theme, &[background, text]), text);
    // Equal colors tie on any band.
    let same = theme.with_text_color(background).unwrap();
    assert_eq!(
        label_color(&same, &[Rgba::opaque(0x33, 0x66, 0x99); 2]),
        background
    );
    // Without bands, the text color; on two dark bands, or one, in Pigment, the background.
    assert_eq!(
        label_color(&theme, &[Rgba::opaque(0x10, 0x10, 0x20)]),
        background
    );
    assert_eq!(label_color(&theme, &[]), text);
    let dark = Rgba::opaque(0x10, 0x10, 0x20);
    assert_eq!(label_color(&theme, &[dark, dark]), background);
    // A transparent background is taken opaque.
    let clear = theme
        .with_background(Rgba {
            a: 0,
            ..Rgba::opaque(0xFF, 0xFF, 0xFF)
        })
        .unwrap();
    assert_eq!(
        label_color(&clear, &[dark, dark]),
        Rgba::opaque(0xFF, 0xFF, 0xFF)
    );
}

/// The subpaths of the first fill among the marks of `scene`: the band below every level.
fn first_band(scene: &Scene) -> Vec<Vec<(f64, f64)>> {
    let path = marks(scene)
        .iter()
        .find_map(|item| match item {
            Item::Fill { path, .. } => Some(path),
            _ => None,
        })
        .expect("a band");
    let mut rings: Vec<Vec<(f64, f64)>> = Vec::new();
    for verb in path.verbs() {
        match *verb {
            PathVerb::MoveTo(p) => rings.push(vec![(f64::from(p.x), f64::from(p.y))]),
            PathVerb::LineTo(p) => {
                if let Some(last) = rings.last_mut() {
                    last.push((f64::from(p.x), f64::from(p.y)));
                }
            }
            _ => {}
        }
    }
    rings
}

/// Whether `(x, y)` lies inside `rings` by the even-odd rule, counted by ray crossings.
fn inside_even_odd(rings: &[Vec<(f64, f64)>], (x, y): (f64, f64)) -> bool {
    let mut odd = false;
    for ring in rings {
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
            if (a.1 > y) != (b.1 > y) && a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) > x {
                odd = !odd;
            }
        }
    }
    odd
}

#[test]
fn a_label_on_a_mesh_folded_by_a_log_axis_stays_inside_the_field_as_drawn() {
    // A mesh valid in the data that a log x axis folds: the middle of its line, near
    // (31.6, 0.575), lies outside the field as drawn, which starts at y = 0.75 there, so the
    // label that would sit there clear of every edge moves to where the drawn field holds it.
    let x = [1.0, 1.0, 100.0, 50.0];
    let y = [0.0, 1.0, 1.0, 0.51];
    let z: Vec<f64> = x.iter().map(|v: &f64| v.log10()).collect();
    let mut f = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
    f.add_axes(ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear)
        .unwrap()
        .tricontour(
            &x,
            &y,
            Triangles::Auto,
            &z,
            ContourOptions {
                levels: ContourLevels::Values(vec![1.5]),
                style: ContourStyle::Both,
                annotate: true,
                ..ContourOptions::default()
            },
        )
        .unwrap();
    let drawn = scene(&f);
    let band = first_band(&drawn);
    let found = labels(&drawn);
    assert!(found.len() <= 1, "{found:?}");
    for label in &found {
        let [left, top, right, bottom] = widened(label.bounds, GAP);
        let centre = ((left + right) / 2.0, (top + bottom) / 2.0);
        for point in [
            centre,
            (left, top),
            (right, top),
            (right, bottom),
            (left, bottom),
        ] {
            assert!(inside_even_odd(&band, point), "{label:?} at {point:?}");
        }
    }
}

/// A ridge on a 3 × 2 grid that only reaches 1 along its middle, filled with lines and labels
/// at `levels` on Midnight.
fn ridge(levels: &[f64]) -> Figure {
    let mut f = figure(Theme::midnight());
    f.single_axes_mut()
        .contour(
            &[0.0, 1.0, 2.0],
            &[0.0, 1.0],
            &[0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
            ContourOptions {
                levels: ContourLevels::Values(levels.to_vec()),
                style: ContourStyle::Both,
                annotate: true,
                ..ContourOptions::default()
            },
        )
        .unwrap();
    f
}

#[test]
fn a_label_with_nothing_painted_above_it_takes_the_color_of_the_band_below() {
    // At level 1 the region at or above it is the ridge line alone: no band is painted above
    // the label, so only the dark band below it counts and the light text color reads there.
    let f = ridge(&[1.0]);
    let (resolved, _) = plan_figure(&f, (0, 0), FontLibrary::global());
    let scale = resolved.color.unwrap();
    let below = scale.band_color(None, Some(1.0));
    let theme = Theme::midnight();
    let found = labels(&scene(&f));
    assert!(!found.is_empty());
    for label in &found {
        assert_eq!(label.color, theme.text_color(), "{label:?}");
        assert_eq!(label.color, label_color(&theme, &[below]));
    }
    // Against the unpainted light band too, the background would have been chosen.
    let above = scale.band_color(Some(1.0), None);
    assert_eq!(label_color(&theme, &[below, above]), theme.background());
}

#[test]
fn a_label_at_the_lowest_level_counts_both_painted_bands_beside_it() {
    // With a level below the ridge's top, the lowest label lies across two painted bands, the
    // whole field below it and the band from 0.5 to 1 above; the top label counts only the
    // band below it.
    let f = ridge(&[0.5, 1.0]);
    let (resolved, _) = plan_figure(&f, (0, 0), FontLibrary::global());
    let scale = resolved.color.unwrap();
    let theme = Theme::midnight();
    let lowest = label_color(
        &theme,
        &[
            scale.band_color(None, Some(0.5)),
            scale.band_color(Some(0.5), Some(1.0)),
        ],
    );
    let top = label_color(&theme, &[scale.band_color(Some(0.5), Some(1.0))]);
    let found = labels(&scene(&f));
    let colors = |text: &str| -> Vec<Rgba> {
        found
            .iter()
            .filter(|label| label.text == text)
            .map(|label| label.color)
            .collect()
    };
    let (low_colors, top_colors) = (colors("0.5"), colors("1.0"));
    assert!(
        !low_colors.is_empty() && !top_colors.is_empty(),
        "{found:?}"
    );
    assert!(low_colors.iter().all(|&c| c == lowest), "{found:?}");
    assert!(top_colors.iter().all(|&c| c == top), "{found:?}");
}
