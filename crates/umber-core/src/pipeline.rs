//! The render pipeline: from a [`Figure`] to its [`Scene`].
//!
//! Scales, ticks and the layout depend on each other: ticks depend on the plot size, and the
//! plot size depends on the tick label widths. [`plan`] settles them in two passes for one
//! axes in its frame; [`scene`] then draws each axes' chrome, marks, legend and baseline.

use crate::error::Error;
use crate::layout::{self, Edges, Frame, Layout, LayoutInput, MeasuredLegend};
use crate::legend::Legend;
use crate::model::{Axes, Figure};
use crate::resolve::{self, Limits, Resolved, ResolvedMark};
use crate::scene::Scene;
use crate::{FontLibrary, Result, Theme, draw};

/// Resolves and lays out the axes of `input` in `frame`, whose sides on the canvas edge `edges`
/// names, in two passes: ticks depend on the plot size, and the plot size depends on the tick
/// label widths. The first pass assumes the plot fills the frame inside the padding; the
/// second uses the plot size of the first, and is final. `marks` are the axes' marks, resolved
/// once by the caller; each pass resolves the axes from them.
///
/// # Errors
///
/// As [`layout::lay_out_in`], from either pass: the second pass needs the plot size of the
/// first.
pub(crate) fn plan(
    input: &LayoutInput<'_>,
    marks: &[ResolvedMark<'_>],
    frame: Frame,
    edges: Edges,
) -> Result<(Resolved, Layout)> {
    let limits = Limits::of(input.axes);
    let guess = layout::initial_plot_size(frame.width, frame.height);
    let (tx, ty) = layout::tick_targets(guess);
    let first = resolve::resolve(marks, &limits, tx, ty);
    let first = layout::lay_out_in(input, &first, frame, edges)?;
    let (tx, ty) = layout::tick_targets(first.plot.size());
    let resolved = resolve::resolve(marks, &limits, tx, ty);
    let layout = layout::lay_out_in(input, &resolved, frame, edges)?;
    Ok((resolved, layout))
}

/// The legend of `axes` with its resolved `marks`, its row and its direct labels, measured once
/// for a `width` pt canvas. The legend uses the style set on the axes, else the theme's.
fn measure_legend<'a>(
    width: f64,
    theme: &Theme,
    axes: &Axes,
    marks: &[ResolvedMark<'a>],
    lib: &FontLibrary,
) -> MeasuredLegend<'a> {
    let style = axes.legend_style().unwrap_or(theme.legend_style());
    let legend = Legend::new(style, marks, &Limits::of(axes));
    MeasuredLegend::measure(width, theme, &legend, lib)
}

/// `err`, a layout error of the axes whose top-left cell is at `row`, `col`, naming that cell
/// first on a grid of more than one cell; unchanged on a 1 × 1 grid.
fn panel_error(figure: &Figure, axes: &Axes, err: Error) -> Error {
    let grid = figure.grid();
    if grid.rows() * grid.cols() == 1 {
        return err;
    }
    let cells = axes.cells();
    err.with_message_prefix(&format!("row {}, col {}: ", cells.row(), cells.col()))
}

/// [`plan`] for the axes of `figure` whose top-left cell is at `cell`, at the figure's size,
/// with its marks resolved against its theme. Panics when the cell holds no axes or the axes
/// cannot be laid out.
#[cfg(test)]
pub(crate) fn plan_figure(
    figure: &Figure,
    cell: (usize, usize),
    lib: &FontLibrary,
) -> (Resolved, Layout) {
    try_plan_figure(figure, cell, lib).expect("the figure lays out")
}

/// [`plan`] for the axes of `figure` whose top-left cell is at `cell`, as [`plan_figure`], with
/// layout errors reported in `"Figure.to_scene()"`, named by the cell as [`scene`] names them.
/// Panics when the cell holds no axes.
#[cfg(test)]
pub(crate) fn try_plan_figure(
    figure: &Figure,
    cell: (usize, usize),
    lib: &FontLibrary,
) -> Result<(Resolved, Layout)> {
    with_layout_input(figure, cell, lib, |input, marks, frame, edges| {
        plan(input, marks, frame, edges).map_err(|err| panel_error(figure, input.axes, err))
    })
}

