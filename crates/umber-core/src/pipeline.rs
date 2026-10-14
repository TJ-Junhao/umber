//! The render pipeline: from a [`Figure`] to its [`Scene`].
//!
//! Scales, ticks and the layout depend on each other: ticks depend on the plot size, and the
//! plot size depends on the tick label widths. [`plan`] settles them in two passes for every
//! panel of a figure at once; [`scene`] then draws each panel's chrome, marks, legend and
//! baseline.

use crate::layout::{
    self, GridInput, Layout, LayoutInput, MeasuredLegend, PanelBands, PlotSize, TickSpacing,
};
use crate::legend::Legend;
use crate::model::{Axes, Figure};
use crate::resolve::{self, Limits, Resolved, ResolvedMark};
use crate::scene::Scene;
use crate::{FontLibrary, Result, Theme, draw};

/// One panel's records that layout and drawing share: its axes, its marks resolved against the
/// theme, and its legend, measured. They are made once, before layout.
struct Panel<'a> {
    /// The axes.
    axes: &'a Axes,
    /// The axes' marks, resolved.
    marks: Vec<ResolvedMark<'a>>,
    /// The axes' legend, measured.
    legend: MeasuredLegend<'a>,
}

/// The records of every panel of `figure`, in the order of [`Figure::panels`]. The legend uses
/// the style set on the axes, else the theme's.
fn panels<'a>(figure: &'a Figure, lib: &FontLibrary) -> Vec<Panel<'a>> {
    let theme = figure.theme();
    figure
        .panels()
        .map(|axes| {
            let marks = resolve::resolve_marks(axes.marks(), theme);
            let legend = measure_legend(theme, axes, &marks, lib);
            Panel {
                axes,
                marks,
                legend,
            }
        })
        .collect()
}

/// The legend of `axes` with its resolved `marks`, its row entries and its direct labels,
/// measured once. The legend uses the style set on the axes, else the theme's.
fn measure_legend<'a>(
    theme: &Theme,
    axes: &Axes,
    marks: &[ResolvedMark<'a>],
    lib: &FontLibrary,
) -> MeasuredLegend<'a> {
    let style = axes.legend_style().unwrap_or(theme.legend_style());
    let legend = Legend::new(style, marks, &Limits::of(axes));
    MeasuredLegend::measure(theme, &legend, lib)
}

