//! Heatmaps: one opaque rectangle per cell over its category bands, in the color its value maps
//! to on the axes' color scale, and the values written on annotated cells.
//!
//! The cells fill their bands exactly and meet without a seam, as [`cells`](super::cells)
//! describes: rows are drawn top to bottom and each row left to right, in axis order, and each
//! cell reaches under its drawn neighbours to the right and below. A cell ends exactly at its
//! bands where it has no drawn neighbour: at the heatmap's right and bottom edges, and next to
//! an empty cell.
//!
//! Drawn as one image ([`Render`](crate::Render)), a heatmap is one pixel per pair of category
//! bands from its first to its last category on each axis, over exactly those bands: a band of
//! another heatmap's category between them, and an empty cell, is a transparent pixel.

use super::MarkContext;
use super::cells::{CLEAR, Reach, cell_path, image_item, reach};
use super::chrome::text;
use super::mapping::AxisMaps;
use crate::color_scale::ResolvedColorScale;
use std::collections::HashSet;

use crate::error::quoted;
use crate::layout::{Label, Layout, centered_tick_label, widths};
use crate::model::HeatmapMark;
use crate::resolve::{AxisScale, Resolved, ResolvedMark};
use crate::scene::{FillRule, Item, Paint, Rect};
use crate::text::SceneFonts;
use crate::{CategoryScale, Error, Result, Rgba, Theme, TickFormat};

/// How far, in points, a label may exceed its cell and still count as fitting: the cell size
/// comes from two rounded edges, so a label of exactly the cell's size can measure larger by a
/// few parts in 1e13.
const FIT_TOLERANCE: f64 = 1e-6;

/// The hint of a heatmap whose labels do not fit its cells.
const LABEL_HINT: &str = "enlarge the figure, write fewer decimals with format, or pass \
                          annotate=False.";

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
    /// The indices of its y and x categories on their axes.
    at: (usize, usize),
    /// The cell's value, finite.
    value: f64,
    /// The color it maps to.
    color: Rgba,
    /// The left, top, right and bottom edges of its bands, in plot points.
    bounds: [f64; 4],
    /// How far it reaches under its neighbours to the right and below.
    reach: Reach,
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
    let drawn: Vec<Vec<_>> = row_at
        .iter()
        .map(|&row| column_at.iter().map(|&column| color(row, column)).collect())
        .collect();
    let at = |r: usize, c: usize| drawn.get(r).and_then(|row| row.get(c)).copied().flatten();
    let band_reach = |count: usize, extent: f64| reach(extent.abs() / count.max(1) as f64);
    let (reach_right, reach_down) = (
        band_reach(columns.categories().len(), maps.x.extent),
        band_reach(rows.categories().len(), maps.y.extent),
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
                at: (r, c),
                value,
                color,
                bounds: [
                    maps.x.plot_at(left),
                    maps.y.plot_at(top),
                    maps.x.plot_at(right),
                    maps.y.plot_at(bottom),
                ],
                reach: Reach {
                    right: if has_right { reach_right } else { 0.0 },
                    down: if has_below { reach_down } else { 0.0 },
                    corner: at(r + 1, c + 1).is_some(),
                },
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

/// The cells of `mark` as one image, one pixel per pair of category bands from its first to
/// its last x and y category on the axes, over those bands (see the module documentation);
/// `None` when no cell is drawn or either axis is not a category axis, which the model rules
/// out.
fn image(
    maps: AxisMaps,
    resolved: &Resolved,
    mark: &HeatmapMark,
    scale: &ResolvedColorScale,
) -> Option<Item> {
    let (AxisScale::Category(columns), AxisScale::Category(rows)) =
        (&resolved.x.scale, &resolved.y.scale)
    else {
        return None;
    };
    let column_at = mark_indices(columns, mark.x_categories());
    let row_at = mark_indices(rows, mark.y_categories());
    let span = |indices: &[Option<usize>]| {
        let first = indices.iter().position(Option::is_some)?;
        let end = indices.iter().rposition(Option::is_some)? + 1;
        Some(first..end)
    };
    let (x_span, y_span) = (span(&column_at)?, span(&row_at)?);
    let width = mark.x_categories().len();
    let color = |row: Option<usize>, column: Option<usize>| {
        let value = *mark.values().get(row? * width + column?)?;
        scale.color(value)
    };
    let pixels: Vec<Rgba> = row_at[y_span.clone()]
        .iter()
        .flat_map(|&row| {
            column_at[x_span.clone()]
                .iter()
                .map(move |&column| color(row, column).unwrap_or(CLEAR))
        })
        .collect();
    let (left, top) = (columns.band(x_span.start).0, rows.band(y_span.start).0);
    let (right, bottom) = (columns.band(x_span.end - 1).1, rows.band(y_span.end - 1).1);
    let bounds = [
        maps.x.plot_at(left),
        maps.y.plot_at(top),
        maps.x.plot_at(right),
        maps.y.plot_at(bottom),
    ];
    image_item(bounds, bounds, (x_span.len(), y_span.len()), pixels)
}

/// The cells of `mark` appended to `items`: as one image when its options draw it as one
/// ([`Render`](crate::Render)), else as opaque fills; nothing without a color scale (no finite
/// value on the axes).
pub(super) fn heatmap_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &HeatmapMark,
    items: &mut Vec<Item>,
) {
    let Some(scale) = &resolved.color else {
        return;
    };
    let count = mark
        .x_categories()
        .len()
        .saturating_mul(mark.y_categories().len());
    if mark.options().render.draws_image(count, true) {
        items.extend(image(context.maps, resolved, mark, scale));
        return;
    }
    items.extend(
        cells(context.maps, resolved, mark, scale)
            .iter()
            .map(|cell| Item::Fill {
                path: cell_path(cell.bounds, cell.reach),
                paint: Paint::Solid(cell.color),
                rule: FillRule::NonZero,
            }),
    );
}