/// `f` called with the layout input of the axes of `figure` whose top-left cell is at `cell`,
/// at the figure's size, with layout errors reported in `"Figure.to_scene()"`, with the axes'
/// marks resolved against the figure's theme, and with the axes' frame and edges. Panics when
/// the cell holds no axes.
#[cfg(test)]
pub(crate) fn with_layout_input<R>(
    figure: &Figure,
    (row, col): (usize, usize),
    lib: &FontLibrary,
    f: impl FnOnce(&LayoutInput<'_>, &[ResolvedMark<'_>], Frame, Edges) -> R,
) -> R {
    use crate::model::GridIndex;
    let theme = figure.theme();
    let at = |i: usize| GridIndex::At(i64::try_from(i).expect("an index on the grid"));
    let axes = figure
        .axes_at(at(row), at(col))
        .expect("the cell holds axes");
    let (width, height) = (figure.width_in() * 72.0, figure.height_in() * 72.0);
    let marks = resolve::resolve_marks(axes.marks(), theme);
    let legend = measure_legend(width, theme, axes, &marks, lib);
    let input = LayoutInput {
        width,
        theme,
        axes,
        legend: &legend,
        lib,
        context: "Figure.to_scene()",
    };
    let (frame, edges) = layout::panel_frame(width, height, figure.grid(), axes.cells());
    f(&input, &marks, frame, edges)
}

