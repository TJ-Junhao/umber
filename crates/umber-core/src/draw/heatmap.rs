//! Heatmaps: one opaque rectangle per cell over its category bands, in the color its value maps
//! to on the axes' color scale, and the values written on annotated cells.
//!
//! The cells fill their bands exactly and meet without a seam: rows are drawn top to bottom and
//! each row left to right, in axis order, and each cell reaches up to [`OVERLAP`] under its
//! neighbours to the right and below, which are drawn after it, so their antialiased edges
//! cover it rather than letting the background show between them. A cell ends exactly at its
//! bands where it has no drawn neighbour: at the heatmap's right and bottom edges, and next to
//! an empty cell.

use super::MarkContext;
use super::chrome::text;
use super::mapping::{AxisMaps, point};
use crate::color_scale::ResolvedColorScale;
use crate::geometry::Path;
use crate::layout::{Layout, centered_tick_label};
use crate::model::HeatmapMark;
use crate::resolve::{AxisScale, Resolved, ResolvedMark};
use crate::scene::{FillRule, Item, Paint, Rect};
use crate::{CategoryScale, FontLibrary, Rgba, Theme, TickFormat};

/// How far, in points, a cell reaches under a neighbour drawn after it; at most half the
/// neighbour's size along that axis.
const OVERLAP: f64 = 1.0;

/// The text color on a cell whose black text contrasts more with it than white text would.
const DARK_TEXT: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};

/// The text color on a cell whose white text contrasts more with it than black text would.
const LIGHT_TEXT: Rgba = Rgba {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};

/// One drawn cell of a heatmap, in drawing order.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Cell {
    /// The cell's value, finite.
    value: f64,
    /// The color it maps to.
    color: Rgba,
    /// The left, top, right and bottom edges of its bands, in plot points.
    bounds: [f64; 4],
    /// How far it reaches right, under the next cell in its row: 0 without one.
    right: f64,
    /// How far it reaches down, under the next cell in its column: 0 without one.
    down: f64,
    /// Whether the cell diagonally below it to the right is drawn, so that the corner where
    /// both reaches meet lies under a cell too.
    corner: bool,
}

/// The drawn cells of `mark` on the axes `resolved`, mapped onto the plot by `maps`, colored by
/// `scale`, in drawing order. Empty when either axis is not a category axis, which the model
/// rules out.
fn cells(
    maps: AxisMaps,
    resolved: &Resolved,
    mark: &HeatmapMark,
    scale: &ResolvedColorScale,
) -> Vec<Cell> {
    let (AxisScale::Category(columns), AxisScale::Category(rows)) =
        (&resolved.x.scale, &resolved.y.scale)
    else {
        return Vec::new();
    };
    // The mark's column and row at each index of the axes, which may hold categories of other
    // heatmaps too.
    let column_at = mark_indices(columns, mark.x_categories());
    let row_at = mark_indices(rows, mark.y_categories());
    let width = mark.x_categories().len();
    let color = |row: Option<usize>, column: Option<usize>| {
        let (row, column) = (row?, column?);
        let value = *mark.values().get(row * width + column)?;
        Some((value, scale.color(value)?))
    };
    let at = |r: usize, c: usize| {
        color(
            row_at.get(r).copied().flatten(),
            column_at.get(c).copied().flatten(),
        )
    };
    let reach = |count: usize, extent: f64| OVERLAP.min(extent.abs() / count.max(1) as f64 / 2.0);
    let (reach_right, reach_down) = (
        reach(columns.categories().len(), maps.x.extent),
        reach(rows.categories().len(), maps.y.extent),
    );
    let mut cells = Vec::new();
    for r in 0..rows.categories().len() {
        for c in 0..columns.categories().len() {
            let Some((value, color)) = at(r, c) else {
                continue;
            };
            let ((left, right), (top, bottom)) = (columns.band(c), rows.band(r));
            let has_right = at(r, c + 1).is_some();
            let has_below = at(r + 1, c).is_some();
            cells.push(Cell {
                value,
                color,
                bounds: [
                    maps.x.plot_at(left),
                    maps.y.plot_at(top),
                    maps.x.plot_at(right),
                    maps.y.plot_at(bottom),
                ],
                right: if has_right { reach_right } else { 0.0 },
                down: if has_below { reach_down } else { 0.0 },
                corner: at(r + 1, c + 1).is_some(),
            });
        }
    }
    cells
}