/// The values of the annotated heatmaps among `marks`, as one group clipped to the plot area:
/// each value centered on its cell in the tick font of `theme`, shaped with `lib`, in black or
/// white by [`text_color_on`]. A heatmap writes its values in its own format, or else in the y
/// tick format `y_format` (see [`ResolvedColorScale::value_labels`]). `Ok(None)` when no cell
/// is annotated.
///
/// The labels are drawn above every cell, so a label whose cell a later heatmap covers (a drawn
/// cell of a later mark on the same pair of categories) is left out, as if each heatmap drew
/// its labels with its cells.
///
/// # Errors
///
/// `Error::Style` under `context` when a drawn label does not fit its cell: naming the widest
/// label when one is wider than the cells, else the label height when it is taller than them.
pub(crate) fn annotations(
    theme: &Theme,
    lib: &SceneFonts<'_>,
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
    (y_format, context): (&TickFormat, &'static str),
) -> Result<Option<Item>> {
    let Some(scale) = resolved.color.as_ref() else {
        return Ok(None);
    };
    let heatmaps: Vec<&HeatmapMark> = marks
        .iter()
        .filter_map(|mark| match mark {
            ResolvedMark::Heatmap { mark, .. } => Some(*mark),
            _ => None,
        })
        .collect();
    if !heatmaps.iter().any(|mark| mark.options().annotate) {
        return Ok(None);
    }
    let plot = layout.plot;
    let maps = AxisMaps::new(&resolved.x, &resolved.y, plot);
    let layers: Vec<Vec<Cell>> = heatmaps
        .iter()
        .map(|mark| cells(maps, resolved, mark, scale))
        .collect();
    let mut placed = Vec::new();
    for (i, (mark, cells)) in heatmaps.iter().zip(&layers).enumerate() {
        if !mark.options().annotate {
            continue;
        }
        let covered: HashSet<(usize, usize)> = layers[i + 1..]
            .iter()
            .flatten()
            .map(|cell| cell.at)
            .collect();
        let shown: Vec<&Cell> = cells
            .iter()
            .filter(|cell| !covered.contains(&cell.at))
            .collect();
        let values: Vec<f64> = shown.iter().map(|cell| cell.value).collect();
        let format = mark.options().format.as_ref().unwrap_or(y_format);
        let labels = scale.value_labels(&values, format);
        for (cell, label) in shown.into_iter().zip(labels) {
            let [left, top, right, bottom] = cell.bounds;
            let center = ((left + right) / 2.0, (top + bottom) / 2.0);
            let label = centered_tick_label(lib, theme, &label, center);
            placed.push((label, cell));
        }
    }
    check_fit(context, &placed)?;
    if placed.is_empty() {
        return Ok(None);
    }
    Ok(Some(Item::Group {
        clip: Some(Rect {
            x: plot.x as f32,
            y: plot.y as f32,
            width: plot.width as f32,
            height: plot.height as f32,
        }),
        items: placed
            .iter()
            .map(|(label, cell)| text(label, text_color_on(cell.color)))
            .collect(),
    }))
}

/// Checks that every label of `placed`, each with the cell it is centered on, fits inside its
/// cell's bands: `Error::Style` under `context` naming the widest label when one is wider than
/// its cell, else the label height when the labels are taller than their cells.
fn check_fit(context: &'static str, placed: &[(Label, &Cell)]) -> Result<()> {
    let size = |cell: &Cell| {
        let [left, top, right, bottom] = cell.bounds;
        (right - left, bottom - top)
    };
    let too_wide = placed
        .iter()
        .filter(|(label, cell)| label.width() > size(cell).0 + FIT_TOLERANCE)
        .max_by(|a, b| a.0.width().total_cmp(&b.0.width()));
    if let Some((label, cell)) = too_wide {
        let (width, room) = widths(label.width(), size(cell).0);
        return Err(Error::style(
            context,
            format!(
                "the heatmap label {} is {width} pt wide; its cells are {room} pt wide.",
                quoted(&label.text)
            ),
        )
        .with_hint(LABEL_HINT));
    }
    let height = |label: &Label| f64::from(label.layout.ascent() + label.layout.descent());
    if let Some((label, cell)) = placed
        .iter()
        .find(|(label, cell)| height(label) > size(cell).1 + FIT_TOLERANCE)
    {
        let (tall, room) = widths(height(label), size(cell).1);
        return Err(Error::style(
            context,
            format!("the heatmap labels are {tall} pt tall; its cells are {room} pt tall."),
        )
        .with_hint(LABEL_HINT));
    }
    Ok(())
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
    use super::super::cells::OVERLAP;
    use super::*;
    use crate::geometry::{Path, PathVerb, Point};
    use crate::model::{Figure, HeatmapOptions};
    use crate::render::render_png;
    use crate::scene::Scene;
    use crate::test_support::image::decode_rgba8;
    use crate::{Cmap, CmapChoice, FontLibrary, ScaleKind};

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

    /// Every text of `scene`, in drawing order, with its color.
    fn scene_texts(scene: &Scene) -> Vec<(String, Rgba)> {
        let mut out = Vec::new();
        let mut stack: Vec<&Item> = scene.items.iter().rev().collect();
        while let Some(item) = stack.pop() {
            match item {
                Item::Group { items, .. } => stack.extend(items.iter().rev()),
                Item::Text(text) => out.push((text.text.clone(), text.color)),
                _ => {}
            }
        }
        out
    }

    /// A figure of heatmaps added in order on one cell `a`/`r`, each `(value, annotate)`.
    fn layers(layers: &[(f64, bool)]) -> Figure {
        let mut f = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = f
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap();
        for &(value, annotate) in layers {
            let options = HeatmapOptions {
                annotate,
                format: annotate.then(|| TickFormat::fixed(3).unwrap()),
                ..HeatmapOptions::default()
            };
            axes.heatmap(&names(&["a"]), &names(&["r"]), &[value], options)
                .unwrap();
        }
        f
    }

    #[test]
    fn a_later_heatmap_covers_the_labels_under_it() {
        // An unannotated heatmap over an annotated one hides its label with its cell.
        let scene = layers(&[(0.0, true), (1.0, false)])
            .to_scene(FontLibrary::global())
            .unwrap();
        let labels: Vec<_> = scene_texts(&scene)
            .into_iter()
            .filter(|(text, _)| text.ends_with(".000"))
            .collect();
        assert_eq!(labels, []);
        // Two annotated layers show the top one's label only, in its contrast color.
        let f = layers(&[(0.0, true), (1.0, true)]);
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let top = f.theme().cmap().at(1.0);
        let labels: Vec<_> = scene_texts(&scene)
            .into_iter()
            .filter(|(text, _)| text.ends_with(".000"))
            .collect();
        assert_eq!(labels, [("1.000".to_owned(), text_color_on(top))]);
        // A NaN cell on top leaves the label under it visible.
        let scene = layers(&[(0.0, true), (f64::NAN, false)])
            .to_scene(FontLibrary::global())
            .unwrap();
        assert!(
            scene_texts(&scene).iter().any(|(text, _)| text == "0.000"),
            "{:?}",
            scene_texts(&scene)
        );
    }

    #[test]
    fn labels_wider_than_their_cells_are_refused() {
        // 30 columns on a figure 3 in wide: cells about 6 pt wide, labels about 30 pt.
        let x: Vec<String> = (0..30).map(|i| format!("c{i}")).collect();
        let x: Vec<&str> = x.iter().map(String::as_str).collect();
        let values: Vec<f64> = (0..30)
            .map(|i| if i % 2 == 0 { 12345.6789 } else { f64::NAN })
            .collect();
        let options = HeatmapOptions {
            annotate: true,
            format: Some(TickFormat::fixed(4).unwrap()),
            ..HeatmapOptions::default()
        };
        let mut f = Figure::new(3.0, 2.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap()
            .heatmap(&names(&x), &names(&["r"]), &values, options.clone())
            .unwrap();
        let err = f.to_scene(FontLibrary::global()).unwrap_err();
        assert!(matches!(err, crate::Error::Style { .. }), "{err:?}");
        let message = err.to_string();
        assert!(
            message.starts_with("Figure.to_scene(): the heatmap label \"12345.6789\" is ")
                && message.contains(" pt wide; its cells are ")
                && message.ends_with(" pt wide."),
            "{message}"
        );
        assert_eq!(err.parameter(), None);
        assert_eq!(err.hint(), Some(LABEL_HINT));
        // Without annotations, or with fewer decimals on a wider figure, it renders.
        let mut plain = Figure::new(3.0, 2.0, Theme::pigment()).unwrap();
        plain
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap()
            .heatmap(
                &names(&x),
                &names(&["r"]),
                &values,
                HeatmapOptions::default(),
            )
            .unwrap();
        assert!(plain.to_scene(FontLibrary::global()).is_ok());
    }

    #[test]
    fn a_label_fitting_its_cell_up_to_rounding_is_accepted() {
        let theme = Theme::pigment();
        let fonts = crate::test_support::figure_fonts(&theme);
        let label = centered_tick_label(&fonts, &theme, "12.5", (0.0, 0.0));
        let width = label.width();
        let height = f64::from(label.layout.ascent() + label.layout.descent());
        // The cell edges are rounded, so the size they give can differ from the label's by a
        // few parts in 1e13 although the two are equal.
        let cell_of = |width: f64, height: f64| Cell {
            at: (0, 0),
            value: 1.0,
            color: Rgba::BLACK,
            bounds: [0.0, 0.0, width, height],
            reach: Reach {
                right: 0.0,
                down: 0.0,
                corner: false,
            },
        };
        let nudge = 1e-13 * width.max(height);
        let fitting = cell_of(width - nudge, height - nudge);
        assert!(check_fit("Figure.to_scene()", &[(label.clone(), &fitting)]).is_ok());
        // A label wider or taller by a visible amount is still refused.
        let narrow = cell_of(width - 0.01, height);
        assert!(check_fit("Figure.to_scene()", &[(label.clone(), &narrow)]).is_err());
        let short = cell_of(width, height - 0.01);
        assert!(check_fit("Figure.to_scene()", &[(label, &short)]).is_err());
    }

    #[test]
    fn labels_taller_than_their_cells_are_refused() {
        let y: Vec<String> = (0..24).map(|i| format!("r{i}")).collect();
        let y: Vec<&str> = y.iter().map(String::as_str).collect();
        let f = heatmap(
            &["a"],
            &y,
            &[1.0; 24],
            HeatmapOptions {
                annotate: true,
                ..HeatmapOptions::default()
            },
        );
        let err = f.to_scene(FontLibrary::global()).unwrap_err();
        let message = err.to_string();
        assert!(
            message.starts_with("Figure.to_scene(): the heatmap labels are ")
                && message.contains(" pt tall; its cells are ")
                && message.ends_with(" pt tall."),
            "{message}"
        );
        assert_eq!(err.hint(), Some(LABEL_HINT));
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