/// The scene of `figure`: `width × 72` by `height × 72` pt on the theme background, holding
/// for each placed axes, in row-major order of their top-left cells,
/// - the axes chrome without the baseline, as one unclipped group;
/// - the visible marks, in insertion order, as one group clipped to the plot area (widened
///   horizontally for line ends, see `draw::marks_clip`);
/// - the legend, when it has row entries or direct labels, as one unclipped group;
/// - the baseline, as an unclipped group of its own.
///
/// Each axes is laid out on its own in its frame ([`layout::panel_frame`]); a figure of one
/// axes has the canvas as its frame. Without axes the scene holds the background only. Each
/// axes' marks are resolved and its legend is built and measured once, before layout, and the
/// same records serve layout and drawing.
///
/// # Errors
///
/// As [`plan`], reported in `context`, the API call that renders, for the first axes that
/// cannot be laid out; on a grid of more than one cell the message starts with the axes'
/// top-left cell, `row {r}, col {c}: `.
pub(crate) fn scene(
    figure: &Figure,
    library: &FontLibrary,
    context: &'static str,
) -> Result<Scene> {
    let theme = figure.theme();
    let (width, height) = (figure.width_in() * 72.0, figure.height_in() * 72.0);
    let mut scene = Scene::new(width as f32, height as f32, theme.background());
    for axes in figure.panels() {
        let marks = resolve::resolve_marks(axes.marks(), theme);
        let legend = measure_legend(width, theme, axes, &marks, library);
        let input = LayoutInput {
            width,
            theme,
            axes,
            legend: &legend,
            lib: library,
            context,
        };
        let (frame, edges) = layout::panel_frame(width, height, figure.grid(), axes.cells());
        let (resolved, layout) =
            plan(&input, &marks, frame, edges).map_err(|err| panel_error(figure, axes, err))?;
        scene.push(draw::chrome(theme, &layout));
        if let Some(marks) = draw::marks(theme, &resolved, &layout, &marks) {
            scene.push(marks);
        }
        if let Some(legend) = draw::legend::legend(theme, &layout) {
            scene.push(legend);
        }
        scene.push(draw::baseline(theme, &layout));
    }
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Grid, GridIndex, LineOptions, Placement};
    use crate::scene::Item;
    use crate::{LegendStyle, ScaleKind};

    /// A figure in `theme` with a title and two labelled lines.
    fn labelled_figure(theme: Theme) -> Figure {
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.title("Station output");
        for label in ["North", "South"] {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: label.to_owned(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        f
    }

    /// The number of legend entries laid out, and the number of top-level groups in the scene:
    /// chrome, marks and baseline, plus one with a legend.
    fn legend_and_groups(f: &Figure) -> (usize, usize) {
        let lib = FontLibrary::global();
        let entries = plan_figure(f, (0, 0), lib).1.legend.entries().len();
        let groups = scene(f, lib, "Figure.to_scene()")
            .unwrap()
            .items
            .iter()
            .filter(|item| matches!(item, Item::Group { .. }))
            .count();
        (entries, groups)
    }

    #[test]
    fn header_theme_shows_the_legend_between_marks_and_baseline() {
        let f = labelled_figure(Theme::pigment());
        assert_eq!(legend_and_groups(&f), (2, 4));
        let s = scene(&f, FontLibrary::global(), "Figure.to_scene()").unwrap();
        // Chrome, marks, legend (unclipped), baseline.
        assert!(
            matches!(s.items.get(2), Some(Item::Group { clip: None, items }) if items.len() == 4)
        );
    }

    #[test]
    fn theme_none_hides_the_legend() {
        let theme = Theme::pigment()
            .with_legend_style(LegendStyle::None)
            .unwrap();
        assert_eq!(theme.legend_style(), LegendStyle::None);
        assert_eq!(legend_and_groups(&labelled_figure(theme)), (0, 3));
    }

    #[test]
    fn axes_none_hides_the_legend_under_a_header_theme() {
        let mut f = labelled_figure(Theme::pigment());
        f.single_axes_mut().legend(LegendStyle::None);
        assert_eq!(legend_and_groups(&f), (0, 3));
        // Without a legend the figure draws as if its marks were unlabelled.
        let mut unlabelled = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = unlabelled
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.title("Station output");
        for _ in 0..2 {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions::default(),
            )
            .unwrap();
        }
        let lib = FontLibrary::global();
        let scene_of = |f: &Figure| scene(f, lib, "Figure.to_scene()").unwrap();
        assert_eq!(scene_of(&f), scene_of(&unlabelled));
    }

    #[test]
    fn axes_header_shows_the_legend_under_a_none_theme() {
        let none = Theme::pigment()
            .with_legend_style(LegendStyle::None)
            .unwrap();
        let mut f = labelled_figure(none);
        f.single_axes_mut().legend(LegendStyle::Header);
        assert_eq!(legend_and_groups(&f), (2, 4));
        // The same scene as under a theme whose own style is the header row.
        let scene_of = |f: &Figure| scene(f, FontLibrary::global(), "Figure.to_scene()").unwrap();
        assert_eq!(scene_of(&f), scene_of(&labelled_figure(Theme::pigment())));
    }

    #[test]
    fn axes_style_replaces_the_theme_style_and_itself() {
        // A theme with direct labels: no row entries, both lines labelled directly, in a
        // legend group.
        let mut f = labelled_figure(Theme::graphite());
        let direct = |f: &Figure| plan_figure(f, (0, 0), FontLibrary::global()).1.direct.len();
        assert_eq!(legend_and_groups(&f), (0, 4));
        assert_eq!(direct(&f), 2);
        // The axes ask for the header row.
        f.single_axes_mut().legend(LegendStyle::Header);
        assert_eq!(legend_and_groups(&f), (2, 4));
        assert_eq!(direct(&f), 0);
        // Calling it again replaces the style.
        f.single_axes_mut().legend(LegendStyle::None);
        assert_eq!(legend_and_groups(&f), (0, 3));
        assert_eq!(direct(&f), 0);
    }

    #[test]
    fn tick_targets_follow_plot_size() {
        // The second pass uses the first pass's plot size: a taller figure gets more y ticks.
        let ticks = |w: f64, h: f64| {
            let mut f = Figure::new(w, h, Theme::graphite()).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .ylim(0.0, 100.0)
                .unwrap()
                .xlim(0.0, 100.0)
                .unwrap();
            let (resolved, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
            let targets = layout::tick_targets(layout.plot.size());
            (resolved.x.ticks.len(), resolved.y.ticks.len(), targets)
        };
        let (sx, sy, small) = ticks(2.0, 1.5);
        let (lx, ly, large) = ticks(12.0, 9.0);
        assert_eq!(small, (3, 3));
        assert_eq!(large, (8, 8));
        assert!(lx > sx && ly > sy, "{sx} {sy} {lx} {ly}");
    }

    /// A `rows × cols` figure at 6 × 4 in per cell, with a titled line in each of `cells`.
    fn grid_of_lines(rows: i64, cols: i64, cells: &[(i64, i64)]) -> Figure {
        let grid = Grid::new(rows, cols).unwrap();
        let (width, height) = grid.auto_size_in();
        let mut f = Figure::with_grid(width, height, Theme::pigment(), grid).unwrap();
        for &(row, col) in cells {
            let at = Placement {
                row: GridIndex::At(row),
                col: GridIndex::At(col),
                ..Placement::default()
            };
            f.add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .title(&format!("{row}, {col}"))
                .line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[1.0, 2.0],
                    LineOptions::default(),
                )
                .unwrap();
        }
        f
    }

    #[test]
    fn each_axes_is_laid_out_in_its_own_cells() {
        // 2 × 2 at 12 × 8 in: cells are 420 × 276 pt inside the 12 pt padding; (1, 1) is empty.
        let f = grid_of_lines(2, 2, &[(1, 0), (0, 0), (0, 1)]);
        let lib = FontLibrary::global();
        for (row, col) in [(0, 0), (0, 1), (1, 0)] {
            let (_, layout) = plan_figure(&f, (row, col), lib);
            let (left, top) = (12.0 + 420.0 * col as f64, 12.0 + 276.0 * row as f64);
            let plot = layout.plot;
            assert!(
                plot.x > left && plot.right() <= left + 420.0,
                "{row}, {col}: {plot:?}"
            );
            assert!(
                plot.y > top && plot.bottom() < top + 276.0,
                "{row}, {col}: {plot:?}"
            );
            let title = layout.title.expect("a title");
            assert_eq!(title.x, left, "{row}, {col}");
            assert!(title.y > top && title.y < plot.y, "{row}, {col}");
        }
        // Chrome, marks and baseline for each axes, in row-major order; nothing for the empty
        // cell.
        let s = scene(&f, lib, "Figure.to_scene()").unwrap();
        let groups = s
            .items
            .iter()
            .filter(|item| matches!(item, Item::Group { .. }))
            .count();
        assert_eq!((s.items.len(), groups), (9, 9));
        let chrome_titles: Vec<bool> = s
            .items
            .iter()
            .step_by(3)
            .zip(["0, 0", "0, 1", "1, 0"])
            .map(|(chrome, title)| {
                let Item::Group { items, .. } = chrome else {
                    return false;
                };
                items
                    .iter()
                    .any(|item| matches!(item, Item::Text(text) if text.text == title))
            })
            .collect();
        assert_eq!(chrome_titles, [true; 3]);
    }

    #[test]
    fn layout_errors_name_the_cell_on_a_grid_only() {
        let wide = |f: &mut Figure, row, col| {
            f.axes_at_mut(GridIndex::At(row), GridIndex::At(col))
                .unwrap()
                .yticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(60)])
                .unwrap();
        };
        let lib = FontLibrary::global();
        let mut single = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        single
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        wide(&mut single, 0, 0);
        let alone = scene(&single, lib, "Figure.to_svg()").unwrap_err();
        assert!(
            alone
                .to_string()
                .starts_with("Figure.to_svg(): y tick label \""),
            "{alone}"
        );
        let mut grid = grid_of_lines(1, 3, &[(0, 0), (0, 2)]);
        wide(&mut grid, 0, 2);
        let err = scene(&grid, lib, "Figure.to_svg()").unwrap_err();
        assert!(
            err.to_string()
                .starts_with("Figure.to_svg(): row 0, col 2: y tick label \""),
            "{err}"
        );
        assert_eq!(err.hint(), alone.hint());
        assert_eq!(
            try_plan_figure(&grid, (0, 2), lib).unwrap_err().to_string(),
            err.to_string()
                .replace("Figure.to_svg()", "Figure.to_scene()")
        );
    }
}