/// For each index of the category axis `axis`, the index of that category among `names`, the
/// categories of one mark; `None` for a category the mark does not have.
fn mark_indices(axis: &CategoryScale, names: &[String]) -> Vec<Option<usize>> {
    let mut indices = vec![None; axis.categories().len()];
    for (i, name) in names.iter().enumerate() {
        if let Some(slot) = axis.index_of(name).and_then(|at| indices.get_mut(at)) {
            *slot = Some(i);
        }
    }
    indices
}

/// The outline of `cell`: its bands, widened by its reach to the right and down. When it
/// reaches both ways but no cell lies diagonally below it, the corner where the two reaches
/// meet is cut out, so nothing of the cell shows over the empty cell there.
fn cell_path(cell: &Cell) -> Path {
    let [left, top, right, bottom] = cell.bounds;
    let (far_right, far_bottom) = (right + cell.right, bottom + cell.down);
    let corners: Vec<(f64, f64)> = if cell.right > 0.0 && cell.down > 0.0 && !cell.corner {
        vec![
            (left, top),
            (far_right, top),
            (far_right, bottom),
            (right, bottom),
            (right, far_bottom),
            (left, far_bottom),
        ]
    } else {
        vec![
            (left, top),
            (far_right, top),
            (far_right, far_bottom),
            (left, far_bottom),
        ]
    };
    let mut path = Path::new();
    for (i, &(x, y)) in corners.iter().enumerate() {
        if i == 0 {
            path.move_to(point(x, y));
        } else {
            path.line_to(point(x, y));
        }
    }
    path.close();
    path
}

/// The cells of `mark` as opaque fills, appended to `items`; nothing without a color scale
/// (no finite value on the axes).
pub(super) fn heatmap_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &HeatmapMark,
    items: &mut Vec<Item>,
) {
    let Some(scale) = &resolved.color else {
        return;
    };
    items.extend(
        cells(context.maps, resolved, mark, scale)
            .iter()
            .map(|cell| Item::Fill {
                path: cell_path(cell),
                paint: Paint::Solid(cell.color),
                rule: FillRule::NonZero,
            }),
    );
}

/// The values of the annotated heatmaps among `marks`, as one group clipped to the plot area:
/// each value centered on its cell in the tick font of `theme`, shaped with `lib`, in black or
/// white by [`text_color_on`]. A heatmap writes its values in its own format, or else in the y
/// tick format `y_format` (see [`ResolvedColorScale::value_labels`]). `None` when no cell is
/// annotated.
pub(crate) fn annotations(
    theme: &Theme,
    lib: &FontLibrary,
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
    y_format: &TickFormat,
) -> Option<Item> {
    let scale = resolved.color.as_ref()?;
    let plot = layout.plot;
    let maps = AxisMaps::new(&resolved.x, &resolved.y, plot);
    let mut items = Vec::new();
    for mark in marks {
        let ResolvedMark::Heatmap { mark, .. } = mark else {
            continue;
        };
        if !mark.options().annotate {
            continue;
        }
        let cells = cells(maps, resolved, mark, scale);
        let values: Vec<f64> = cells.iter().map(|cell| cell.value).collect();
        let format = mark.options().format.as_ref().unwrap_or(y_format);
        let labels = scale.value_labels(&values, format);
        for (cell, label) in cells.iter().zip(labels) {
            let [left, top, right, bottom] = cell.bounds;
            let center = ((left + right) / 2.0, (top + bottom) / 2.0);
            let placed = centered_tick_label(lib, theme, &label, center);
            items.push(text(&placed, text_color_on(cell.color)));
        }
    }
    if items.is_empty() {
        return None;
    }
    Some(Item::Group {
        clip: Some(Rect {
            x: plot.x as f32,
            y: plot.y as f32,
            width: plot.width as f32,
            height: plot.height as f32,
        }),
        items,
    })
}

/// Black or white, whichever has the higher WCAG 2 contrast ratio with the opaque `fill`;
/// black on a tie.
fn text_color_on(fill: Rgba) -> Rgba {
    let luminance = relative_luminance(fill);
    if (luminance + 0.05) / 0.05 >= 1.05 / (luminance + 0.05) {
        DARK_TEXT
    } else {
        LIGHT_TEXT
    }
}