/// Resolves and lays out every panel of `figure`, whose records are `panels`, in two passes: ticks
/// depend on the plot size, and the plot size depends on the tick label widths. The first pass
/// assumes each plot fills its cells inside the padding (the whole canvas inside the padding for a
/// figure of one cell); the second uses each panel's plot size from the first, and is final. Each
/// pass resolves every panel from its marks for its tick rooms ([`layout::tick_rooms`]: the first
/// for the tick targets alone, the second with tick labels kept apart on the plot sizes of the
/// first), measures it ([`layout::measure_panel`]) and solves the grid ([`layout::solve_grid`]);
/// the panels are then placed for the final solve ([`layout::place_panel`]), and on a grid the
/// texts of every panel are checked to stay in its cells ([`layout::check_panel_contents`]). Layout
/// errors are reported in `context`.
///
/// # Errors
///
/// As [`layout::solve_grid`], from either pass: the second pass needs the plot sizes of the
/// first; then as [`layout::check_panel_contents`], for the final layout only.
fn plan(
    figure: &Figure,
    panels: &[Panel<'_>],
    lib: &FontLibrary,
    context: &'static str,
) -> Result<Vec<(Resolved, Layout)>> {
    let theme = figure.theme();
    let inputs: Vec<LayoutInput<'_>> = panels
        .iter()
        .map(|panel| LayoutInput {
            theme,
            axes: panel.axes,
            legend: &panel.legend,
            lib,
            context,
        })
        .collect();
    let grid = GridInput {
        width: figure.width_in() * 72.0,
        height: figure.height_in() * 72.0,
        grid: figure.grid(),
        theme,
        context,
        panels: &inputs,
    };
    let limits: Vec<Limits> = panels.iter().map(|panel| Limits::of(panel.axes)).collect();
    let pass = |sizes: Vec<PlotSize>, spacing: TickSpacing| -> (Vec<Resolved>, Vec<PanelBands>) {
        let resolved: Vec<Resolved> = panels
            .iter()
            .zip(&limits)
            .zip(sizes)
            .map(|((panel, limits), size)| {
                let (x, y) = layout::tick_rooms(size, theme, lib, spacing);
                resolve::resolve(&panel.marks, limits, &x, &y)
            })
            .collect();
        let bands = inputs
            .iter()
            .zip(&resolved)
            .map(|(input, resolved)| {
                layout::measure_panel(input, resolved, grid.span(input.axes.cells()))
            })
            .collect();
        (resolved, bands)
    };
    let guesses = inputs
        .iter()
        .map(|input| grid.initial_plot_size(input.axes.cells()))
        .collect();
    let (_, bands) = pass(guesses, TickSpacing::Target);
    let first = layout::solve_grid(&grid, &bands)?;
    let sizes = first.panels.iter().map(|slot| slot.plot.size()).collect();
    let (resolved, bands) = pass(sizes, TickSpacing::LabelsApart);
    let solution = layout::solve_grid(&grid, &bands)?;
    let (resolved, layouts): (Vec<Resolved>, Vec<Layout>) = inputs
        .iter()
        .zip(resolved)
        .zip(bands)
        .zip(&solution.panels)
        .map(|(((input, resolved), bands), slot)| {
            let layout = layout::place_panel(input, &resolved, bands, slot);
            (resolved, layout)
        })
        .unzip();
    layout::check_panel_contents(&grid, &layouts)?;
    Ok(resolved.into_iter().zip(layouts).collect())
}

/// [`plan`] for the axes of `figure` whose top-left cell is at `cell`, with the figure laid out
/// as [`scene`] lays it out. Panics when the cell holds no axes or the figure cannot be laid
/// out.
#[cfg(test)]
pub(crate) fn plan_figure(
    figure: &Figure,
    cell: (usize, usize),
    lib: &FontLibrary,
) -> (Resolved, Layout) {
    try_plan_figure(figure, cell, lib).expect("the figure lays out")
}

/// [`plan`] for the axes of `figure` whose top-left cell is at `cell`, as [`plan_figure`], with
/// layout errors reported in `"Figure.to_scene()"` as [`scene`] reports them. Panics when the
/// cell holds no axes.
#[cfg(test)]
pub(crate) fn try_plan_figure(
    figure: &Figure,
    cell: (usize, usize),
    lib: &FontLibrary,
) -> Result<(Resolved, Layout)> {
    let index = panel_index(figure, cell);
    let panels = panels(figure, lib);
    let mut planned = plan(figure, &panels, lib, "Figure.to_scene()")?;
    Ok(planned.swap_remove(index))
}

/// [`plan`] for every axes of `figure`, in the order of [`Figure::panels`], with layout errors
/// reported in `"Figure.to_scene()"` as [`scene`] reports them.
#[cfg(test)]
pub(crate) fn try_plan_panels(
    figure: &Figure,
    lib: &FontLibrary,
) -> Result<Vec<(Resolved, Layout)>> {
    plan(figure, &panels(figure, lib), lib, "Figure.to_scene()")
}

/// The index, in the order of [`Figure::panels`], of the axes of `figure` whose top-left cell
/// is at `cell`. Panics when the cell holds no axes.
#[cfg(test)]
fn panel_index(figure: &Figure, (row, col): (usize, usize)) -> usize {
    figure
        .panels()
        .position(|axes| axes.cells().row() == row && axes.cells().col() == col)
        .expect("the cell holds axes")
}

/// `f` called with the layout input of the axes of `figure` whose top-left cell is at `cell`,
/// with layout errors reported in `"Figure.to_scene()"` and the axes' marks resolved against
/// the figure's theme, and with the grid input of the whole figure. Panics when the cell holds
/// no axes.
#[cfg(test)]
pub(crate) fn with_layout_input<R>(
    figure: &Figure,
    cell: (usize, usize),
    lib: &FontLibrary,
    f: impl FnOnce(&LayoutInput<'_>, &GridInput<'_>) -> R,
) -> R {
    let index = panel_index(figure, cell);
    let panels = panels(figure, lib);
    let inputs: Vec<LayoutInput<'_>> = panels
        .iter()
        .map(|panel| LayoutInput {
            theme: figure.theme(),
            axes: panel.axes,
            legend: &panel.legend,
            lib,
            context: "Figure.to_scene()",
        })
        .collect();
    let grid = GridInput {
        width: figure.width_in() * 72.0,
        height: figure.height_in() * 72.0,
        grid: figure.grid(),
        theme: figure.theme(),
        context: "Figure.to_scene()",
        panels: &inputs,
    };
    f(&inputs[index], &grid)
}

/// The scene of `figure`: `width × 72` by `height × 72` pt on the theme background, holding
/// for each placed axes, in row-major order of their top-left cells,
/// - the axes chrome without the baseline, as one unclipped group;
/// - the visible marks, in insertion order, as one group clipped to the plot area (widened
///   horizontally for line ends, see `draw::marks_clip`);
/// - the legend, when it has row entries or direct labels, as one unclipped group;
/// - the baseline, as an unclipped group of its own.
///
/// Every axes is laid out by one solve of the grid ([`plan`]); a figure of one axes has the
/// canvas as its frame. Without axes the scene holds the background only. Each axes' marks are
/// resolved and its legend is built and measured once, before layout, and the same records
/// serve layout and drawing.
///
/// # Errors
///
/// As [`plan`], reported in `context`, the API call that renders.
pub(crate) fn scene(
    figure: &Figure,
    library: &FontLibrary,
    context: &'static str,
) -> Result<Scene> {
    let theme = figure.theme();
    let (width, height) = (figure.width_in() * 72.0, figure.height_in() * 72.0);
    let mut scene = Scene::new(width as f32, height as f32, theme.background());
    let panels = panels(figure, library);
    if panels.is_empty() {
        return Ok(scene);
    }
    let planned = plan(figure, &panels, library, context)?;
    for (panel, (resolved, layout)) in panels.iter().zip(&planned) {
        scene.push(draw::chrome(theme, layout));
        if let Some(marks) = draw::marks(theme, resolved, layout, &panel.marks) {
            scene.push(marks);
        }
        if let Some(legend) = draw::legend::legend(theme, layout) {
            scene.push(legend);
        }
        scene.push(draw::baseline(theme, layout));
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
    fn grid_panels_line_up_with_titles_over_their_plots() {
        // 2 × 2 at 12 × 8 in; (1, 1) is empty.
        let f = grid_of_lines(2, 2, &[(1, 0), (0, 0), (0, 1)]);
        let lib = FontLibrary::global();
        let layout = |row, col| plan_figure(&f, (row, col), lib).1;
        let (a, b, c) = (layout(0, 0), layout(0, 1), layout(1, 0));
        // Equal plots, aligned by column and by row.
        for l in [&b, &c] {
            assert!((l.plot.width - a.plot.width).abs() < 1e-9);
            assert!((l.plot.height - a.plot.height).abs() < 1e-9);
        }
        assert_eq!(a.plot.x, c.plot.x);
        assert_eq!(a.plot.y, b.plot.y);
        assert!(c.plot.y > a.plot.bottom() + 24.0);
        assert!(b.plot.x > a.plot.right() + 24.0);
        // Each title starts at its plot's left edge and sits above it, its row's top in the
        // first row at the padding.
        for l in [&a, &b, &c] {
            let title = l.title.as_ref().expect("a title");
            assert_eq!(title.x, l.plot.x);
            assert!(title.y < l.plot.y);
        }
        assert_eq!(a.header_bottom, b.header_bottom);
        // Neighbouring frames touch halfway between the cells.
        assert_eq!(a.frame.right(), b.frame.x);
        assert_eq!(a.frame.bottom(), c.frame.y);
        assert_eq!((a.frame.x, a.frame.y), (0.0, 0.0));
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
                .yticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(200)])
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

    /// The labels of `ticks`.
    fn tick_labels(ticks: &[crate::resolve::AxisTick]) -> Vec<&str> {
        ticks.iter().map(|tick| tick.label.as_str()).collect()
    }

    /// A line over x from 0 to 96 whose y values rise evenly from `low` to `high`.
    fn ramp(axes: &mut crate::AxesMut<'_>, low: f64, high: f64) {
        let x: Vec<f64> = (0..=96).map(f64::from).collect();
        let y: Vec<f64> = (0..=96)
            .map(|i| low + (high - low) * f64::from(i) / 96.0)
            .collect();
        axes.line(AxisData::Numbers(x), &y, LineOptions::default())
            .unwrap();
    }

    /// The panels of `figure` resolved for a plot of `size` and the tick spacing `spacing`, with
    /// their tick rooms.
    fn resolved_at<'a>(
        figure: &Figure,
        size: PlotSize,
        spacing: TickSpacing,
        lib: &'a FontLibrary,
    ) -> Vec<(Resolved, layout::TickRoom<'a>, layout::TickRoom<'a>)> {
        panels(figure, lib)
            .iter()
            .map(|panel| {
                let (x, y) = layout::tick_rooms(size, figure.theme(), lib, spacing);
                let resolved = resolve::resolve(&panel.marks, &Limits::of(panel.axes), &x, &y);
                (resolved, x, y)
            })
            .collect()
    }

    #[test]
    fn crowded_y_ticks_keep_their_labels_apart() {
        // A panel of a 4 × 5 grid at 12 × 8 in has a plot of about 126 × 72 pt. For data from
        // −0.01 to 5.69, the target alone (3) gives 8 y ticks about 8.2 pt apart.
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        ramp(
            &mut f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap(),
            -0.01,
            5.69,
        );
        let lib = FontLibrary::global();
        let size = PlotSize {
            width: 126.0,
            height: 72.0,
        };
        let (old, _, _) = resolved_at(&f, size, TickSpacing::Target, lib).remove(0);
        assert_eq!(
            tick_labels(&old.y.ticks),
            ["\u{2212}1", "0", "1", "2", "3", "4", "5", "6"]
        );
        let (new, _, room) = resolved_at(&f, size, TickSpacing::LabelsApart, lib).remove(0);
        assert!(room.slack(&old.y.ticks) < 0.0);
        assert!(room.slack(&new.y.ticks) >= 0.0);
        // A line (11.05 pt) and the gap (4.25 pt) apart, on a domain padded to 1.1 times the
        // span of the ticks, at most 1 + 72 / (1.1 × 15.3) = 5.3 ticks fit.
        assert_eq!(
            tick_labels(&new.y.ticks),
            ["\u{2212}0.4", "1.2", "2.8", "4.4", "6.0"]
        );
        // The x ticks were apart already, and stay as they were.
        assert_eq!(new.x.ticks, old.x.ticks);
    }

    #[test]
    fn grid_panels_keep_their_tick_labels_apart() {
        // Twenty panels of data from −0.01 to 5.69 on a 4 × 5 grid at 12 × 8 in.
        let mut f =
            Figure::with_grid(12.0, 8.0, Theme::pigment(), Grid::new(4, 5).unwrap()).unwrap();
        for i in 0..20 {
            let at = Placement {
                row: GridIndex::At(i / 5),
                col: GridIndex::At(i % 5),
                ..Placement::default()
            };
            let mut axes = f
                .add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)
                .unwrap();
            axes.title(&format!("Sensor {}", i + 1));
            ramp(&mut axes, -0.01, 5.69);
        }
        let lib = FontLibrary::global();
        for (resolved, layout) in try_plan_panels(&f, lib).unwrap() {
            let (x, y) =
                layout::tick_rooms(layout.plot.size(), f.theme(), lib, TickSpacing::LabelsApart);
            assert!(y.slack(&resolved.y.ticks) >= 0.0, "{:?}", layout.plot);
            assert!(x.slack(&resolved.x.ticks) >= 0.0, "{:?}", layout.plot);
            // The drawn labels: baselines a line and the gap apart.
            let baselines: Vec<f64> = layout.y_tick_labels.iter().map(|l| l.y).collect();
            assert!(baselines.windows(2).all(|w| (w[0] - w[1]).abs() >= 15.0));
        }
    }

    #[test]
    fn log_axes_keep_their_labels_apart() {
        let log = ScaleKind::Log {
            base: crate::LogBase::TEN,
        };
        let lib = FontLibrary::global();
        let y_ticks = |f: &Figure, size: PlotSize, spacing| {
            let (resolved, _, room) = resolved_at(f, size, spacing, lib).remove(0);
            let slack = room.slack(&resolved.y.ticks);
            (resolved.y.ticks, slack)
        };
        let line = |f: &mut Figure, low: f64, high: f64| {
            let mut axes = f.add_axes(ScaleKind::Linear, log).unwrap();
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[low, high],
                LineOptions::default(),
            )
            .unwrap();
        };
        // Thirteen decades whose lowest tick moves below the data: fewer powers, and none
        // left next to the new lower end.
        let theme = Theme::pigment().with_font_scale(2.2).unwrap();
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        line(&mut f, 1.9e-5, 1.4e7);
        let size = PlotSize {
            width: 20.0,
            height: 362.0,
        };
        let (old, _) = y_ticks(&f, size, TickSpacing::Target);
        let (new, slack) = y_ticks(&f, size, TickSpacing::LabelsApart);
        assert_eq!(old.len(), 15);
        assert!(slack >= 0.0 && new.len() < old.len(), "{new:?}");
        // Limits within one decade get linear ticks; those that were apart stay as they were.
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        line(&mut f, 0.0021, 0.0072);
        f.single_axes_mut().ylim(0.0021, 0.0072).unwrap();
        let size = PlotSize {
            width: 20.0,
            height: 28.0,
        };
        let (old, _) = y_ticks(&f, size, TickSpacing::Target);
        let (new, slack) = y_ticks(&f, size, TickSpacing::LabelsApart);
        assert_eq!(tick_labels(&new), ["0.0025", "0.0050"]);
        assert!(slack >= 0.0);
        assert_eq!(new, old);
    }

    mod spacing {
        use super::*;
        use crate::model::{AreaOptions, ScatterOptions};
        use crate::resolve::{AxisTick, NormalizedPos};
        use crate::{LogBase, TickFormat};
        use proptest::prelude::*;

        /// How the marks of a random figure fit their axes.
        #[derive(Debug, Clone, Copy)]
        enum Kind {
            /// A line: exact x limits, nice y limits.
            Line,
            /// Points: nice limits on both axes.
            Scatter,
            /// An area: its y axis keeps 0 on the plot edge.
            Area,
        }

        /// A data range on a linear (`log` false) or log axis: `(low, high)`.
        fn range(log: bool) -> impl Strategy<Value = (f64, f64)> {
            if log {
                (-5.0f64..5.0, 0.05f64..12.0)
                    .prop_map(|(e, decades)| (10f64.powf(e), 10f64.powf(e + decades)))
                    .boxed()
            } else {
                (-1e4f64..1e4, -3.0f64..5.0)
                    .prop_map(|(low, e)| (low, low + 10f64.powf(e)))
                    .boxed()
            }
        }

        /// A figure of one axes with a mark of `kind` over the `x` and `y` ranges, on the given
        /// axes kinds, explicit limits and a percent x format when asked.
        #[allow(clippy::too_many_arguments)]
        fn figure(
            theme: Theme,
            (x_log, y_log): (bool, bool),
            (x, y): ((f64, f64), (f64, f64)),
            kind: Kind,
            (xlim, ylim): (bool, bool),
            percent: bool,
        ) -> Figure {
            let scale = |log: bool| {
                if log {
                    ScaleKind::Log { base: LogBase::TEN }
                } else {
                    ScaleKind::Linear
                }
            };
            let mut f = Figure::new(6.0, 4.0, theme).unwrap();
            let mut axes = f.add_axes(scale(x_log), scale(y_log)).unwrap();
            let xs = AxisData::Numbers(vec![x.0, (x.0 + x.1) / 2.0, x.1]);
            let ys = [y.0, (y.0 + y.1) / 2.0, y.1];
            match kind {
                Kind::Line => axes.line(xs, &ys, LineOptions::default()),
                Kind::Scatter => axes.scatter(xs, &ys, ScatterOptions::default()),
                Kind::Area if !y_log => axes.area(xs, &ys, AreaOptions::default()),
                Kind::Area => axes.line(xs, &ys, LineOptions::default()),
            }
            .unwrap();
            if xlim {
                axes.xlim(x.0, x.1).unwrap();
            }
            if ylim {
                axes.ylim(y.0, y.1).unwrap();
            }
            if percent {
                axes.xtick_format(TickFormat::percent(1).unwrap()).unwrap();
            }
            f
        }

        /// One resolved axis: its domain and its ticks.
        type Axis<'a> = (Option<&'a crate::ContinuousScale>, &'a [AxisTick]);

        /// Checks one axis: its labels keep apart, or none of its candidates could and it
        /// fell back to a few ticks on a short axis; and ticks whose labels were apart for the
        /// target alone stay as they were, on the same domain.
        fn check_axis(
            name: &str,
            room: &layout::TickRoom<'_>,
            new: Axis<'_>,
            old: Axis<'_>,
        ) -> std::result::Result<(), TestCaseError> {
            let ticks = new.1;
            if room.slack(ticks) < 0.0 {
                // The widest candidate: two ticks, or three with the lowest moved below the
                // data, on an axis so short that the outer labels crowd a quarter of it apart.
                prop_assert!(ticks.len() <= 3, "{}: {} ticks crowd", name, ticks.len());
                let at = |tick: &AxisTick, pos: f64| AxisTick {
                    pos: NormalizedPos::new(pos),
                    label: tick.label.clone(),
                };
                let outer = [at(&ticks[0], 0.0), at(&ticks[ticks.len() - 1], 0.25)];
                prop_assert!(room.slack(&outer) < 0.0, "{}: {:?}", name, ticks);
            }
            if room.slack(old.1) >= 0.0 {
                prop_assert_eq!(new, old, "{}", name);
            }
            Ok(())
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(192))]

            #[test]
            fn automatic_tick_labels_keep_apart(
                theme in 0usize..3,
                font_scale in 0.5f64..2.5,
                (logs, ranges) in (any::<bool>(), any::<bool>())
                    .prop_flat_map(|(a, b)| (Just((a, b)), (range(a), range(b)))),
                kind in prop_oneof![Just(Kind::Line), Just(Kind::Scatter), Just(Kind::Area)],
                limits in (any::<bool>(), any::<bool>()),
                percent in any::<bool>(),
                width in 20.0f64..800.0,
                height in 20.0f64..800.0,
            ) {
                let theme = [Theme::pigment(), Theme::graphite(), Theme::midnight()][theme]
                    .with_font_scale(font_scale)
                    .unwrap();
                let f = figure(theme, logs, ranges, kind, limits, percent);
                let lib = FontLibrary::global();
                let size = PlotSize { width, height };
                let (old, _, _) = resolved_at(&f, size, TickSpacing::Target, lib).remove(0);
                let (new, x_room, y_room) =
                    resolved_at(&f, size, TickSpacing::LabelsApart, lib).remove(0);
                let x_axis = |r: &Resolved| (r.x.continuous().copied(), r.x.ticks.clone());
                let y_axis = |r: &Resolved| (Some(r.y.scale), r.y.ticks.clone());
                for (name, room, new, old) in [
                    ("x", &x_room, x_axis(&new), x_axis(&old)),
                    ("y", &y_room, y_axis(&new), y_axis(&old)),
                ] {
                    check_axis(name, room, (new.0.as_ref(), &new.1), (old.0.as_ref(), &old.1))?;
                }
            }
        }
    }
}