/// The WCAG 2 relative luminance of `color`'s sRGB channels, from 0 (black) to 1 (white).
fn relative_luminance(color: Rgba) -> f64 {
    let linear = |channel: u8| {
        let c = f64::from(channel) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PathVerb, Point};
    use crate::model::{Figure, HeatmapOptions};
    use crate::render::render_png;
    use crate::scene::Scene;
    use crate::test_support::image::decode_rgba8;
    use crate::{Cmap, CmapChoice, ScaleKind};

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    /// A figure of one heatmap of `x` by `y` categories with `values`, row by row.
    fn heatmap(x: &[&str], y: &[&str], values: &[f64], options: HeatmapOptions) -> Figure {
        let mut f = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap()
            .heatmap(&names(x), &names(y), values, options)
            .unwrap();
        f
    }

    /// The corners of a path of straight edges, in order.
    fn corners(path: &Path) -> Vec<(f64, f64)> {
        path.verbs()
            .iter()
            .filter_map(|verb| match *verb {
                PathVerb::MoveTo(p) | PathVerb::LineTo(p) => Some((f64::from(p.x), f64::from(p.y))),
                _ => None,
            })
            .collect()
    }

    /// The corners and the color of each fill of a marks group.
    type Fills = Vec<(Vec<(f64, f64)>, Rgba)>;

    /// The fills of the marks group of the one axes of `f`, with the layout they are drawn on.
    fn cell_fills(f: &Figure) -> (Layout, Fills) {
        let (resolved, layout) = crate::pipeline::plan_figure(f, (0, 0), FontLibrary::global());
        let marks = crate::resolve::resolve_figure_marks(f).swap_remove(0);
        let fills = match crate::draw::marks(f.theme(), &resolved, &layout, &marks) {
            Some(Item::Group { items, .. }) => items
                .iter()
                .map(|item| match item {
                    Item::Fill {
                        path,
                        paint: Paint::Solid(color),
                        ..
                    } => (corners(path), *color),
                    other => panic!("expected a solid fill, got {other:?}"),
                })
                .collect(),
            None => Vec::new(),
            other => panic!("expected a marks group, got {other:?}"),
        };
        (layout, fills)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn cells_fill_bands_exactly() {
        let f = heatmap(
            &["a", "b", "c"],
            &["p", "q"],
            &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            HeatmapOptions::default(),
        );
        let (layout, fills) = cell_fills(&f);
        let plot = layout.plot;
        let (w, h) = (plot.width / 3.0, plot.height / 2.0);
        assert_eq!(fills.len(), 6, "{fills:?}");
        for (i, (corners, _)) in fills.iter().enumerate() {
            // x runs left to right and y top to bottom, row by row.
            let (row, column) = (i / 3, i % 3);
            let left = plot.x + column as f64 * w;
            let top = plot.y + row as f64 * h;
            let reach_right = if column < 2 { OVERLAP } else { 0.0 };
            let reach_down = if row < 1 { OVERLAP } else { 0.0 };
            let expected = [
                (left, top),
                (left + w + reach_right, top),
                (left + w + reach_right, top + h + reach_down),
                (left, top + h + reach_down),
            ];
            assert_eq!(corners.len(), 4, "cell {i}: {corners:?}");
            for (got, want) in corners.iter().zip(expected) {
                assert!(
                    close(got.0, want.0) && close(got.1, want.1),
                    "cell {i}: {corners:?}, expected {expected:?}"
                );
            }
        }
        // The last cell ends at the plot's bottom right corner: the heatmap fills the plot.
        let last = &fills[5].0;
        assert!(close(last[2].0, plot.right()) && close(last[2].1, plot.bottom()));
        // The colors follow the values: the first cell is the low end of the colormap.
        let cmap = f.theme().cmap();
        assert_eq!(fills[0].1, cmap.at(0.0));
        assert_eq!(fills[5].1, cmap.at(1.0));
    }

    #[test]
    fn cells_meet_without_seams() {
        // Every cell has the same color, so any pixel inside the plot that is not that color
        // shows the background through a seam between two cells.
        let f = heatmap(
            &["a", "b", "c"],
            &["p", "q", "r"],
            &[7.0; 9],
            HeatmapOptions::default(),
        );
        let color = f.theme().cmap().at(0.5);
        let (layout, _) = cell_fills(&f);
        let scene: Scene = f.to_scene(FontLibrary::global()).unwrap();
        for dpi in [72, 200] {
            let image = decode_rgba8(&render_png(&scene, dpi).unwrap()).unwrap();
            let scale = f64::from(dpi) / 72.0;
            let plot = layout.plot;
            let (x0, x1) = (
                (plot.x * scale).ceil() as u32,
                (plot.right() * scale).floor() as u32,
            );
            let (y0, y1) = (
                (plot.y * scale).ceil() as u32,
                (plot.bottom() * scale).floor() as u32,
            );
            for y in y0..y1 {
                for x in x0..x1 {
                    let pixel = image.at(x, y);
                    let off = [color.r, color.g, color.b, 255]
                        .iter()
                        .zip(pixel)
                        .map(|(&want, got)| want.abs_diff(got))
                        .max();
                    // A seam would show the background over a quarter of the pixel.
                    assert!(
                        off <= Some(2),
                        "pixel ({x}, {y}) at {dpi} dpi is {pixel:?}, not {color:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn nan_cell_empty() {
        // The middle cell of the top row is NaN: no fill covers it, and its neighbours do not
        // reach into it.
        let f = heatmap(
            &["a", "b", "c"],
            &["p", "q"],
            &[0.0, f64::NAN, 2.0, 3.0, 4.0, 5.0],
            HeatmapOptions::default(),
        );
        let (layout, fills) = cell_fills(&f);
        let plot = layout.plot;
        let (w, h) = (plot.width / 3.0, plot.height / 2.0);
        assert_eq!(fills.len(), 5);
        let hole = (plot.x + w, plot.y, plot.x + 2.0 * w, plot.y + h);
        for (corners, _) in &fills {
            let (left, right) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                (lo.min(p.0), hi.max(p.0))
            });
            let (top, bottom) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                (lo.min(p.1), hi.max(p.1))
            });
            let overlaps = left < hole.2 - 1e-6
                && right > hole.0 + 1e-6
                && top < hole.3 - 1e-6
                && bottom > hole.1 + 1e-6;
            assert!(!overlaps, "{corners:?} covers the empty cell {hole:?}");
        }
        // The cell below the hole reaches neither up nor into it; the first cell, left of the
        // hole, keeps its reach down but not to the right, under the hole.
        let first = &fills[0].0;
        assert!(first.iter().all(|p| p.0 <= plot.x + w + 1e-6), "{first:?}");
        assert!(
            first.iter().any(|p| close(p.1, plot.y + h + OVERLAP)),
            "{first:?}"
        );
    }

    #[test]
    fn corner_under_an_empty_cell_is_cut_out() {
        // The bottom right cell is NaN: the top left cell reaches right and down, but not into
        // the corner over that empty cell.
        let f = heatmap(
            &["a", "b"],
            &["p", "q"],
            &[1.0, 2.0, 3.0, f64::NAN],
            HeatmapOptions::default(),
        );
        let (layout, fills) = cell_fills(&f);
        let plot = layout.plot;
        let (w, h) = (plot.width / 2.0, plot.height / 2.0);
        let first = &fills[0].0;
        let (right, bottom) = (plot.x + w, plot.y + h);
        let expected = [
            (plot.x, plot.y),
            (right + OVERLAP, plot.y),
            (right + OVERLAP, bottom),
            (right, bottom),
            (right, bottom + OVERLAP),
            (plot.x, bottom + OVERLAP),
        ];
        assert_eq!(first.len(), 6, "{first:?}");
        for (got, want) in first.iter().zip(expected) {
            assert!(close(got.0, want.0) && close(got.1, want.1), "{first:?}");
        }
    }

    #[test]
    fn annotation_contrast_flips() {
        // A dark and a light end of a fixed colormap: white text on the dark cell, black on
        // the light one.
        let dark = Rgba {
            r: 0x10,
            g: 0x10,
            b: 0x30,
            a: 255,
        };
        let light = Rgba {
            r: 0xF0,
            g: 0xF0,
            b: 0xD0,
            a: 255,
        };
        assert_eq!(text_color_on(dark), LIGHT_TEXT);
        assert_eq!(text_color_on(light), DARK_TEXT);
        // The WCAG crossover: luminance sqrt(1.05 * 0.05) - 0.05, about 0.179, lies between
        // the grays 117 (0.178) and 118 (0.181).
        let gray = |level| Rgba {
            r: level,
            g: level,
            b: level,
            a: 255,
        };
        assert_eq!(text_color_on(gray(117)), LIGHT_TEXT);
        assert_eq!(text_color_on(gray(118)), DARK_TEXT);

        let cmap = Cmap::from_colors(&[dark, light]).unwrap();
        let f = heatmap(
            &["low", "high"],
            &["row"],
            &[0.0, 1.0],
            HeatmapOptions {
                cmap: CmapChoice::Fixed(cmap),
                annotate: true,
                format: Some(TickFormat::fixed(1).unwrap()),
                ..HeatmapOptions::default()
            },
        );
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let notes: Vec<_> = scene
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Group {
                    clip: Some(_),
                    items,
                } => Some(items),
                _ => None,
            })
            .nth(1)
            .expect("an annotation group after the marks group")
            .iter()
            .map(|item| match item {
                Item::Text(text) => (text.text.clone(), text.color, text.origin),
                other => panic!("expected text, got {other:?}"),
            })
            .collect();
        assert_eq!(
            notes
                .iter()
                .map(|n| (n.0.as_str(), n.1))
                .collect::<Vec<_>>(),
            [("0.0", LIGHT_TEXT), ("1.0", DARK_TEXT)]
        );
        // Each label is centered on its cell.
        let (layout, _) = cell_fills(&f);
        let plot = layout.plot;
        for (i, (_, _, origin)) in notes.iter().enumerate() {
            let center = plot.x + (i as f64 + 0.5) * plot.width / 2.0;
            assert!(f64::from(origin.x) < center && f64::from(origin.x) > center - 20.0);
            let Point { y, .. } = *origin;
            assert!(f64::from(y) > plot.y + plot.height / 2.0, "{origin:?}");
        }
    }

    #[test]
    fn automatic_annotations_follow_the_color_ticks() {
        let labels = |values: &[f64], options: HeatmapOptions| {
            let x: Vec<String> = (0..values.len()).map(|i| format!("c{i}")).collect();
            let x: Vec<&str> = x.iter().map(String::as_str).collect();
            let f = heatmap(
                &x,
                &["r"],
                values,
                HeatmapOptions {
                    annotate: true,
                    ..options
                },
            );
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let group = scene
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Group {
                        clip: Some(_),
                        items,
                    } => Some(items),
                    _ => None,
                })
                .nth(1)
                .expect("an annotation group");
            group
                .iter()
                .map(|item| match item {
                    Item::Text(text) => text.text.clone(),
                    other => panic!("expected text, got {other:?}"),
                })
                .collect::<Vec<_>>()
        };
        // Ticks of 0 to 1 are 0.25 apart: two decimals, whatever the value's own digits.
        assert_eq!(
            labels(&[0.0, 0.7341, 1.0], HeatmapOptions::default()),
            ["0.00", "0.73", "1.00"]
        );
        // A centered scale over -1 to 1 ticks every 0.5, so one decimal too; negatives use
        // U+2212.
        assert_eq!(
            labels(
                &[-0.84, 0.26, 1.0],
                HeatmapOptions {
                    center: Some(0.0),
                    ..HeatmapOptions::default()
                }
            ),
            ["\u{2212}0.8", "0.3", "1.0"]
        );
        // Whole-number ticks give whole numbers; NaN cells have no label.
        assert_eq!(
            labels(&[12.0, f64::NAN, 96.4], HeatmapOptions::default()),
            ["12", "96"]
        );
    }

    #[test]
    fn two_category_axes_draw_no_baseline() {
        let f = heatmap(&["a"], &["p"], &[1.0], HeatmapOptions::default());
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let groups = scene
            .items
            .iter()
            .filter(|item| matches!(item, Item::Group { .. }))
            .count();
        // Chrome, marks and the colorbar; no baseline group.
        assert_eq!(groups, 3, "{:#?}", scene.items);
    }
}
