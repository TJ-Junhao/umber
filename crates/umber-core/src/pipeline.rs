//! The render pipeline: from a [`Figure`] to its [`Scene`].
//!
//! Scales, ticks and the layout depend on each other: ticks depend on the plot size, and the
//! plot size depends on the tick label widths. [`plan`] settles them in two passes for every
//! panel of a figure at once; [`scene`] then draws each panel's chrome, marks, legend and
//! baseline.

use crate::bubble_fit::BubbleFits;
use crate::color_scale::ResolvedColorScale;
use crate::draw::bar_labels::{self, BarLabels};
use crate::draw::point_labels::{self, ScatterLabels};
use crate::field::arrow::{ArrowFrame, ScaleSearch};
use crate::layout::{
    self, FigureHeader, GridInput, Layout, LayoutInput, MeasuredLegend, PanelBands, PlotSize,
    TickSpacing,
};
use crate::legend::Legend;
use crate::model::Layer;
use crate::model::{Axes, Figure, ShareGroups, label_visibility};
use crate::resolve::{
    self, AxisRole, Limits, NormalizedPos, Resolved, ResolvedAxis, ResolvedMark, ValueAxis,
};
use crate::scene::Scene;
use crate::text::SceneFonts;
use crate::tick_format::AutomaticTickTarget;
use crate::{Error, FontLibrary, Result, Theme, draw};

/// One panel's records that layout and drawing share: its axes, its marks resolved against the
/// theme, and its legend, measured. They are made once, before layout.
struct Panel<'a> {
    /// The axes.
    axes: Layer<'a>,
    /// The axes' marks, resolved.
    marks: Vec<ResolvedMark<'a>>,
    /// The axes' legend, measured.
    legend: MeasuredLegend<'a>,
    /// The color scale shared by the axes' color group.
    color: Option<ResolvedColorScale>,
    /// The values written on the axes' bars, measured.
    labels: BarLabels,
    /// The values written next to the axes' scatter points, measured.
    points: ScatterLabels,
}

/// The legend of the axes `layer` of `figure`, whose resolved marks are `marks`, measured.
///
/// # Errors
///
/// As [`MeasuredLegend::measure`], for the first legend label that cannot be laid out.
fn layer_legend<'a>(
    figure: &'a Figure,
    layer: Layer<'a>,
    marks: &[ResolvedMark<'a>],
    lib: &SceneFonts<'_>,
) -> Result<MeasuredLegend<'a>> {
    let theme = figure.theme();
    match layer {
        Layer::Primary(crate::Panel::Pie(panel)) => {
            let mut measured = MeasuredLegend::measure(
                theme,
                &Legend::pie(panel.legend, panel.pie.as_ref()),
                lib,
                panel.legend_size,
            )?;
            if panel.legend != crate::LegendStyle::Direct {
                measured.position = panel.legend_position.unwrap_or(theme.legend_position());
            }
            Ok(measured)
        }
        Layer::Primary(crate::Panel::Cartesian { axes, .. }) | Layer::Twin { axes, .. } => {
            if figure.has_merged_legend() {
                let style = axes.legend_style().unwrap_or(theme.legend_style());
                MeasuredLegend::measure(
                    theme,
                    &Legend::panel(style, marks, &Limits::of(axes), true),
                    lib,
                    figure.legend_size(),
                )
            } else {
                measure_legend(theme, axes, marks, lib)
            }
        }
    }
}

/// Moves the legend row of every twin into its parent's, in the order of `panels`.
fn join_twin_legends(panels: &mut [Panel<'_>]) {
    for i in 0..panels.len() {
        match panels[i].axes {
            Layer::Primary(_) => {}
            Layer::Twin { parent, .. } => {
                let (before, after) = panels.split_at_mut(i);
                before[parent].legend.join_row(&mut after[0].legend);
            }
        }
    }
}

/// The records of every panel of `figure`, in the order of [`Figure::panels`]. The legend uses
/// the style set on the axes, else the theme's.
///
/// # Errors
///
/// As [`MeasuredLegend::measure`], for the first legend label that cannot be laid out.
fn panels<'a>(
    figure: &'a Figure,
    groups: &ShareGroups,
    lib: &SceneFonts<'_>,
) -> Result<Vec<Panel<'a>>> {
    let theme = figure.theme();
    let mut panels: Vec<_> = figure
        .layers()
        .zip(resolve::resolve_figure_marks(figure))
        .map(|(axes, marks)| {
            let legend = layer_legend(figure, axes, &marks, lib)?;
            let labels = match axes {
                Layer::Primary(crate::Panel::Cartesian { axes, .. }) | Layer::Twin { axes, .. } => {
                    BarLabels::measure(theme, lib, axes, &marks)?
                }
                Layer::Primary(crate::Panel::Pie(_)) => BarLabels::default(),
            };
            Ok(Panel {
                axes,
                marks,
                legend,
                color: None,
                labels,
                points: ScatterLabels::default(),
            })
        })
        .collect::<Result<_>>()?;
    join_twin_legends(&mut panels);
    resolve_colors(figure, &mut panels, groups);
    // Which color-mapped points are drawn, and so labelled, depends on the color scale.
    for panel in &mut panels {
        if let Some(axes) = panel.axes.cartesian() {
            panel.points =
                ScatterLabels::measure(theme, lib, axes, &panel.marks, panel.color.as_ref())?;
        }
    }
    Ok(panels)
}

/// Resolves each color group's scale from its members' current mapped values, with the
/// contour level labels in the format of the group's colorbar ticks.
fn resolve_colors(figure: &Figure, panels: &mut [Panel<'_>], groups: &ShareGroups) {
    for i in 0..panels.len() {
        if groups.color[i] != Some(i) {
            continue;
        }
        let Some((format, _)) = color_tick_format(figure, &panels[i]) else {
            continue;
        };
        let format = format.clone();
        let marks = panels
            .iter()
            .enumerate()
            .filter(|(j, _)| groups.color[*j] == Some(i))
            .flat_map(|(_, panel)| panel.marks.iter());
        let color = ResolvedColorScale::resolve(
            marks,
            panels[i].axes.cartesian().and_then(Axes::color_limits),
            (figure.theme(), &format),
        );
        for (j, panel) in panels.iter_mut().enumerate() {
            if groups.color[j] == Some(i) {
                panel.color = color.clone();
            }
        }
    }
}

/// Checks that every axes that sets a value column title
/// ([`AxesMut::value_column`](crate::AxesMut::value_column)) has a scatter that writes its
/// values in that column: `Error::Usage` under `context` for the first that has none, named
/// by its cell, rather than a title over nothing.
fn check_value_columns(figure: &Figure, context: &'static str) -> Result<()> {
    for layer in figure.layers() {
        let Some(axes) = layer.cartesian() else {
            continue;
        };
        if axes.value_column_options().is_none() {
            continue;
        }
        let used = axes.marks().iter().any(|mark| {
            matches!(mark, crate::model::Mark::Scatter(scatter)
                if scatter.options().labels.annotate
                    && scatter.options().labels.place == crate::Place::Column)
        });
        if !used {
            return Err(in_cell(
                figure,
                layer,
                Error::usage(
                    context,
                    "value_column() is set on these axes, but none of their scatters writes its \
                     values in the column."
                        .to_owned(),
                )
                .with_hint(
                    "pass annotate=True and place=Place.COLUMN to the scatter whose values the \
                     column shows, or remove the value_column() call.",
                ),
            ));
        }
    }
    Ok(())
}

/// Whether the axes of `panel` have equal aspect; a pie has no axes.
fn has_equal_aspect(panel: &Panel<'_>) -> bool {
    panel.axes.cartesian().is_some_and(Axes::has_equal_aspect)
}

/// Checks that no axes with equal aspect share an axis with another panel, which may have been
/// added after [`AxesMut::equal_aspect`](crate::AxesMut::equal_aspect): `Error::Usage` under
/// `context` for the first such axes, named by its cell. The call refuses every other kind of
/// axes, so layout finds two linear axes on any panel with equal aspect.
fn check_equal_aspect(figure: &Figure, context: &'static str) -> Result<()> {
    for layer in figure.layers() {
        if let Some(axes) = layer.cartesian().filter(|axes| axes.has_equal_aspect()) {
            figure
                .check_equal_aspect_share(axes, context)
                .map_err(|error| in_cell(figure, layer, error))?;
        }
    }
    Ok(())
}

/// Checks that the color-mapped contours of each color group, which share one colorbar, all
/// have the same levels, so the bar explains every one of them: `Error::Style` under `context`
/// naming the panels of the first two whose levels differ. It reads the levels the model
/// resolved, before any contour geometry is built.
fn check_contour_levels(
    figure: &Figure,
    groups: &ShareGroups,
    context: &'static str,
) -> Result<()> {
    const HINT: &str = "pass the same levels to every contour that shares a colorbar.";
    let panels: Vec<_> = figure.layers().collect();
    for source in 0..panels.len() {
        if groups.color.get(source) != Some(&Some(source)) {
            continue;
        }
        let mut first: Option<(usize, Vec<f64>)> = None;
        let members = panels
            .iter()
            .enumerate()
            .filter(|(i, _)| groups.color.get(*i) == Some(&Some(source)));
        for (i, panel) in members {
            let Some(axes) = panel.cartesian() else {
                continue;
            };
            let contours = axes.marks().iter().filter_map(|mark| {
                use crate::model::Mark;
                match mark {
                    Mark::Contour(contour) => contour.color_scale().map(|_| contour),
                    Mark::Line(_)
                    | Mark::Scatter(_)
                    | Mark::Bubbles(_)
                    | Mark::Quiver(_)
                    | Mark::Barbs(_)
                    | Mark::Stem(_)
                    | Mark::Heatmap(_)
                    | Mark::Area(_)
                    | Mark::Bar(_)
                    | Mark::Violin(_)
                    | Mark::GroupedViolin(_)
                    | Mark::Box(_)
                    | Mark::GroupedBox(_)
                    | Mark::GroupedBar(_)
                    | Mark::StackedBar(_)
                    | Mark::StackedArea(_)
                    | Mark::Mesh(_)
                    | Mark::Hist2d(_)
                    | Mark::Hexbin(_)
                    | Mark::Hist(_)
                    | Mark::Step(_)
                    | Mark::HLine(_)
                    | Mark::VLine(_)
                    | Mark::HSpan(_)
                    | Mark::VSpan(_)
                    | Mark::Band(_)
                    | Mark::ErrorBars(_)
                    | Mark::Events(_)
                    | Mark::Strip(_)
                    | Mark::Swarm(_)
                    | Mark::GroupedHist(_)
                    | Mark::Ecdf(_)
                    | Mark::Kde(_) => None,
                }
            });
            for contour in contours {
                let values = contour.levels();
                let Some((j, known)) = &first else {
                    first = Some((i, values));
                    continue;
                };
                if *known == values {
                    continue;
                }
                let required = "contours that share a colorbar need the same levels.";
                if *j == i {
                    let error = Error::style(
                        context,
                        format!("two contours of these axes have different levels; {required}"),
                    )
                    .with_hint(HINT);
                    return Err(in_cell(figure, axes, error));
                }
                let (a, b) = (panels[*j].cells(), axes.cells());
                return Err(Error::style(
                    context,
                    format!(
                        "the contours of row {}, col {} and row {}, col {} have different \
                         levels; {required}",
                        a.row(),
                        a.col(),
                        b.row(),
                        b.col()
                    ),
                )
                .with_hint(HINT));
            }
        }
    }
    Ok(())
}

/// The legend of `axes` with its resolved `marks`, its row entries and its direct labels,
/// measured once. The legend uses the style set on the axes, else the theme's.
fn measure_legend<'a>(
    theme: &Theme,
    axes: &Axes,
    marks: &[ResolvedMark<'a>],
    lib: &SceneFonts<'_>,
) -> Result<MeasuredLegend<'a>> {
    let style = axes.legend_style().unwrap_or(theme.legend_style());
    let legend = Legend::new(style, marks, &Limits::of(axes));
    let mut measured = MeasuredLegend::measure(theme, &legend, lib, axes.legend_size())?;
    if style != crate::LegendStyle::Direct {
        measured.position = axes.legend_position().unwrap_or(theme.legend_position());
    }
    Ok(measured)
}

/// Measures the figure title, subtitle and merged legend once, before the grid solve.
///
/// # Errors
///
/// As [`MeasuredLegend::measure`] and [`FigureHeader::measure`].
fn figure_header(
    figure: &Figure,
    panels: &[Panel<'_>],
    lib: &SceneFonts<'_>,
) -> Result<FigureHeader> {
    let legend = if figure.has_merged_legend() {
        let merged = Legend::merged(figure, panels.iter().map(|panel| panel.marks.as_slice()));
        MeasuredLegend::measure(figure.theme(), &merged, lib, figure.legend_size())?
    } else {
        MeasuredLegend::default()
    };
    FigureHeader::measure(figure, &legend, lib)
}

/// The figure-wide header and the resolved layout of every panel.
struct FigurePlan {
    header: FigureHeader,
    panels: Vec<(Resolved, Layout)>,
    /// The final solve of the grid: where the panels went, and the shared colorbars.
    solution: layout::GridSolution,
    /// Whether every bubble fit settled, so that the plots hold every drawn bubble (see
    /// [`BubbleFits::settled`]).
    bubbles_settled: bool,
}

/// The same figure without equal aspect, as far as a figure with the setting takes anything from
/// it: where the grid put the panels, and the hexbin counts of each panel, in mark order.
#[derive(Debug, Clone)]
pub(crate) struct Reference {
    /// The solve of the grid.
    solution: layout::GridSolution,
    /// The hexbin counts of each panel, in the order of the panels.
    hexbins: Vec<Vec<crate::bin2d::HexBins>>,
}

/// Where a plan takes the rooms of the plots from, and how much it refuses.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Rooms<'a> {
    /// A figure without equal aspect: each solve of the grid gives the plots.
    Solved,
    /// The figure that a figure with equal aspect is without the setting, laid out only for its
    /// rooms: each solve of the grid gives the plots, and nothing is refused that depends on
    /// where things are placed, since it is the final figure that has to hold them. The reference
    /// lands in `settled` once the layout is complete. `equal` says which panels have the setting
    /// in that final figure: they are laid out and drawn again there, so what fails on them here
    /// is no error, and a panel that fails to draw is left out.
    Reference {
        /// Where the reference lands.
        settled: &'a std::cell::RefCell<Option<Reference>>,
        /// Whether each panel has equal aspect in the final figure.
        equal: &'a [bool],
    },
    /// A figure with equal aspect: the grid keeps the solve of this reference, and the plots
    /// with equal aspect shrink inside its rooms, so that no change of their limits or tick
    /// labels reaches the other panels.
    Given(&'a Reference),
    /// As [`Rooms::Given`], on the way to the final layout: hexbin cells are still changing the
    /// limits, so nothing is refused for the size or shape of this plan.
    Provisional(&'a Reference),
}

impl Rooms<'_> {
    /// The rooms for a figure with the reference `reference`, `None` for a figure without
    /// equal aspect.
    fn of(reference: Option<&Reference>) -> Rooms<'_> {
        reference.map_or(Rooms::Solved, Rooms::Given)
    }

    /// Whether this plan is a final one, whose checks of limits, sizes and contents apply.
    fn is_final(self) -> bool {
        matches!(self, Rooms::Solved | Rooms::Given(_))
    }
}

/// Why a layout was not made: an error, or bubbles that no automatic limits hold, which
/// [`plan_panels`] turns into an error naming a figure size that holds them.
#[derive(Debug)]
enum Unplanned {
    /// The layout fails with this error.
    Failed(Error),
    /// No automatic limits hold these bubbles on the plots of the layout.
    Bubbles(crate::bubble_fit::BubbleFailure),
}

impl From<Error> for Unplanned {
    fn from(error: Error) -> Self {
        Unplanned::Failed(error)
    }
}

/// One side of a plot area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The x axis.
    X,
    /// The y axis.
    Y,
}

impl Side {
    /// Both sides, x first.
    const BOTH: [Side; 2] = [Side::X, Side::Y];

    /// The index of this side in a pair.
    fn index(self) -> usize {
        match self {
            Side::X => 0,
            Side::Y => 1,
        }
    }

    /// Panel index to the first member of this side's group.
    fn groups(self, groups: &ShareGroups) -> &[usize] {
        match self {
            Self::X => &groups.x,
            Self::Y => &groups.y,
        }
    }

    /// The smallest plot size among members that show this side's labels. Hidden labels
    /// must not limit the ticks of a wider or taller visible member.
    fn group_plot(
        self,
        groups: &ShareGroups,
        i: usize,
        plots: &[PlotSize],
        show_labels: &[[bool; 2]],
    ) -> PlotSize {
        let keys = self.groups(groups);
        plots
            .iter()
            .enumerate()
            .filter(|(j, _)| keys[*j] == keys[i] && show_labels[*j][self.index()])
            .map(|(_, plot)| *plot)
            .reduce(|size, plot| PlotSize {
                width: size.width.min(plot.width),
                height: size.height.min(plot.height),
            })
            .unwrap_or(plots[i])
    }

    /// The axis on this side.
    fn axis(self) -> ValueAxis {
        match self {
            Side::X => ValueAxis::X,
            Side::Y => ValueAxis::Y,
        }
    }

    /// Whether this side of an axes with `limits` gets automatic ticks: a category axis, or a
    /// linear, log or time axis without explicit ticks.
    fn is_automatic(self, limits: &Limits<'_>) -> bool {
        match limits.axis(self.axis()) {
            resolve::AxisLimits::Continuous(axis) => axis.ticks.explicit().is_none(),
            resolve::AxisLimits::Time(axis) => axis.ticks.explicit().is_none(),
            resolve::AxisLimits::Category => true,
        }
    }

    /// The ticks of this side of `resolved`.
    fn ticks(self, resolved: &Resolved) -> &[resolve::AxisTick] {
        match self {
            Side::X => &resolved.x.ticks,
            Side::Y => &resolved.y.ticks,
        }
    }

    /// The tick room of this side of a plot area of `plot`, with the tick labels of `tick`.
    fn room<'a>(self, plot: PlotSize, tick: &'a layout::TickFont<'a>) -> layout::TickRoom<'a> {
        let (x, y) = layout::tick_rooms(plot, tick, TickSpacing::LabelsApart);
        match self {
            Side::X => x,
            Side::Y => y,
        }
    }

    /// Tick room for axis labels, distinct from the theme-sized colorbar labels.
    fn axis_room<'a>(
        self,
        plot: PlotSize,
        ticks: &'a [layout::TickFont<'a>; 2],
    ) -> layout::TickRoom<'a> {
        self.room(plot, &ticks[self.index()])
    }
}

/// How the final layout chooses the automatic ticks of one side of a panel.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TickChoice {
    /// The plot size the ticks are chosen for.
    plot: PlotSize,
    /// The most ticks allowed; `None` before the ticks are first chosen again.
    most: Option<usize>,
    /// Whether the ticks may still be chosen again.
    state: ChoiceState,
}

/// Whether the ticks of one side may still be chosen again, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChoiceState {
    /// They may: `last` is the number of ticks the last choice gave, if they were chosen
    /// again before.
    Open {
        /// The number of ticks the last choice gave.
        last: Option<usize>,
    },
    /// They stay: no set the layout can hold keeps the labels apart.
    Settled,
}

impl TickChoice {
    /// The first choice, for the plot size `plot`.
    fn first(plot: PlotSize) -> TickChoice {
        TickChoice {
            plot,
            most: None,
            state: ChoiceState::Open { last: None },
        }
    }

    /// The tick room of `side`, with the tick labels of `tick`.
    fn room<'a>(&self, side: Side, tick: &'a layout::TickFont<'a>) -> layout::TickRoom<'a> {
        let room = side.room(self.plot, tick);
        match self.most {
            Some(most) => room.at_most(most),
            None => room,
        }
    }

    /// The choice again for the plot size `plot`, of `ticks` ticks whose labels crowd on it:
    /// as many ticks at most the first time at that number, one fewer when the last choice
    /// gave that number already. A linear pair can retry once on the final plot, where it
    /// may need the single-tick fallback. Other axes keep their two-tick minimum.
    fn again(&self, plot: PlotSize, ticks: usize, retry_pair: bool) -> Option<TickChoice> {
        let ChoiceState::Open { last } = self.state else {
            return None;
        };
        if ticks <= 2 && !(retry_pair && ticks == 2 && last != Some(2)) {
            return None;
        }
        let most = if last == Some(ticks) {
            ticks - 1
        } else {
            ticks
        };
        Some(TickChoice {
            plot,
            most: Some(most),
            state: ChoiceState::Open { last: Some(ticks) },
        })
    }
}

/// The axis whose tick-room geometry a colorbar follows.
fn color_side(side: layout::ColorbarSide) -> Side {
    match side {
        layout::ColorbarSide::Bottom => Side::X,
        layout::ColorbarSide::Right => Side::Y,
    }
}

/// The visible colorbar's format and whether it came from its source panel's y tick format.
fn color_tick_format<'a>(
    figure: &'a Figure,
    panel: &'a Panel<'_>,
) -> Option<(&'a crate::TickFormat, AutomaticTickTarget)> {
    let shared = figure.grid().share_color() != crate::Share::None;
    let axes = panel.axes.cartesian()?;
    let options = if shared {
        figure.colorbar_options()
    } else {
        axes.colorbar_options()
    };
    Some((
        options.format.as_ref().unwrap_or(axes.y_tick_format()),
        AutomaticTickTarget::Colorbar {
            shared,
            inherited: options.format.is_none(),
        },
    ))
}

/// Relabels and spaces a visible colorbar without changing its norm or colormap.
fn choose_color_ticks(
    figure: &Figure,
    panel: &Panel<'_>,
    resolved: &mut Resolved,
    room: &layout::TickRoom<'_>,
) {
    if let Some(color) = &resolved.color
        && let Some((format, _)) = color_tick_format(figure, panel)
    {
        resolved.color = Some(color.with_ticks(room, format));
    }
}

/// Resolves each group once, in row-major panel order, in the role its members' marks give it
/// ([`AxisRole::of_group`]), then gives its members identical scales and ticks, with each
/// group's data extent widened to fit its bubbles as `fits` says. Each panel's baseline is the
/// base position of its value axis. Marks remain per panel for drawing and legend colors.
fn resolve_groups(
    panels: &[Panel<'_>],
    limits: &[Limits<'_>],
    groups: &ShareGroups,
    rooms: &[[layout::TickRoom<'_>; 2]],
    fits: &BubbleFits,
) -> Vec<Resolved> {
    // Each panel's axes with their base positions, x then y; a group's members copy its first.
    let mut axes: Vec<[(ResolvedAxis, NormalizedPos); 2]> = Vec::with_capacity(panels.len());
    for i in 0..panels.len() {
        let resolved = Side::BOTH.map(|side| {
            let keys = side.groups(groups);
            let first = keys[i];
            if first != i {
                return axes[first][side.index()].clone();
            }
            let members = || (0..panels.len()).filter(|&j| keys[j] == i);
            let marks = members().flat_map(|j| panels[j].marks.iter());
            let role = AxisRole::of_group(
                side.axis(),
                members().map(|j| (limits[j].orientation(), panels[j].marks.as_slice())),
            );
            let bubbles = match side {
                Side::X => fits.x_extent(i),
                Side::Y => fits.y_extent(i),
            };
            resolve::resolve_axis(
                marks,
                side.axis(),
                role,
                limits[i].axis(side.axis()),
                &rooms[i][side.index()],
                bubbles,
            )
        });
        axes.push(resolved);
    }
    axes.into_iter()
        .zip(panels)
        .map(|([x, y], panel)| Resolved::new(x, y, panel.color.clone()))
        .collect()
}

/// Resolves and lays out every panel of `figure`, whose records are `panels`.
///
/// Ticks depend on the plot size, and the plot size depends on the tick label widths. A first
/// pass resolves every panel for its tick targets alone, as if each plot filled its cells
/// inside the padding (the whole canvas inside the padding for a figure of one cell), measures
/// it ([`layout::measure_panel`]) and solves the grid ([`layout::solve_grid`]). The final pass
/// first tries the previous constrained choices, including target retries, on those sizes
/// ([`layout::tick_rooms`]). If all their labels fit on the solved plots, both the ticks and
/// the layout stay unchanged. Otherwise it resolves and solves again, repairing crowded
/// choices with candidates that draw at least two ticks.
///
/// That solve can give a plot smaller than the one the ticks were chosen for. While automatic
/// numeric or time tick labels crowd on the solved plot, its ticks are chosen again for that
/// plot ([`TickChoice::again`]): at most as many ticks the first time at a number of ticks, one
/// fewer the next, and the grid is solved again. A side whose ticks number two, whose new
/// choice does not fit even on the plot it was made for, or whose new choice the grid cannot
/// hold (the solve fails, and the previous solve is kept) keeps its ticks. Each side is chosen
/// again at most twice per number of ticks, and its ticks never grow in number while the
/// domain stays fixed. Before retaining a single or crowded set, pairs are tried on their own
/// solved layouts without moving any domain: whole-step pairs on linear axes, the existing
/// outer ticks on log axes. A pair can fit without the wider labels that made the preferred
/// choice fail the solve. If no linear pair fits, a crowded set can keep its existing tick
/// nearest zero when inside the domain, otherwise the midpoint.
///
/// Category axes provisionally thin their labels on the guessed plot sizes. Each final solve
/// tries category candidates on the layouts their own labels produce, starting with only
/// category 0. Each accepted change lowers a 1/2/5 decade step while keeping every category
/// axis apart; finite choices therefore cannot oscillate. Shared axes choose for their
/// smallest visible member and copy the same ticks to the whole group.
///
/// Before each round the bubble fits are asked again on the solved plots ([`BubbleFits`]):
/// the first time, every automatic axis holding bubbles widens its data extent to fit them on
/// those plots, before padding (and log-axis rounding), and later whenever a bubble reaches past a
/// plot edge, shared panels with hidden tick labels included. A changed extent resolves the
/// axes and solves the grid again, with the same tick choices. Bubbles that no extent holds
/// fail the layout ([`BubbleFits::failure`]; [`plan_panels`] names a figure size that holds
/// them), and so does a grid that cannot hold the fitted limits. Only a fit still unsettled
/// after the rounds leaves its axis with the limits of the data positions, and the plot clips
/// its bubbles; on axes with equal aspect that is an error.
///
/// A figure with equal aspect takes its rooms from the same figure without the setting, laid
/// out to the end first ([`reference_rooms`]): the grid keeps that solve ([`Rooms::Given`]), so
/// each panel without the setting lays out as it does there, and each panel with it stays in the
/// room it has there. Every solve ends by shrinking those plots for the limits it was made with
/// ([`layout::fit_equal_aspect`]), so ticks, bubble fits and everything placed around a plot read
/// the smaller plot, and the scales are equal on the final layout. Their bubbles and value
/// labels are fitted on one scale for both axes ([`BubbleFits::update`]). That function explains
/// why the passes end.
///
/// The panels are then placed for the final solve ([`layout::place_panel`]), and on a grid the
/// texts of every panel are checked to stay in its cells ([`layout::check_panel_contents`]).
/// Layout errors are reported in `context`.
///
/// # Errors
///
/// As [`layout::solve_grid`], from either pass: the final pass needs the plot sizes of the
/// first; [`Unplanned::Bubbles`] for bubbles that no automatic limits hold
/// ([`BubbleFits::failure`]); then as [`layout::check_panel_contents`], for the final layout
/// only.
fn plan(
    figure: &Figure,
    panels: &[Panel<'_>],
    groups: &ShareGroups,
    lib: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
    rooms: Rooms<'_>,
) -> std::result::Result<FigurePlan, Unplanned> {
    let theme = figure.theme();
    let header = figure_header(figure, panels, lib)?;
    header.check(figure, context)?;
    let tick = layout::TickFont::new(lib, theme)?;
    // The tick labels of each axis, those of its highlighted categories in SemiBold.
    let axis_ticks: Vec<_> = panels
        .iter()
        .map(|panel| {
            let axes = panel.axes.cartesian();
            let font = |size: fn(&Axes) -> crate::SizeSpec,
                        highlight: fn(&Axes) -> &crate::TickHighlight| {
                let font =
                    layout::TickFont::sized(lib, theme, axes.map_or(crate::SizeSpec::Auto, size))?;
                match axes {
                    Some(axes) => font.highlighted(highlight(axes)),
                    None => Ok(font),
                }
            };
            Ok([
                font(Axes::xtick_size, Axes::x_tick_highlight)?,
                font(Axes::ytick_size, Axes::y_tick_highlight)?,
            ])
        })
        .collect::<Result<_>>()?;
    let inputs: Vec<LayoutInput<'_>> = panels
        .iter()
        .map(|panel| LayoutInput {
            theme,
            figure_title_size: figure.title_size(),
            axes: panel.axes,
            legend: &panel.legend,
            column: panel.points.column(),
            lib,
            context,
        })
        .collect();
    let grid = GridInput {
        width: figure.width_in() * 72.0,
        height: header.grid_height(figure),
        caption: header.caption.room(),
        top: header.bottom,
        grid: figure.grid(),
        theme,
        context,
        panels: &inputs,
        colorbars: &[],
    };
    let shared = figure.grid().share_color() != crate::Share::None;
    let color_cells = groups.colorbar_cells(figure);
    let color_sides: Vec<_> = panels
        .iter()
        .enumerate()
        .map(|(i, panel)| {
            if shared {
                color_cells[i]
                    .and_then(|_| layout::ColorbarSide::shared(figure))
                    .map(color_side)
            } else {
                panel
                    .axes
                    .cartesian()
                    .and_then(|axes| layout::ColorbarSide::resolve(axes, theme))
                    .map(color_side)
            }
        })
        .collect();
    let equal_aspect: Vec<_> = panels.iter().map(has_equal_aspect).collect();
    let solve_rooms =
        |resolved: &[Resolved], bands: &[PanelBands]| -> Result<layout::GridSolution> {
            let colorbars: Vec<_> = color_cells
                .iter()
                .enumerate()
                .filter_map(|(i, cells)| {
                    let cells = (*cells)?;
                    let position = layout::ColorbarSide::shared(figure)?;
                    let color = resolved[i].color.as_ref()?;
                    let measured = layout::MeasuredColorbar::with_options(
                        figure.colorbar_options(),
                        color,
                        position,
                        theme,
                        lib,
                    );
                    Some(measured.map(|measured| layout::SharedColorbar {
                        source: i,
                        cells,
                        measured,
                    }))
                })
                .collect::<Result<_>>()?;
            layout::solve_grid(
                &GridInput {
                    colorbars: &colorbars,
                    ..grid
                },
                bands,
            )
        };
    let solve = |resolved: &[Resolved], bands: &[PanelBands]| -> Result<layout::GridSolution> {
        let mut solution = match rooms {
            Rooms::Solved | Rooms::Reference { .. } => solve_rooms(resolved, bands)?,
            Rooms::Given(reference) | Rooms::Provisional(reference) => reference.solution.clone(),
        };
        // Every solve ends here, so whatever reads a plot below reads the final rectangle.
        for (i, slot) in solution.panels.iter_mut().enumerate() {
            // Equal aspect is refused on any other axes (`check_equal_aspect`).
            if let (true, Some(x), Some(y)) = (
                equal_aspect[i],
                resolved[i].x.continuous(),
                resolved[i].y.continuous(),
            ) {
                let cells = panels[i].axes.cells();
                let outer = [
                    cells.col() == 0,
                    cells.col() + cells.col_span() == figure.grid().cols(),
                ];
                layout::fit_equal_aspect(slot, &bands[i], outer, [x, y], context)
                    .map_err(|error| in_cell(figure, panels[i].axes, error))?;
            }
        }
        Ok(solution)
    };
    let color_plots = |solution: &layout::GridSolution| {
        let mut plots: Vec<_> = solution
            .panels
            .iter()
            .map(|slot| slot.plot.size())
            .collect();
        for (source, bar) in &solution.colorbars {
            plots[*source] = bar.bar.size();
        }
        plots
    };
    let limits: Vec<Limits> = panels.iter().map(|panel| Limits::of(panel.axes)).collect();
    let marks: Vec<&[ResolvedMark<'_>]> =
        panels.iter().map(|panel| panel.marks.as_slice()).collect();
    let ends: Vec<_> = panels
        .iter()
        .map(|panel| match panel.axes {
            Layer::Primary(crate::Panel::Cartesian { .. }) | Layer::Twin { .. } => {
                let mut ends = panel.labels.end_reaches();
                ends.extend(panel.points.end_reaches());
                ends
            }
            Layer::Primary(crate::Panel::Pie(_)) => Vec::new(),
        })
        .collect();
    let scales: Vec<_> = panels.iter().map(|panel| panel.color.as_ref()).collect();
    let fills = crate::bubble_fit::Fills {
        theme,
        scales: &scales,
    };
    let mut fits = BubbleFits::new(&marks, &ends, &limits, groups, fills);
    // How the extents of the panels with equal aspect, whose axes are their own, are finished.
    let extent_paddings: Vec<_> = (0..panels.len())
        .map(|i| {
            equal_aspect[i].then(|| {
                Side::BOTH.map(|side| {
                    let member = (limits[i].orientation(), marks[i]);
                    let role = AxisRole::of_group(side.axis(), std::iter::once(member));
                    resolve::extent_padding(marks[i].iter(), side.axis(), role)
                })
            })
        })
        .collect();
    let show_labels: Vec<_> = panels
        .iter()
        .map(|panel| match panel.axes {
            Layer::Primary(axes) => label_visibility(figure, axes.cells()),
            Layer::Twin { .. } => [false, true],
        })
        .collect();
    let has_shared_colorbar = shared
        && layout::ColorbarSide::shared(figure).is_some()
        && panels.iter().any(|panel| panel.color.is_some());
    // Whether a style can be laid out does not depend on its text, and every pass sets text in
    // the same styles; the first pass, below, propagates its measurement errors, so a later
    // pass that takes a failed solve for a layout that does not fit never hides a text error.
    let measure = |i: usize, resolved: &Resolved| -> Result<PanelBands> {
        let input = &inputs[i];
        let mut span = grid.span(input.axes.cells());
        if has_shared_colorbar {
            // Keep direct labels inside their panel, before any shared colorbar.
            span.reserve_cap = f64::INFINITY;
        }
        let bands = layout::measure_panel(input, resolved, span, show_labels[i], labels)?;
        Ok(if shared {
            bands.without_colorbar(span.reserve_cap)
        } else {
            bands
        })
    };

    // Try category steps on their own solved plots, not on the previous step's plot. Y goes
    // first because its label widths can release room for x. Begin with only category 0, then
    // only lower steps, accepting a change only when all category groups still fit. Repeating
    // the search lets a group use room released by another without ever increasing a step.
    let solve_ticks = |mut resolved: Vec<Resolved>| -> Result<_> {
        let mut categories = Vec::new();
        for side in [Side::Y, Side::X] {
            for (i, r) in resolved.iter().enumerate() {
                if side.groups(groups)[i] != i {
                    continue;
                }
                let axis = match side {
                    Side::X => &r.x,
                    Side::Y => &r.y,
                };
                let resolve::AxisScale::Category(scale) = &axis.scale else {
                    continue;
                };
                let ticks = resolve::category_ticks(scale, side.axis());
                if ticks.len() <= 1 {
                    continue;
                }
                let mut candidates = Vec::new();
                let mut decade = 1_usize;
                'steps: loop {
                    for factor in [1_usize, 2, 5] {
                        let selected: Vec<_> = ticks
                            .iter()
                            .step_by(decade.saturating_mul(factor))
                            .cloned()
                            .collect();
                        let last = selected.len() == 1;
                        candidates.push(selected);
                        if last {
                            break 'steps;
                        }
                    }
                    decade = decade.saturating_mul(10);
                }
                categories.push((i, side, candidates));
            }
        }
        let set_ticks =
            |resolved: &mut [Resolved], i: usize, side: Side, ticks: &[resolve::AxisTick]| {
                for (j, r) in resolved.iter_mut().enumerate() {
                    if side.groups(groups)[j] == i {
                        match side {
                            Side::X => r.x.ticks = ticks.to_vec(),
                            Side::Y => r.y.ticks = ticks.to_vec(),
                        }
                    }
                }
            };
        let mut selected: Vec<_> = categories
            .iter()
            .map(|(i, side, candidates)| {
                let last = candidates.len() - 1;
                set_ticks(&mut resolved, *i, *side, &candidates[last]);
                last
            })
            .collect();
        let mut bands: Vec<_> = resolved
            .iter()
            .enumerate()
            .map(|(i, r)| measure(i, r))
            .collect::<Result<_>>()?;
        let mut solution = solve(&resolved, &bands)?;
        loop {
            let mut changed = false;
            for (group, (i, side, candidates)) in categories.iter().enumerate() {
                for (candidate, ticks) in candidates.iter().enumerate().take(selected[group]) {
                    let mut tried = resolved.clone();
                    set_ticks(&mut tried, *i, *side, ticks);
                    let tried_bands: Vec<_> = tried
                        .iter()
                        .enumerate()
                        .map(|(i, r)| measure(i, r))
                        .collect::<Result<_>>()?;
                    let Ok(tried_solution) = solve(&tried, &tried_bands) else {
                        continue;
                    };
                    let plots: Vec<_> = tried_solution
                        .panels
                        .iter()
                        .map(|slot| slot.plot.size())
                        .collect();
                    if categories.iter().all(|(i, side, _)| {
                        let plot = side.group_plot(groups, *i, &plots, &show_labels);
                        side.axis_room(plot, &axis_ticks[*i])
                            .with_labels(labels)
                            .slack(side.ticks(&tried[*i]))
                            >= 0.0
                    }) {
                        selected[group] = candidate;
                        resolved = tried;
                        bands = tried_bands;
                        solution = tried_solution;
                        changed = true;
                        break;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        Ok((resolved, bands, solution))
    };

    let rooms_for = |plots: &[PlotSize], spacing| {
        (0..panels.len())
            .map(|i| {
                let x = Side::X.group_plot(groups, i, plots, &show_labels);
                let y = Side::Y.group_plot(groups, i, plots, &show_labels);
                let (x, _) = layout::tick_rooms(x, &axis_ticks[i][0], spacing);
                let (_, y) = layout::tick_rooms(y, &axis_ticks[i][1], spacing);
                [x.with_labels(labels), y.with_labels(labels)]
            })
            .collect::<Vec<_>>()
    };
    // The first pass: ticks for their targets alone, on guessed plot sizes.
    let guesses: Vec<_> = inputs
        .iter()
        .map(|input| grid.initial_plot_size(input.axes.cells()))
        .collect();
    let mut first = resolve_groups(
        panels,
        &limits,
        groups,
        &rooms_for(&guesses, TickSpacing::Target),
        &fits,
    );
    for (i, resolved) in first.iter_mut().enumerate() {
        if let Some(side) = color_sides[i] {
            let size = color_cells[i].map_or(guesses[i], |cells| grid.initial_plot_size(cells));
            let room = side.room(size, &tick);
            choose_color_ticks(figure, &panels[i], resolved, &room);
        }
    }
    let first_bands: Vec<PanelBands> = first
        .iter()
        .enumerate()
        .map(|(i, r)| measure(i, r))
        .collect::<Result<_>>()?;
    let first = solve(&first, &first_bands).or_else(|error| {
        // Automatic x labels can exceed the canvas inset before their first fit. Obtain
        // a provisional plot without them, then let the normal tick choice retry on it.
        // Explicit ticks and every other layout constraint still take part in this solve.
        let mut provisional = first.clone();
        let mut changed = false;
        for (i, r) in provisional.iter_mut().enumerate() {
            if Side::X.is_automatic(&limits[i])
                && r.x
                    .continuous()
                    .is_some_and(|scale| scale.transform() == crate::Transform::Linear)
                && !r.x.ticks.is_empty()
            {
                r.x.ticks.clear();
                changed = true;
            }
        }
        if !changed {
            return Err(error);
        }
        let bands: Vec<_> = provisional
            .iter()
            .enumerate()
            .map(|(i, r)| measure(i, r))
            .collect::<Result<_>>()?;
        solve(&provisional, &bands).map_err(|_| error)
    })?;
    let plots: Vec<_> = first.panels.iter().map(|slot| slot.plot.size()).collect();
    let bar_plots = color_plots(&first);

    // The final pass: tick labels kept apart on the plot sizes of the first.
    let resolve_choices = |choices: &[[TickChoice; 3]], fits: &BubbleFits| {
        let rooms: Vec<_> = choices
            .iter()
            .zip(&axis_ticks)
            .map(|(c, ticks)| {
                [
                    c[0].room(Side::X, &ticks[0]).with_labels(labels),
                    c[1].room(Side::Y, &ticks[1]).with_labels(labels),
                ]
            })
            .collect();
        let mut resolved = resolve_groups(panels, &limits, groups, &rooms, fits);
        for (i, r) in resolved.iter_mut().enumerate() {
            if let Some(side) = color_sides[i] {
                let room = choices[i][2].room(side, &tick);
                choose_color_ticks(figure, &panels[i], r, &room);
            }
        }
        resolved
    };
    let mut choices: Vec<[TickChoice; 3]> = (0..panels.len())
        .map(|i| {
            let [x, y] = Side::BOTH
                .map(|side| TickChoice::first(side.group_plot(groups, i, &plots, &show_labels)));
            [x, y, TickChoice::first(bar_plots[i])]
        })
        .collect();
    // First keep the complete previous constrained search. Its fallback may crowd on the
    // size used to choose it yet fit on the solved plot; only that plot decides whether to
    // repair it. This also keeps the layout, not just the ticks, when everything already fits.
    let mut resolved = resolve_groups(
        panels,
        &limits,
        groups,
        &rooms_for(&plots, TickSpacing::Deferred),
        &fits,
    );
    for (i, r) in resolved.iter_mut().enumerate() {
        if let Some(side) = color_sides[i] {
            let room = side.room(bar_plots[i], &tick);
            choose_color_ticks(figure, &panels[i], r, &room);
        }
    }
    let (mut resolved, mut bands, mut solution) = match solve_ticks(resolved) {
        Ok((resolved, bands, solution))
            if solution.panels.iter().enumerate().all(|(i, slot)| {
                Side::BOTH.into_iter().all(|side| {
                    !show_labels[i][side.index()]
                        || !side.is_automatic(&limits[i])
                        || side
                            .axis_room(slot.plot.size(), &axis_ticks[i])
                            .slack(side.ticks(&resolved[i]))
                            >= 0.0
                })
            }) =>
        {
            (resolved, bands, solution)
        }
        _ => solve_ticks(resolve_choices(&choices, &fits))?,
    };

    // Numeric and time sides retry at most twice per tick count.
    let rounds: usize = resolved
        .iter()
        .map(|r| {
            2 * (r.x.ticks.len() + r.y.ticks.len() + r.color.as_ref().map_or(0, |c| c.ticks.len()))
                + 1
        })
        .sum();
    // Resolves the axes for the current tick choices and bubble fits and solves the grid
    // again; a grid that cannot hold the fitted limits fails.
    let refit = |fits: &BubbleFits, choices: &[[TickChoice; 3]]| {
        solve_ticks(resolve_choices(choices, fits))
    };
    for _ in 0..rounds + fits.budget() {
        // The sides whose labels crowd on the solved plots, chosen again.
        let mut next = choices.clone();
        let mut again: Vec<(usize, Side)> = Vec::new();
        let plots: Vec<_> = solution
            .panels
            .iter()
            .map(|slot| slot.plot.size())
            .collect();
        // The plots with equal aspect have one scale for both axes, in the room they may take.
        let joint: Vec<_> = solution
            .panels
            .iter()
            .zip(&extent_paddings)
            .map(|(slot, padding)| {
                padding.map(|padding| crate::bubble_fit::JointRoom {
                    room: slot.plot_room.size(),
                    padding,
                })
            })
            .collect();
        let refitted = fits.update(&resolved, &plots, &joint);
        // A reference has data limits where bubbles do not fit: the final figure reports them.
        if let (false, Some(failure)) = (
            matches!(rooms, Rooms::Reference { .. }),
            fits.failure(&marks),
        ) {
            return Err(Unplanned::Bubbles(failure));
        }
        if refitted {
            (resolved, bands, solution) = refit(&fits, &choices)?;
            continue;
        }
        for (i, r) in resolved.iter().enumerate() {
            for side in Side::BOTH {
                if side.groups(groups)[i] != i {
                    continue;
                }
                let plot = side.group_plot(groups, i, &plots, &show_labels);
                let ticks = side.ticks(r);
                let axis = match side {
                    Side::X => &r.x,
                    Side::Y => &r.y,
                };
                if axis.is_category() {
                    continue;
                }
                if !side.is_automatic(&limits[i])
                    || side.axis_room(plot, &axis_ticks[i]).slack(ticks) >= 0.0
                {
                    continue;
                }
                let choice = &mut next[i][side.index()];
                let retry_pair = axis
                    .continuous()
                    .is_some_and(|scale| scale.transform() == crate::Transform::Linear);
                match choice.again(plot, ticks.len(), retry_pair) {
                    Some(new) => {
                        *choice = new;
                        again.push((i, side));
                    }
                    None => choice.state = ChoiceState::Settled,
                }
            }
        }
        let bar_plots = color_plots(&solution);
        let mut color_again = Vec::new();
        for (i, r) in resolved.iter().enumerate() {
            let Some(color) = &r.color else {
                continue;
            };
            let Some(side) = color_sides[i] else {
                continue;
            };
            if side.room(bar_plots[i], &tick).slack(&color.ticks) >= 0.0 {
                continue;
            }
            let choice = &mut next[i][2];
            match choice.again(bar_plots[i], color.ticks.len(), false) {
                Some(new) => {
                    *choice = new;
                    color_again.push(i);
                }
                None => choice.state = ChoiceState::Settled,
            }
        }
        if again.is_empty() && color_again.is_empty() {
            break;
        }
        let tried_resolved = resolve_choices(&next, &fits);
        for &(i, side) in &again {
            // A choice that crowds even on the plot it was made for is the fallback.
            let choice = &mut next[i][side.index()];
            if choice
                .room(side, &axis_ticks[i][side.index()])
                .slack(side.ticks(&tried_resolved[i]))
                < 0.0
            {
                choice.state = ChoiceState::Settled;
            }
        }
        for &i in &color_again {
            if let Some(color) = &tried_resolved[i].color
                && let Some(side) = color_sides[i]
                && next[i][2].room(side, &tick).slack(&color.ticks) < 0.0
            {
                next[i][2].state = ChoiceState::Settled;
            }
        }
        match solve_ticks(tried_resolved) {
            Ok(tried) => {
                (resolved, bands, solution) = tried;
                choices = next;
            }
            Err(_) => {
                // The grid cannot hold the new choices: those sides keep their ticks.
                for &(i, side) in &again {
                    choices[i][side.index()].state = ChoiceState::Settled;
                }
                for &i in &color_again {
                    choices[i][2].state = ChoiceState::Settled;
                }
            }
        }
    }

    let plots: Vec<_> = solution
        .panels
        .iter()
        .map(|slot| slot.plot.size())
        .collect();
    if fits.clip_unsettled(&resolved, &plots) {
        (resolved, bands, solution) = refit(&fits, &choices)?;
    }
    // The limits of the data positions would keep the scales equal, but a plot that clips its
    // bubbles has not settled.
    if let Some(i) = (0..panels.len()).find(|&i| equal_aspect[i] && fits.clipped(i)) {
        return Err(in_cell(
            figure,
            panels[i].axes,
            Error::style(
                context,
                "the bubbles and the equal aspect of these axes do not settle at this plot size.",
            )
            .with_hint("make the figure larger, or set explicit limits, which clip the bubbles."),
        )
        .into());
    }

    // A single label can release room that the previous crowded set needed. A crowded set
    // can also survive a failed solve: the preferred wider step may need wider labels, while
    // another pair fits. Try pairs, then a single linear tick, on their own layouts without
    // moving any domain. An accepted change leaves every label apart; fitting pairs are never
    // retried, so this finishes even when the axes affect each other.
    loop {
        let mut changed = false;
        for side in [Side::Y, Side::X] {
            for i in 0..panels.len() {
                if side.groups(groups)[i] != i {
                    continue;
                }
                let plots: Vec<_> = solution
                    .panels
                    .iter()
                    .map(|slot| slot.plot.size())
                    .collect();
                let plot = side.group_plot(groups, i, &plots, &show_labels);
                let ticks = side.ticks(&resolved[i]);
                if ticks.len() != 1 && side.axis_room(plot, &axis_ticks[i]).slack(ticks) >= 0.0 {
                    continue;
                }
                let resolve::AxisLimits::Continuous(axis) = limits[i].axis(side.axis()) else {
                    continue;
                };
                if axis.ticks.explicit().is_some() {
                    continue;
                }
                let scale = match side {
                    Side::X => resolved[i].x.continuous(),
                    Side::Y => resolved[i].y.continuous(),
                };
                let Some(scale) = scale.copied() else {
                    continue;
                };
                let mut candidates: Vec<Vec<_>> = if scale.transform() == crate::Transform::Linear {
                    scale
                        .tick_pairs(axis.ticks.format())
                        .into_iter()
                        .map(|pair| {
                            pair.into_iter()
                                .map(|tick| resolve::AxisTick {
                                    pos: NormalizedPos::new(scale.normalize(tick.value)),
                                    label: tick.label,
                                })
                                .collect()
                        })
                        .collect()
                } else if ticks.len() > 2 {
                    // Keep the existing outer log ticks: refitting fewer powers can widen
                    // automatic limits and their labels, making the same solve fail again.
                    vec![vec![ticks[0].clone(), ticks[ticks.len() - 1].clone()]]
                } else {
                    Vec::new()
                };
                if scale.transform() == crate::Transform::Linear && ticks.len() > 1 {
                    let (lo, hi) = scale.domain();
                    let middle = if lo <= 0.0 && hi >= 0.0 {
                        scale.normalize(0.0)
                    } else {
                        0.5
                    };
                    let tick = ticks.iter().min_by(|a, b| {
                        (a.pos.get() - middle)
                            .abs()
                            .total_cmp(&(b.pos.get() - middle).abs())
                    });
                    if let Some(tick) = tick {
                        candidates.push(vec![tick.clone()]);
                    }
                }
                for ticks in candidates {
                    let mut tried = resolved.clone();
                    for (j, r) in tried.iter_mut().enumerate() {
                        if side.groups(groups)[j] == i {
                            match side {
                                Side::X => r.x.ticks = ticks.clone(),
                                Side::Y => r.y.ticks = ticks.clone(),
                            }
                        }
                    }
                    let Ok((tried, tried_bands, tried_solution)) = solve_ticks(tried) else {
                        continue;
                    };
                    let axes_fit = tried_solution.panels.iter().enumerate().all(|(j, slot)| {
                        Side::BOTH.into_iter().all(|side| {
                            !show_labels[j][side.index()]
                                || !side.is_automatic(&limits[j])
                                || side
                                    .axis_room(slot.plot.size(), &axis_ticks[j])
                                    .slack(side.ticks(&tried[j]))
                                    >= 0.0
                        })
                    });
                    let bars = color_plots(&tried_solution);
                    let colors_fit =
                        tried
                            .iter()
                            .enumerate()
                            .all(|(j, r)| match (color_sides[j], &r.color) {
                                (Some(side), Some(color)) => {
                                    side.room(bars[j], &tick).slack(&color.ticks) >= 0.0
                                }
                                _ => true,
                            });
                    let plots: Vec<_> = tried_solution
                        .panels
                        .iter()
                        .map(|slot| slot.plot.size())
                        .collect();
                    if axes_fit && colors_fit && !fits.clone().clip_unsettled(&tried, &plots) {
                        (resolved, bands, solution) = (tried, tried_bands, tried_solution);
                        changed = true;
                        break;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Check only final domains: shared data, cells and bubble fitting can widen them, and
    // provisional layouts may deliberately omit automatic x ticks.
    for (i, r) in resolved.iter().enumerate().filter(|_| rooms.is_final()) {
        if let (true, Some(x), Some(y)) = (equal_aspect[i], r.x.continuous(), r.y.continuous()) {
            layout::check_equal_aspect_size(&solution.panels[i], [x, y], context)
                .map_err(|error| in_cell(figure, panels[i].axes, error))?;
        }
        for (side, axis, name) in [(Side::X, &r.x, "x"), (Side::Y, &r.y, "y")] {
            if let resolve::AxisLimits::Continuous(limits) = limits[i].axis(side.axis())
                && limits.transform == crate::Transform::Linear
                && limits.ticks.explicit().is_none()
                && let Some(scale) = axis.continuous()
            {
                limits
                    .ticks
                    .format()
                    .check_automatic_ticks(
                        &axis.ticks,
                        scale.domain(),
                        AutomaticTickTarget::Axis(name),
                        context,
                    )
                    .map_err(|error| in_cell(figure, panels[i].axes, error))?;
            }
        }
        if color_sides[i].is_some()
            && let Some(color) = &r.color
            && !matches!(color.norm, crate::color_scale::Norm::Log { .. })
            && let Some((format, target)) = color_tick_format(figure, &panels[i])
        {
            format
                .check_automatic_ticks(&color.ticks, color.norm.range(), target, context)
                .map_err(|error| in_cell(figure, panels[i].axes, error))?;
        }
    }

    layout::twin::align_placed(&inputs, &mut bands, &solution);
    let mut planned: Vec<(Resolved, Layout)> = inputs
        .iter()
        .zip(resolved)
        .zip(bands)
        .zip(&solution.panels)
        .map(|(((input, resolved), bands), slot)| {
            let layout = layout::place_panel(input, &resolved, bands, slot);
            (resolved, layout)
        })
        .collect();
    for (i, input) in inputs.iter().enumerate() {
        if let Layer::Twin { parent, .. } = input.axes {
            let (before, after) = planned.split_at_mut(i);
            inputs[parent]
                .legend
                .direct
                .separate_twins(&mut before[parent].1, &mut after[0].1);
        }
        let layout = &planned[i].1;
        if input.legend.position != crate::LegendPosition::Best
            || layout.legend.is_empty()
            || !layout::placement::best_fits(input.legend, &layout.plot)
        {
            continue;
        }
        let twin = panels
            .iter()
            .enumerate()
            .find_map(|(j, panel)| match panel.axes {
                Layer::Primary(_) => None,
                Layer::Twin { parent, .. } => (parent == i).then_some(j),
            });
        let marks = std::iter::once(i)
            .chain(twin)
            .flat_map(|j| {
                let panel = &panels[j];
                let (resolved, layout) = &planned[j];
                panel.marks.iter().map(move |mark| {
                    let item =
                        draw::marks(theme, lib, resolved, layout, std::slice::from_ref(mark))
                            .map_err(|err| in_cell(figure, panel.axes, err))?;
                    let geometry = item.map(|item| crate::coverage::Geometry::of(&item));
                    Ok(mark.for_coverage(geometry))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        planned[i].1.legend = layout::placement::best(input.legend, &layout.plot, &marks);
    }
    let (resolved, layouts): (Vec<Resolved>, Vec<Layout>) = planned.into_iter().unzip();
    if rooms.is_final() {
        layout::check_panel_contents(&grid, &layouts, &solution.panels)?;
    }
    Ok(FigurePlan {
        header,
        panels: resolved.into_iter().zip(layouts).collect(),
        solution,
        bubbles_settled: fits.settled(),
    })
}

/// Resolves cell geometry against the solved plot, then settles its limits and measured
/// labels together. Automatic y limits keep the chosen lattice and vary only its room.
/// An explicit y window needs a fresh count when the plot narrows, including when the
/// colorbar first appears. A wider plot can grow automatic x limits without changing rows;
/// with both limits fixed, any aspect change needs a recount. Fitting never stretches the
/// coordinate map of counted cells.
///
/// Several hexbins with points on one axes are counted in the metric of the axes, as shared
/// axes are, so that all of their hexagons are regular on the same limits. A metric in which a
/// hexbin's own range spans more than [`MAX_LATTICE_LENGTH`](crate::bin2d::MAX_LATTICE_LENGTH)
/// cells cannot count it exactly, and is an error.
///
/// On axes with equal aspect the plot takes its shape from the limits, so the lattice cannot
/// adapt to the plot: it is counted once, in the metric of the first limits, whose data units
/// are equally long, so its hexagons are regular on any plot with equal scales. An independent
/// lattice would round its rows, and leave them stretched. Automatic limits then grow to hold
/// whole cells, and the next pass finds nothing left to do. Those limits can give a very
/// different plot from the first, so only the settled layout is checked ([`Rooms::Provisional`]).
///
/// A lone automatic hexbin is fitted tightly: its whole cells plus the least room, on one
/// axis, that gives the plot's shape. That room moves the automatic ticks, and their labels
/// can change the plot's shape back, so tight fits can alternate between shapes forever.
/// Tight fits end on either of two triggers. After the first pass, each pass depends only on
/// the plot aspects, the axis domains and the aspects the lattices were counted at; when all
/// of them repeat those of an earlier pass, the passes repeat forever. And a lone panel can
/// alternate while another panel's limits keep growing, so that nothing repeats: tight fits
/// also end at pass [`TIGHT_PASSES`], half the budget. From then on each tight fit instead only
/// grows the limits of the last pass to the new shape, as shared and mixed axes always do: the
/// room that gave a shape is kept, so the limits stop moving once the labels keep the shape. A
/// figure that settles within [`TIGHT_PASSES`] passes lays out as with tight fits alone; one
/// that needed more tight passes may lay out differently. The passes are still bounded, and a
/// layout that does not settle within them is an error.
fn plan_panels<'a>(
    figure: &'a Figure,
    panels: &mut [Panel<'a>],
    groups: &ShareGroups,
    lib: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
    rooms: Rooms<'_>,
) -> Result<FigurePlan> {
    match lay_out(figure, panels, groups, lib, context, labels, rooms) {
        Ok(planned) => {
            if let Rooms::Reference { settled, .. } = rooms {
                settled.replace(Some(Reference {
                    solution: planned.solution.clone(),
                    hexbins: panels.iter().map(hexbin_counts).collect(),
                }));
            }
            Ok(planned)
        }
        Err(Unplanned::Failed(error)) => Err(error),
        Err(Unplanned::Bubbles(failure)) => Err(in_cell(
            figure,
            panels[failure.panel].axes,
            bubbles_error(figure, failure, lib, context, labels),
        )),
    }
}

/// [`plan_panels`] for a figure as [`scene`] lays it out, its rooms found first.
#[cfg(test)]
fn plan_cells<'a>(
    figure: &'a Figure,
    panels: &mut [Panel<'a>],
    groups: &ShareGroups,
    lib: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
) -> Result<FigurePlan> {
    let reference = reference_rooms(figure, groups, lib, context, labels)?;
    let rooms = Rooms::of(reference.as_ref());
    plan_panels(figure, panels, groups, lib, context, labels, rooms)
}

/// The hexbin counts of `panel`, in mark order.
fn hexbin_counts(panel: &Panel<'_>) -> Vec<crate::bin2d::HexBins> {
    panel
        .marks
        .iter()
        .filter_map(|mark| match mark {
            ResolvedMark::Hexbin { bins, .. } => Some(bins.clone()),
            _ => None,
        })
        .collect()
}

/// The same figure as `figure` without equal aspect, for the plots of `figure` to shrink in;
/// `None` for a figure without the setting, which solves the grid itself.
///
/// The reference is that figure laid out and drawn to the end, with its own hexbin counts and
/// the sides of its own value labels, so its rooms hold nothing that the setting changed.
///
/// What fails on a panel without the setting fails `figure`, so it is raised here, with the cell
/// it names. A panel with the setting is the exception: it is laid out and drawn again on
/// another plot, so cells that cannot be counted, marks and labels that do not fit and labels
/// that do not settle are left aside for `figure` to judge ([`Rooms::Reference`]). Only a
/// completed layout counts as a reference, so every hexbin of a panel without the setting in
/// it is counted and settled.
///
/// # Errors
///
/// As [`plan_panels`] and [`draw_scene`], and the error of labels that do not settle, for what
/// fails outside the panels with the setting.
fn reference_rooms(
    figure: &Figure,
    groups: &ShareGroups,
    fonts: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
) -> Result<Option<Reference>> {
    if !figure.all_cartesian_axes().any(Axes::has_equal_aspect) {
        return Ok(None);
    }
    let plain = figure.without_equal_aspect();
    let equal: Vec<_> = figure
        .layers()
        .map(|layer| layer.cartesian().is_some_and(Axes::has_equal_aspect))
        .collect();
    let mut complete = None;
    let mut pass = 0;
    let scene = point_labels::fit_then_place(|held| {
        pass += 1;
        let settled = std::cell::RefCell::new(None);
        let mut records = panels(&plain, groups, fonts)?;
        if let Some(held) = held {
            for (panel, sides) in records.iter_mut().zip(held) {
                panel.points.hold(sides.clone());
            }
        }
        let rooms = Rooms::Reference {
            settled: &settled,
            equal: &equal,
        };
        let (scene, mut placements) =
            draw_scene(&plain, &mut records, groups, fonts, context, labels, rooms)?;
        complete = settled.into_inner().or(complete.take());
        if pass == point_labels::MAX_PASSES {
            for (placement, _) in placements
                .iter_mut()
                .zip(&equal)
                .filter(|(_, equal)| **equal)
            {
                placement.accept();
            }
        }
        Ok((scene, placements))
    })?;
    scene.map(|_| complete)
}

/// The most layouts [`bubbles_error`] makes of a figure at larger sizes.
const MAX_PROBES: usize = 40;

/// The most of those layouts that grow the figure before the largest figure is tried.
const GROWTH_PROBES: usize = 20;

#[cfg(test)]
thread_local! {
    /// How many layouts [`bubbles_error`] made for the last error it built.
    pub(crate) static BUBBLE_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The error for bubbles that no automatic limits hold in the layout of `figure`, as
/// `failure` names them, reported in `context` with the fonts `lib` and the cache `labels`.
///
/// It names the smallest figure size, in whole points, whose own layout holds every drawn
/// bubble, found by laying the same figure out at larger sizes, so that every layout effect,
/// padding, log-axis finishing, tick labels, shared panels and spanned cells included, is the
/// real one: the sides that the failing fits need grow, from the failure's estimate
/// ([`crate::bubble_fit::BubbleFailure::estimate`]) and then doubling their growth, until a
/// layout holds the bubbles; then each grown side is bisected on whole points. A layout holds
/// them when it is made and every bubble fit settled. At most [`MAX_PROBES`] layouts are
/// made; the size named is always one whose layout held them. When even the largest figure
/// ([`crate::model::MAX_FIGURE_IN`] on the grown sides) does not, the error names the axis
/// whose limits would leave the supported range.
fn bubbles_error(
    figure: &Figure,
    failure: crate::bubble_fit::BubbleFailure,
    lib: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
) -> Error {
    let largest = crate::model::MAX_FIGURE_IN * 72.0;
    let own = [figure.width_in() * 72.0, figure.height_in() * 72.0];
    let probes = std::cell::Cell::new(0);
    // Whether the figure at `size` lays out holding every drawn bubble; the bubbles that do
    // not fit when it does not, if that is why.
    let holds = |size: [f64; 2]| -> std::result::Result<(), Option<_>> {
        probes.set(probes.get() + 1);
        let resized = figure.resized(size[0] / 72.0, size[1] / 72.0);
        let groups = ShareGroups::new(&resized);
        let mut records = panels(&resized, &groups, lib).map_err(|_| None)?;
        let reference =
            reference_rooms(&resized, &groups, lib, context, labels).map_err(|_| None)?;
        let rooms = Rooms::of(reference.as_ref());
        match lay_out(&resized, &mut records, &groups, lib, context, labels, rooms) {
            Ok(planned) if planned.bubbles_settled => Ok(()),
            Ok(_) | Err(Unplanned::Failed(_)) => Err(None),
            Err(Unplanned::Bubbles(failure)) => Err(Some(failure)),
        }
    };
    // How much one point more along a side lengthens the panel's plot along it.
    let cells = match figure.grid().rows() * figure.grid().cols() {
        1 => [1.0, 1.0],
        _ => {
            let panel = figure
                .layers()
                .nth(failure.panel)
                .map(|layer| layer.cells());
            panel.map_or([1.0, 1.0], |cells| {
                [
                    figure.grid().cols() as f64 / cells.col_span() as f64,
                    figure.grid().rows() as f64 / cells.row_span() as f64,
                ]
            })
        }
    };
    let plot = [failure.plot.width, failure.plot.height];
    let mut grows = [false; 2];
    grows[failure.side()] = true;
    let mut size = own;
    for side in 0..2 {
        match failure.estimate[side] {
            Some(length) if length > plot[side] || grows[side] => {
                grows[side] = true;
                let growth = ((length - plot[side]) * cells[side]).max(1.0);
                size[side] = (own[side] + growth).ceil().min(largest);
            }
            None if grows[side] => size[side] = largest,
            _ => {}
        }
    }
    // The largest size of each side known not to hold the bubbles: the failing side's own;
    // the other side may hold them as it is.
    let mut short = own.map(|side| side - 1.0);
    short[failure.side()] = own[failure.side()];
    let mut last = failure;
    loop {
        let failed = match holds(size) {
            Ok(()) => break,
            Err(failed) => failed,
        };
        let mut sides = grows;
        if let Some(failed) = failed {
            sides = [false; 2];
            sides[failed.side()] = true;
            last = failed;
        }
        if (0..2).all(|side| !sides[side] || size[side] >= largest) {
            return last.out_of_range(context);
        }
        for side in (0..2).filter(|&side| sides[side]) {
            grows[side] = true;
            short[side] = short[side].max(size[side]);
            size[side] = if probes.get() >= GROWTH_PROBES {
                largest
            } else {
                (size[side] + (size[side] - own[side]).max(36.0))
                    .ceil()
                    .min(largest)
            };
        }
    }
    // Each grown side down to the least whole point, or the figure's own size, that still
    // holds them, the other kept.
    for side in (0..2).filter(|&side| grows[side]) {
        let (mut low, mut high) = (short[side], size[side]);
        while probes.get() < MAX_PROBES {
            // Never below the figure's own size, which the other sides keep.
            let middle = (low + (high - low) / 2.0).ceil().max(own[side]);
            if middle >= high {
                break;
            }
            let mut tried = size;
            tried[side] = middle;
            if holds(tried).is_ok() {
                high = middle;
                size = tried;
            } else {
                low = middle;
            }
        }
    }
    #[cfg(test)]
    BUBBLE_PROBES.with(|count| count.set(probes.get()));
    last.needs_figure(context, size, own)
}

/// Whether the final figure judges the panel at `index` itself, so that nothing it does in
/// `rooms` is an error: on a reference, a panel that has equal aspect there, whose cells are
/// counted and whose marks and labels are placed again on another plot.
fn judged_later(rooms: Rooms<'_>, index: usize) -> bool {
    matches!(rooms, Rooms::Reference { equal, .. } if equal[index])
}

/// The most layouts [`lay_out`] makes of a figure to settle its arrows.
const MAX_ARROW_LAYOUTS: usize = 16;

/// How far two numbers of an arrow frame may lie apart, as a share of the larger, to count as
/// settled: well below what a coordinate of the scene, an `f32`, can show.
const ARROW_TOLERANCE: f64 = 1e-6;

/// Whether the frames `a` and `b` measure the same arrows, to [`ARROW_TOLERANCE`].
fn same_frame(a: &ArrowFrame, b: &ArrowFrame) -> bool {
    let close = |a: f64, b: f64| (a - b).abs() <= ARROW_TOLERANCE * a.abs().max(b.abs());
    close(a.pt_per_unit, b.pt_per_unit)
        && close(a.key_length, b.key_length)
        && close(a.scales[0], b.scales[0])
        && close(a.scales[1], b.scales[1])
}

/// The frames of the quivers of each panel, in mark order.
type PanelFrames = Vec<Vec<ArrowFrame>>;

/// The quivers of `panel` with their frames, in mark order.
fn quivers<'p, 'a>(
    panel: &'p Panel<'a>,
) -> impl Iterator<Item = (&'a crate::QuiverMark, &'p ArrowFrame)> {
    panel.marks.iter().filter_map(|mark| match mark {
        ResolvedMark::Quiver { mark, frame, .. } => Some((*mark, frame)),
        _ => None,
    })
}

/// The frames `panels` hold now.
fn held_frames(panels: &[Panel<'_>]) -> PanelFrames {
    panels
        .iter()
        .map(|panel| quivers(panel).map(|(_, frame)| *frame).collect())
        .collect()
}

/// Whether the quiver `mark` takes the automatic scale of its axes.
fn is_automatic(mark: &crate::QuiverMark) -> bool {
    mark.options().length == crate::ArrowLength::Auto
}

/// Gives `panel` the automatic scale `scale` in its `frames`, keys included.
fn set_automatic_scale(panel: &Panel<'_>, frames: &mut [ArrowFrame], scale: f64) {
    for ((mark, _), frame) in quivers(panel).zip(frames) {
        if is_automatic(mark) {
            frame.pt_per_unit = scale;
            frame.key_length = draw::field::key_length(mark, scale);
        }
    }
}

/// The automatic scale of `panel` in `frames`; `None` without an automatic quiver.
fn automatic_scale(panel: &Panel<'_>, frames: &[ArrowFrame]) -> Option<f64> {
    quivers(panel)
        .zip(frames)
        .find_map(|((mark, _), frame)| is_automatic(mark).then_some(frame.pt_per_unit))
}

/// The frames to lay out after the layout `planned` of `panels`, measured on it
/// ([`draw::field::measured_frames`]), with each automatic scale chosen by its search; `None`
/// when no frame moved past [`ARROW_TOLERANCE`], so that a settled figure is drawn with the very
/// frames its last layout held.
fn next_frames(
    figure: &Figure,
    panels: &[Panel<'_>],
    planned: &FigurePlan,
    searches: &mut [ScaleSearch],
) -> Option<PanelFrames> {
    let mut measured: PanelFrames = panels
        .iter()
        .zip(&planned.panels)
        .map(|(panel, (resolved, layout))| {
            draw::field::measured_frames(
                figure.theme(),
                &panel.marks,
                resolved,
                layout.plot,
                has_equal_aspect(panel),
            )
        })
        .collect();
    let held = held_frames(panels);
    let settled = held
        .iter()
        .flatten()
        .zip(measured.iter().flatten())
        .all(|(a, b)| same_frame(a, b));
    if settled {
        return None;
    }
    for (i, panel) in panels.iter().enumerate() {
        if let (Some(old), Some(new)) = (
            automatic_scale(panel, &held[i]),
            automatic_scale(panel, &measured[i]),
        ) {
            let scale = searches[i].measured(old, new);
            set_automatic_scale(panel, &mut measured[i], scale);
        }
    }
    Some(measured)
}

/// Gives `panels` the frames `frames`, and measures the legends again for their keys.
///
/// # Errors
///
/// As [`MeasuredLegend::measure`].
fn set_frames<'a>(
    figure: &'a Figure,
    panels: &mut [Panel<'a>],
    frames: &PanelFrames,
    lib: &SceneFonts<'_>,
) -> Result<()> {
    let mut keys = false;
    for (panel, frames) in panels.iter_mut().zip(frames) {
        let held = panel.marks.iter_mut().filter_map(|mark| match mark {
            ResolvedMark::Quiver { frame, .. } => Some(frame),
            _ => None,
        });
        for (held, frame) in held.zip(frames) {
            keys |= held.key_length != frame.key_length;
            *held = *frame;
        }
    }
    if keys {
        for panel in panels.iter_mut() {
            panel.legend = layer_legend(figure, panel.axes, &panel.marks, lib)?;
        }
        join_twin_legends(panels);
    }
    Ok(())
}

/// The frames to try after the frames `frames` of `panels` failed with `failure`: a shorter
/// automatic scale for the panel whose arrows no limits hold, and for every panel with an
/// automatic key when the layout failed otherwise, since its key takes room. `None` when no
/// automatic scale can help.
fn lowered_frames(
    panels: &[Panel<'_>],
    frames: &PanelFrames,
    failure: &Unplanned,
    searches: &mut [ScaleSearch],
) -> Option<PanelFrames> {
    let involved: Vec<usize> = match failure {
        Unplanned::Bubbles(failure) if failure.lowers => vec![failure.panel],
        Unplanned::Bubbles(_) => Vec::new(),
        Unplanned::Failed(_) => (0..panels.len())
            .filter(|&i| {
                quivers(&panels[i])
                    .any(|(mark, _)| is_automatic(mark) && mark.options().key.is_some())
            })
            .collect(),
    };
    let mut lowered = frames.clone();
    let mut changed = false;
    for i in involved {
        if let Some(scale) = automatic_scale(&panels[i], &frames[i]) {
            let next = searches[i].failed(scale);
            set_automatic_scale(&panels[i], &mut lowered[i], next);
            changed = true;
        }
    }
    changed.then_some(lowered)
}

/// The first key of `panels` that a plot of `planned` cannot show, among the panels `judged`
/// says to look at: a key on arrows without any length, which has no scale, or a key arrow longer
/// than the shorter side of its plot. `None` when there is none.
fn key_error(
    figure: &Figure,
    panels: &[Panel<'_>],
    planned: &FigurePlan,
    context: &'static str,
    judged: impl Fn(usize) -> bool,
) -> Option<Error> {
    let error = |panel: &Panel<'_>, error: Error| in_cell(figure, panel.axes, error);
    for (i, (panel, (_, layout))) in panels.iter().zip(&planned.panels).enumerate() {
        for (mark, frame) in quivers(panel).filter(|_| judged(i)) {
            let Some(key) = &mark.options().key else {
                continue;
            };
            if frame.pt_per_unit == 0.0 {
                return Some(error(
                    panel,
                    Error::style(
                        context,
                        format!(
                            "the key {} has no scale: none of the arrows has a length.",
                            crate::error::quoted(&key.text)
                        ),
                    )
                    .with_parameter("key")
                    .with_hint("pass pt_per_unit, or remove the key."),
                ));
            }
            let shorter = layout.plot.width.min(layout.plot.height);
            if frame.key_length > shorter {
                return Some(error(
                    panel,
                    Error::style(
                        context,
                        format!(
                            "the arrow of the key {} is {} pt long; the shorter side of the plot \
                             is {} pt.",
                            crate::error::quoted(&key.text),
                            crate::error::number(frame.key_length),
                            crate::error::number(shorter)
                        ),
                    )
                    .with_parameter("key")
                    .with_hint("pass a smaller value to key, or make the figure larger."),
                ));
            }
        }
    }
    None
}

/// [`lay_out_cells`] until the arrows of the figure settle.
///
/// The arrows, the limits and the plot depend on each other. The automatic scale is a spacing in
/// points on the final plot; arrows of that scale reach by a number of points; automatic limits
/// hold those reaches, which changes the points per data unit; and a key, as long as its arrow,
/// takes room from the plot. [`Direction::Data`](crate::Direction::Data) reads the plot's shape
/// as well. So a layout holds and draws one [`ArrowFrame`] per quiver, fixed for that layout, and
/// each layout measures the next frame on its plot, starting from a frame of no length. Limits,
/// arrows and key read the same frame, so whatever frame the last layout had, the limits hold
/// the whole arrows as drawn and the key is exactly as long as its arrow.
///
/// # Convergence
///
/// Everything here is a pure function of the figure and the previous layouts, so the outcome is
/// deterministic. The layouts repeat until the frames they measure are the frames they held, to
/// [`ARROW_TOLERANCE`]. An automatic scale `k` measures a scale `g(k)` that falls as `k` rises,
/// so `g(k) = k` has one solution, and [`ScaleSearch`] closes in on it from both sides: a plot of
/// 20 columns takes four or five layouts, and a few long arrows, for which following `g` alone
/// would never end, a few more. A scale that no limits hold moves the search down, so arrows
/// that cannot be as long as their spacing end as long as limits can hold them. Frames that take
/// no part in a search settle by themselves: a fixed scale gives a fixed key, and the shape of
/// a plot under [`Direction::Data`](crate::Direction::Data) changes by a share of its last change.
///
/// Tick labels and log-axis rounding move a plot in steps rather than smoothly, so `g` can jump
/// over its solution, and no frame then reproduces itself. [`MAX_ARROW_LAYOUTS`] ends that: the
/// last layout made stands, with the guarantees above, and only the longest arrow can differ
/// from the spacing, by at most that step.
fn lay_out<'a>(
    figure: &'a Figure,
    panels: &mut [Panel<'a>],
    groups: &ShareGroups,
    lib: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
    rooms: Rooms<'_>,
) -> std::result::Result<FigurePlan, Unplanned> {
    let mut planned = lay_out_cells(figure, panels, groups, lib, context, labels, rooms)?;
    if panels.iter().all(|panel| quivers(panel).next().is_none()) {
        return Ok(planned);
    }
    let mut searches = vec![ScaleSearch::default(); panels.len()];
    let mut held = held_frames(panels);
    let mut next = next_frames(figure, panels, &planned, &mut searches);
    for _ in 1..MAX_ARROW_LAYOUTS {
        let Some(frames) = next.take() else {
            break;
        };
        set_frames(figure, panels, &frames, lib)?;
        match lay_out_cells(figure, panels, groups, lib, context, labels, rooms) {
            Ok(layout) => {
                planned = layout;
                held = frames;
                next = next_frames(figure, panels, &planned, &mut searches);
            }
            Err(failure) => match lowered_frames(panels, &frames, &failure, &mut searches) {
                Some(lowered) => next = Some(lowered),
                // A key too long for any plot can break the layout before it can be judged.
                None => {
                    return Err(
                        match key_error(figure, panels, &planned, context, |_| true) {
                            Some(error) => Unplanned::Failed(error),
                            None => failure,
                        },
                    );
                }
            },
        }
    }
    set_frames(figure, panels, &held, lib)?;
    match key_error(figure, panels, &planned, context, |i| {
        !judged_later(rooms, i)
    }) {
        Some(error) => Err(Unplanned::Failed(error)),
        None => Ok(planned),
    }
}

/// [`plan_panels`] without naming a figure size for bubbles that no automatic limits hold, and
/// with the frames of its arrows as they stand.
fn lay_out_cells(
    figure: &Figure,
    panels: &mut [Panel<'_>],
    groups: &ShareGroups,
    lib: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
    rooms: Rooms<'_>,
) -> std::result::Result<FigurePlan, Unplanned> {
    if !panels.iter().any(|panel| {
        panel
            .marks
            .iter()
            .any(|mark| matches!(mark, ResolvedMark::Hexbin { .. }))
    }) {
        return plan(figure, panels, groups, lib, context, labels, rooms);
    }
    // Cells set the limits only once counted, so the plans before that decide nothing. A panel
    // without equal aspect has the cells of its reference, which fit the plot it has there.
    let passing = match rooms {
        Rooms::Given(reference) => {
            for (panel, counts) in panels.iter_mut().zip(&reference.hexbins) {
                if !has_equal_aspect(panel) {
                    let marks = panel.marks.iter_mut().filter_map(|mark| match mark {
                        ResolvedMark::Hexbin { bins, .. } => Some(bins),
                        _ => None,
                    });
                    for (bins, count) in marks.zip(counts) {
                        bins.clone_from(count);
                    }
                }
            }
            resolve_colors(figure, panels, groups);
            Rooms::Provisional(reference)
        }
        Rooms::Solved | Rooms::Reference { .. } | Rooms::Provisional(_) => rooms,
    };
    let mut planned = plan(figure, panels, groups, lib, context, labels, passing)?;
    let axis_scales = |planned: &FigurePlan| {
        planned
            .panels
            .iter()
            .map(|(r, _)| {
                [&r.x, &r.y].map(|axis| {
                    axis.continuous()
                        .copied()
                        .unwrap_or_else(|| crate::ContinuousScale::unit(crate::Transform::Linear))
                })
            })
            .collect::<Vec<_>>()
    };
    // Recount from the original metric, not from domains already enlarged for end caps.
    let counting_scales = axis_scales(&planned);
    let mut counting_aspects: Vec<_> = planned
        .panels
        .iter()
        .map(|(_, layout)| layout.plot.width / layout.plot.height)
        .collect();
    let mut seen_states = Vec::new();
    let mut tight = true;
    let mut moving: Vec<bool> = Vec::new();
    for pass in 0..64 {
        let aspects: Vec<_> = planned
            .panels
            .iter()
            .map(|(_, layout)| layout.plot.width / layout.plot.height)
            .collect();
        let scales = axis_scales(&planned);
        if pass >= TIGHT_PASSES && !keeps_tight_fits() {
            tight = false;
        } else if pass > 0 && tight && !keeps_tight_fits() {
            // Everything this pass reads: a repeat means the passes cycle.
            let domains: Vec<_> = scales.iter().map(|s| s.map(|s| s.domain())).collect();
            let state = (aspects.clone(), domains, counting_aspects.clone());
            tight = !seen_states.contains(&state);
            seen_states.push(state);
        }
        for (i, panel) in panels.iter_mut().enumerate() {
            let Some(axes) = panel.axes.cartesian() else {
                continue;
            };
            let equal = has_equal_aspect(panel);
            if matches!(passing, Rooms::Provisional(_)) && !equal {
                continue;
            }
            let limits = [axes.x_limits(), axes.y_limits()];
            let shared = groups.x.iter().filter(|&&g| g == groups.x[i]).count() > 1
                || groups.y.iter().filter(|&&g| g == groups.y[i]).count() > 1;
            // Hexbins without paired finite points count no cells, and leave the others alone.
            let hexbins = panel
                .marks
                .iter()
                .filter(|m| {
                    matches!(m, ResolvedMark::Hexbin { mark, .. } if !mark.extents[0].is_empty())
                })
                .count();
            let independent = !shared && !equal && limits == [None, None] && hexbins <= 1;
            let only_hexbin = panel
                .marks
                .iter()
                .all(|m| matches!(m, ResolvedMark::Hexbin { .. }));
            let aspect_change = aspects[i] / counting_aspects[i] - 1.0;
            let recount = !equal
                && limits[1].is_some()
                && if limits[0].is_some() {
                    aspect_change.abs() >= 1e-12
                } else {
                    aspect_change < -1e-12
                };
            let axes = panel.axes;
            let mut count = || -> std::result::Result<(), Unplanned> {
                for mark in &mut panel.marks {
                    if let ResolvedMark::Hexbin { mark, bins, .. } = mark {
                        if pass == 0 || recount {
                            let counted = if independent {
                                crate::bin2d::HexBins::new(
                                    &mark.x,
                                    &mark.y,
                                    mark.gridsize,
                                    mark.transforms,
                                    aspects[i],
                                )
                            } else {
                                let [x, y] = counting_scales[i].map(|scale| {
                                    let (lo, hi) = scale.domain();
                                    hi - lo
                                });
                                crate::bin2d::HexBins::on_axes(
                                    &mark.x,
                                    &mark.y,
                                    mark.gridsize,
                                    counting_scales[i],
                                    // The shape of the limits, which a provisional plot may not
                                    // have, and no room for end caps: both keep the metric equal.
                                    if equal { x / y } else { aspects[i] },
                                    if equal { [None, None] } else { limits },
                                )
                            };
                            *bins =
                                counted.map_err(|_| {
                                    in_cell(
                                figure,
                                axes,
                                crate::Error::data(
                                    context,
                                    "the hexbin ranges are too far apart for one lattice of \
                                     regular hexagons on these axes.",
                                )
                                .with_hint("draw them on separate axes, or set explicit limits."),
                            )
                                })?;
                        }
                        if equal {
                            bins.fit_equal_aspect(scales[i], limits);
                        } else if independent && only_hexbin && tight {
                            bins.fit(aspects[i]);
                        } else {
                            bins.fit_on_axes(aspects[i], scales[i], limits, context)
                                .map_err(Unplanned::Failed)?;
                        }
                        for j in 0..2 {
                            if let [lo, hi] = bins.domain(j) {
                                crate::ContinuousScale::new(mark.transforms[j], *lo, *hi, context)
                                    .map_err(|_| {
                                        crate::Error::data(
                                            context,
                                            "the whole hexagons exceed the supported axis range; \
                                         their bounds must fit a numeric axis.",
                                        )
                                        .with_parameter(if j == 0 { "x" } else { "y" })
                                        .with_hint("rescale the coordinates, or use hist2d().")
                                    })?;
                            }
                        }
                    }
                }
                Ok(())
            };
            match count() {
                Err(_) if judged_later(passing, i) => {}
                outcome => outcome?,
            }
            if pass == 0 || recount {
                counting_aspects[i] = aspects[i];
            }
        }
        resolve_colors(figure, panels, groups);
        planned = plan(figure, panels, groups, lib, context, labels, passing)?;
        moving = planned
            .panels
            .iter()
            .zip(&aspects)
            .zip(&scales)
            .map(|(((r, layout), aspect), old)| {
                !(((layout.plot.width / layout.plot.height) / aspect - 1.0).abs() < 1e-12
                    && [&r.x, &r.y].iter().zip(old).all(|(new, old)| {
                        let Some(new) = new.continuous() else {
                            return true;
                        };
                        let (lo, hi) = new.domain();
                        old.normalize(lo).abs() < 1e-10 && (old.normalize(hi) - 1.0).abs() < 1e-10
                    }))
            })
            .collect();
        if !moving.contains(&true) {
            #[cfg(test)]
            SETTLED_PASS.with(|settled| settled.set(pass));
            return match passing {
                Rooms::Provisional(_) => plan(figure, panels, groups, lib, context, labels, rooms),
                Rooms::Solved | Rooms::Reference { .. } | Rooms::Given(_) => Ok(planned),
            };
        }
    }
    let settled_enough = moving
        .iter()
        .enumerate()
        .all(|(i, moves)| !moves || judged_later(passing, i));
    if settled_enough {
        return Ok(planned);
    }
    Err(crate::Error::data(
        context,
        "the hexagon bounds and tick labels do not settle at this plot size; \
         a stable numeric range is required.",
    )
    .with_parameter("gridsize")
    .with_hint("choose a larger gridsize, or increase the figure size.")
    .into())
}

/// The passes after which [`plan_panels`] stops fitting lone hexbins tightly.
pub(crate) const TIGHT_PASSES: usize = 32;

#[cfg(test)]
thread_local! {
    /// Whether [`plan_panels`] keeps fitting lone hexbins tightly through every pass.
    pub(crate) static KEEP_TIGHT_FITS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The pass at which [`plan_panels`] last settled.
    pub(crate) static SETTLED_PASS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Whether lone hexbins stay tightly fitted through every pass, as tests may ask.
#[cfg(test)]
fn keeps_tight_fits() -> bool {
    KEEP_TIGHT_FITS.with(std::cell::Cell::get)
}

/// Whether lone hexbins stay tightly fitted through every pass: never.
#[cfg(not(test))]
fn keeps_tight_fits() -> bool {
    false
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
    library: &FontLibrary,
) -> Result<(Resolved, Layout)> {
    let index = panel_index(figure, cell);
    let fonts = layout::figure_fonts(library, figure.theme())?.in_context("Figure.to_scene()");
    let lib = &fonts;
    let groups = ShareGroups::new(figure);
    let mut panels = panels(figure, &groups, lib)?;
    let labels = layout::LabelCache::new(lib);
    let mut planned = plan_cells(
        figure,
        &mut panels,
        &groups,
        lib,
        "Figure.to_scene()",
        &labels,
    )?;
    Ok(planned.panels.swap_remove(index))
}

/// [`plan`] for every axes of `figure`, in the order of [`Figure::panels`], together with the
/// axes' marks as they are drawn: hexbins counted on the solved lattice. Panics when the figure
/// cannot be laid out.
#[cfg(test)]
pub(crate) fn plan_panel_marks<'a>(
    figure: &'a Figure,
    library: &FontLibrary,
) -> Vec<(Resolved, Layout, Vec<ResolvedMark<'a>>)> {
    let fonts = layout::figure_fonts(library, figure.theme())
        .expect("the fonts resolve")
        .in_context("Figure.to_scene()");
    let lib = &fonts;
    let groups = ShareGroups::new(figure);
    let mut panels = panels(figure, &groups, lib).expect("the legends lay out");
    let labels = layout::LabelCache::new(lib);
    let planned = plan_cells(
        figure,
        &mut panels,
        &groups,
        lib,
        "Figure.to_scene()",
        &labels,
    )
    .expect("the figure lays out");
    planned
        .panels
        .into_iter()
        .zip(panels)
        .map(|((resolved, layout), panel)| (resolved, layout, panel.marks))
        .collect()
}

/// [`plan`] for every axes of `figure`, in the order of [`Figure::panels`], with layout errors
/// reported in `"Figure.to_scene()"` as [`scene`] reports them.
#[cfg(test)]
pub(crate) fn try_plan_panels(
    figure: &Figure,
    library: &FontLibrary,
) -> Result<Vec<(Resolved, Layout)>> {
    let fonts = layout::figure_fonts(library, figure.theme())?.in_context("Figure.to_scene()");
    let lib = &fonts;
    let groups = ShareGroups::new(figure);
    plan_cells(
        figure,
        &mut panels(figure, &groups, lib)?,
        &groups,
        lib,
        "Figure.to_scene()",
        &layout::LabelCache::new(lib),
    )
    .map(|planned| planned.panels)
}

/// The index, in the order of [`Figure::panels`], of the axes of `figure` whose top-left cell
/// is at `cell`. Panics when the cell holds no axes.
#[cfg(test)]
fn panel_index(figure: &Figure, (row, col): (usize, usize)) -> usize {
    figure
        .layers()
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
    library: &FontLibrary,
    f: impl FnOnce(&LayoutInput<'_>, &GridInput<'_>) -> R,
) -> R {
    let index = panel_index(figure, cell);
    let fonts = layout::figure_fonts(library, figure.theme())
        .expect("the fonts resolve")
        .in_context("Figure.to_scene()");
    let lib = &fonts;
    let groups = ShareGroups::new(figure);
    let panels = panels(figure, &groups, lib).expect("the legends lay out");
    let header = figure_header(figure, &panels, lib).expect("the figure header lays out");
    let inputs: Vec<LayoutInput<'_>> = panels
        .iter()
        .map(|panel| LayoutInput {
            theme: figure.theme(),
            figure_title_size: figure.title_size(),
            axes: panel.axes,
            legend: &panel.legend,
            column: panel.points.column(),
            lib,
            context: "Figure.to_scene()",
        })
        .collect();
    let grid = GridInput {
        width: figure.width_in() * 72.0,
        height: header.grid_height(figure),
        caption: header.caption.room(),
        top: header.bottom,
        grid: figure.grid(),
        theme: figure.theme(),
        context: "Figure.to_scene()",
        panels: &inputs,
        colorbars: &[],
    };
    f(&inputs[index], &grid)
}

/// The scene of `figure`: `width × 72` by `height × 72` pt on the theme background, holding
/// for each placed axes, in row-major order of their top-left cells,
/// - the axes chrome without the baseline, as one unclipped group;
/// - the visible marks, in insertion order, as one group clipped to the plot area (widened
///   horizontally for line ends, see `draw::marks_clip`);
/// - the values written on annotated heatmap cells, when there are any, as one group clipped
///   to the plot area;
/// - the values written on annotated bars, when there are any, as one unclipped group
///   ([`bar_labels::items`]);
/// - the values written next to annotated scatter points, when there are any, as one
///   unclipped group ([`point_labels::items`]);
/// - the legend, when it has row entries or direct labels, as one unclipped group;
/// - the baseline, as an unclipped group of its own, when the axes hold a mark that grows
///   from zero, unless overridden by `AxesMut::baseline` (see [`Resolved::draws_baseline`]);
/// - the colorbar of the axes, when it has one of its own.
///
/// The shared colorbars and the figure header follow the panels. Last, over everything else,
/// comes the text each axes writes at data positions or positions in the plot, with its
/// arrows, as one unclipped group per axes that has any, in the order of the axes
/// ([`draw::annotations`]): it may reach past its cells, and no later panel, colorbar or header
/// covers it.
///
/// The scene's fonts are the figure's ([`layout::figure_fonts`]): the theme's title font, then
/// its body font.
///
/// Every axes is laid out by one solve of the grid ([`plan`]); a figure of one axes has the
/// canvas as its frame. The figure header follows the panels as its own unclipped group when
/// non-empty; without axes the scene holds the background and that header. Each axes' marks are
/// resolved and its legend is built and measured once, before layout, and the same records
/// serve layout and drawing. When automatic value labels of scatter points leave the plot,
/// the figure is measured, laid out and drawn again so that automatic limits hold them
/// ([`point_labels::fit_then_place`]).
///
/// # Errors
///
/// As [`plan`], reported in `context`, the API call that renders.
pub(crate) fn scene(
    figure: &Figure,
    library: &FontLibrary,
    context: &'static str,
) -> Result<Scene> {
    let fonts = layout::figure_fonts(library, figure.theme())?.in_context(context);
    scene_with_labels(figure, &fonts, context, &layout::LabelCache::new(&fonts))
}

/// Draws with a cache owned by this scene call, shared by all its panels and layout passes.
/// The scene's fonts are `fonts`, so the faces of its text items index them.
///
/// The value labels of scatter points can make room for themselves: each pass lays the figure
/// out with the sides its automatic labels took in the pass before, none at first, and draws
/// it, and the passes end once the labels settle ([`point_labels::fit_then_place`]). Each pass
/// measures the panels again, so that nothing but those sides carries over. A value column is
/// judged against the settled layout: its errors come from the last pass only, since an earlier
/// one may lay the plot out narrower.
fn scene_with_labels(
    figure: &Figure,
    fonts: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
) -> Result<Scene> {
    let groups = ShareGroups::new(figure);
    check_contour_levels(figure, &groups, context)?;
    check_value_columns(figure, context)?;
    check_equal_aspect(figure, context)?;
    let reference = reference_rooms(figure, &groups, fonts, context, labels)?;
    let rooms = Rooms::of(reference.as_ref());
    point_labels::fit_then_place(|held| {
        let mut panels = panels(figure, &groups, fonts)?;
        if let Some(held) = held {
            for (panel, sides) in panels.iter_mut().zip(held) {
                panel.points.hold(sides.clone());
            }
        }
        draw_scene(figure, &mut panels, &groups, fonts, context, labels, rooms)
    })?
}

/// The scene of `figure` laid out and drawn once, with the records `panels` of its panels, as
/// [`scene`] describes, and where the automatic value labels of each panel went, in the order
/// of the panels. The scene is an error instead when a highlighted category's label is left
/// out ([`check_tick_highlights`]), or else the first error of a value column, when one has
/// values that cannot be written ([`point_labels::column_items`]): the pass still places every
/// automatic label, so that the caller raises that error only from a settled pass.
///
/// # Errors
///
/// As [`scene`], but for the errors judged against the settled layout; and as
/// [`check_tick_highlights`] for a highlighted category that is not a category of its axis.
fn draw_scene<'a>(
    figure: &'a Figure,
    panels: &mut [Panel<'a>],
    groups: &ShareGroups,
    fonts: &SceneFonts<'_>,
    context: &'static str,
    labels: &layout::LabelCache<'_>,
    rooms: Rooms<'_>,
) -> Result<(Result<Scene>, Vec<point_labels::Placement>)> {
    let theme = figure.theme();
    let (width, height) = (figure.width_in() * 72.0, figure.height_in() * 72.0);
    let mut scene = Scene::new(width as f32, height as f32, theme.background());
    scene.fonts = fonts.fonts().to_vec();
    if panels.is_empty() {
        let header = figure_header(figure, panels, fonts)?;
        header.check(figure, context)?;
        if let Some(header) = draw::figure_header(theme, &header) {
            scene.push(header);
        }
        return Ok((Ok(scene), Vec::new()));
    }
    let planned = plan_panels(figure, panels, groups, fonts, context, labels, rooms)?;
    // Judged against the settled layout only, as the errors of value columns: an earlier pass
    // may lay the plot out smaller and thin out a highlighted label that the last one shows.
    let mut settled_error = check_tick_highlights(figure, panels, &planned, context)?;
    let mut layers = draw_layers(figure, panels, &planned, fonts, context, rooms)?;
    let mut placements: Vec<point_labels::Placement> = std::iter::repeat_with(Default::default)
        .take(panels.len())
        .collect();
    let mut points = place_point_labels(figure, panels, &planned, &layers, context, rooms)?;
    let mut annotations = Vec::new();
    let mut paired_legends = vec![None; panels.len()];
    for (i, (panel, (resolved, layout))) in panels.iter().zip(&planned.panels).enumerate() {
        let axes = match panel.axes {
            Layer::Primary(crate::Panel::Cartesian { axes, .. }) | Layer::Twin { axes, .. } => axes,
            Layer::Primary(crate::Panel::Pie(pie_panel)) => {
                scene.push(draw::chrome(theme, layout));
                if let Some(pie) = &pie_panel.pie {
                    let placed = layout::pie::place(
                        pie,
                        pie_panel.legend,
                        layout::pie::PieRoom::new(layout.plot, layout.aligned),
                        theme,
                        fonts,
                    )
                    .map_err(|err| in_cell(figure, panel.axes, err))?;
                    scene.push(draw::pie::draw(theme, &placed));
                }
                if let Some(legend) = draw::legend::legend(theme, layout) {
                    scene.push(legend);
                }
                continue;
            }
        };
        let Some(LayerMarks {
            drawn,
            notes,
            values,
        }) = layers[i].take()
        else {
            continue;
        };
        let points = points[i].take().unwrap_or_default();
        placements[i] = points
            .placement
            .map_error(|err| in_cell(figure, panel.axes, err));
        let column = match point_labels::column_items(
            theme,
            resolved,
            layout,
            &panel.marks,
            &panel.points,
            (
                context,
                figure.grid().rows() * figure.grid().cols() > 1,
                width,
            ),
        ) {
            Ok(column) => column,
            Err(_) if judged_later(rooms, i) => None,
            Err(err) => {
                // Judged against the settled layout only: see `scene_with_labels`.
                settled_error.get_or_insert_with(|| in_cell(figure, panel.axes, err));
                None
            }
        };
        scene.push(draw::chrome(theme, layout));
        let stacked = [
            drawn.into_item(),
            notes,
            values.map(|values| values.item),
            points.item,
            column,
        ];
        for item in stacked.into_iter().flatten() {
            scene.push(item);
        }
        let legend = draw::legend::legend(theme, layout);
        match panel.axes {
            Layer::Primary(crate::Panel::Cartesian { twin: Some(_), .. }) => {
                paired_legends[i] = legend;
            }
            Layer::Primary(crate::Panel::Cartesian { twin: None, .. } | crate::Panel::Pie(_)) => {
                if let Some(legend) = legend {
                    scene.push(legend);
                }
            }
            Layer::Twin { parent, .. } => {
                if let Some(legend) = paired_legends[parent].take() {
                    scene.push(legend);
                }
                if let Some(legend) = legend {
                    scene.push(legend);
                }
            }
        }
        match panel.axes {
            Layer::Primary(_) => {
                if resolved.draws_baseline(axes.marks(), axes.baseline_visibility()) {
                    scene.push(draw::baseline(theme, layout));
                }
            }
            Layer::Twin { .. } => {}
        }
        if let (Some(color), Some(bar)) = (&resolved.color, &layout.colorbar) {
            scene.push(draw::colorbar::draw(theme, color, bar));
        }
        let notes = draw::annotations(
            theme,
            fonts,
            resolved,
            layout,
            axes.annotations(),
            draw::AnnotationCanvas {
                width,
                height,
                caption_top: planned.header.caption.top(),
            },
            context,
        );
        match notes {
            Ok(notes) => annotations.extend(notes),
            Err(_) if judged_later(rooms, i) => {}
            Err(err) => return Err(in_cell(figure, panel.axes, err)),
        }
    }
    for (source, bar) in &planned.solution.colorbars {
        if let Some(color) = &planned.panels[*source].0.color {
            scene.push(draw::colorbar::draw(theme, color, bar));
        }
    }
    if let Some(header) = draw::figure_header(theme, &planned.header) {
        scene.push(header);
    }
    // Text at data positions may reach past its cell: it goes over everything else.
    for notes in annotations {
        scene.push(notes);
    }
    Ok((settled_error.map_or(Ok(scene), Err), placements))
}

/// What one Cartesian layer of a panel draws under its point labels: its marks, the values
/// written on its heatmap cells and the values written on its bars.
struct LayerMarks {
    /// The marks, with the items of each mark kept apart.
    drawn: draw::DrawnMarks,
    /// The values written on annotated heatmap cells.
    notes: Option<crate::scene::Item>,
    /// The values written on annotated bars, with their boxes.
    values: Option<bar_labels::PlacedLabels>,
}

/// The marks and the cell and bar labels of every Cartesian layer of `panels`, laid out as
/// `planned`, in the order of the panels; `None` for a pie. They are drawn before any point
/// label is placed, so that the point labels of a twin-axes panel avoid those of every layer.
///
/// # Errors
///
/// As [`draw::check_images`], [`draw::check_points`], [`draw::drawn_marks`],
/// [`draw::heatmap_annotations`] and [`bar_labels::items`], for the first layer that fails,
/// named by its cell. A layer that `rooms` leaves to be judged later is `None` instead
/// ([`judged_later`]).
fn draw_layers(
    figure: &Figure,
    panels: &[Panel<'_>],
    planned: &FigurePlan,
    fonts: &SceneFonts<'_>,
    context: &'static str,
    rooms: Rooms<'_>,
) -> Result<Vec<Option<LayerMarks>>> {
    let theme = figure.theme();
    let layer = |panel: &Panel<'_>, (resolved, layout): &(Resolved, Layout)| {
        let Some(axes) = panel.axes.cartesian() else {
            return Ok(None);
        };
        let cell = |err| in_cell(figure, panel.axes, err);
        draw::check_images(resolved, layout, &panel.marks, context).map_err(cell)?;
        draw::check_points(resolved, layout, &panel.marks, context).map_err(cell)?;
        let drawn =
            draw::drawn_marks(theme, fonts, resolved, layout, &panel.marks).map_err(cell)?;
        let notes = draw::heatmap_annotations(
            theme,
            fonts,
            resolved,
            layout,
            &panel.marks,
            (axes.y_tick_format(), context),
        )
        .map_err(cell)?;
        let values = bar_labels::items(
            theme,
            resolved,
            layout,
            axes,
            &panel.marks,
            &panel.labels,
            context,
        )
        .map_err(cell)?;
        Ok(Some(LayerMarks {
            drawn,
            notes,
            values,
        }))
    };
    panels
        .iter()
        .zip(&planned.panels)
        .enumerate()
        .map(|(i, (panel, plan))| match layer(panel, plan) {
            Err(_) if judged_later(rooms, i) => Ok(None),
            outcome => outcome,
        })
        .collect()
}

/// The point labels of every Cartesian layer of `panels`, laid out as `planned` and drawn as
/// `layers`, in the order of the panels; `None` for a pie. The layers of one plot, a panel and
/// its twin, share the obstacles of their labels: the marks and bar labels of every layer, then
/// each label placed. They place their labels in the order of the panels, the left axes first.
///
/// # Errors
///
/// As [`point_labels::items`], for the first label that cannot be placed, named by its cell.
/// Labels that `rooms` leaves to be judged later are `None` instead ([`judged_later`]).
fn place_point_labels(
    figure: &Figure,
    panels: &[Panel<'_>],
    planned: &FigurePlan,
    layers: &[Option<LayerMarks>],
    context: &'static str,
    rooms: Rooms<'_>,
) -> Result<Vec<Option<point_labels::Placed>>> {
    let theme = figure.theme();
    // The plot each layer is drawn in: its own for a primary axes, its parent's for a twin.
    let plot_of = |i: usize| match panels[i].axes {
        Layer::Twin { parent, .. } => parent,
        Layer::Primary(_) => i,
    };
    let mut placed: Vec<Option<point_labels::Placed>> =
        std::iter::repeat_with(|| None).take(panels.len()).collect();
    for plot in 0..panels.len() {
        let members: Vec<usize> = (0..panels.len())
            .filter(|&i| plot_of(i) == plot && layers[i].is_some())
            .collect();
        if members.iter().all(|&i| panels[i].points.is_empty()) {
            continue;
        }
        let mut obstacles = point_labels::obstacles(planned.panels[plot].1.plot);
        for &i in &members {
            if let Some(layer) = &layers[i] {
                let boxes = layer.values.as_ref().map_or(&[][..], |v| &v.boxes[..]);
                point_labels::add_obstacles(
                    &mut obstacles,
                    &panels[i].marks,
                    (&layer.drawn, boxes),
                );
            }
        }
        for &i in &members {
            let (panel, (resolved, layout)) = (&panels[i], &planned.panels[i]);
            let labels = point_labels::items(
                theme,
                resolved,
                layout,
                &panel.marks,
                &panel.points,
                &mut obstacles,
                context,
            )
            .map_err(|err| in_cell(figure, panel.axes, err));
            match labels {
                Ok(labels) => placed[i] = Some(labels),
                Err(_) if judged_later(rooms, i) => {}
                Err(err) => return Err(err),
            }
        }
    }
    Ok(placed)
}

/// Checks the highlighted categories of every axes of `figure`, whose records are `panels`,
/// laid out as `planned` ([`AxesMut::xtick_highlight`](crate::AxesMut::xtick_highlight)).
/// A twin's x axis is its parent's, and its y axis is numeric, so only primary axes are
/// checked. Errors are named by the axes' cell, x before y, and in order of the categories.
///
/// Returns `Error::Style` under `context` for the first highlighted category whose label the
/// automatic labels of a crowded axis leave out (they show every k-th category), since its
/// highlight would not show; `None` when every highlighted label is drawn.
///
/// # Errors
///
/// `Error::Data` under `context` for the first highlighted name that is not a category of its
/// axis, before any label is judged.
fn check_tick_highlights(
    figure: &Figure,
    panels: &[Panel<'_>],
    planned: &FigurePlan,
    context: &'static str,
) -> Result<Option<Error>> {
    let axes: Vec<_> = panels
        .iter()
        .zip(&planned.panels)
        .filter_map(|(panel, (resolved, _))| match panel.axes {
            Layer::Primary(crate::Panel::Cartesian { axes, .. }) => Some((panel, axes, resolved)),
            Layer::Primary(crate::Panel::Pie(_)) | Layer::Twin { .. } => None,
        })
        .flat_map(|(panel, axes, resolved)| {
            [
                (panel, "x", axes.x_tick_highlight(), &resolved.x),
                (panel, "y", axes.y_tick_highlight(), &resolved.y),
            ]
        })
        .filter_map(|(panel, side, highlight, axis)| match &axis.scale {
            resolve::AxisScale::Category(scale) if !highlight.is_empty() => {
                Some((panel, side, highlight, axis, scale))
            }
            _ => None,
        })
        .collect();
    for &(panel, side, highlight, _, scale) in &axes {
        if let Some(name) = highlight
            .categories()
            .iter()
            .find(|name| scale.index_of(name).is_none())
        {
            let err = Error::data(
                context,
                format!(
                    "{side}tick_highlight() names {}, which is not a category of the {side} axis.",
                    crate::error::quoted(name)
                ),
            )
            .with_hint("pass names of the axis' categories, exactly as the marks give them.");
            return Err(in_cell(figure, panel.axes, err));
        }
    }
    for &(panel, side, highlight, axis, scale) in &axes {
        let shown = |name: &String| axis.ticks.iter().any(|tick| tick.label == *name);
        let Some(name) = highlight.categories().iter().find(|name| !shown(name)) else {
            continue;
        };
        let index = |tick: &resolve::AxisTick| scale.index_of(&tick.label).unwrap_or(0);
        let labelled = match axis.ticks.as_slice() {
            [first, second, ..] => format!(
                "every {} category",
                ordinal(index(second).saturating_sub(index(first)))
            ),
            _ => "only its first category".to_owned(),
        };
        let err = Error::style(
            context,
            format!(
                "the {side} axis labels {labelled} to keep its labels apart, which leaves out \
                 the highlighted {}.",
                crate::error::quoted(name)
            ),
        )
        .with_hint(match side {
            "x" => "enlarge the figure, or set a smaller tick label size with xtick_labels().",
            _ => "enlarge the figure, or set a smaller tick label size with ytick_labels().",
        });
        return Ok(Some(in_cell(figure, panel.axes, err)));
    }
    Ok(None)
}

/// `n` as an English ordinal: `1st`, `2nd`, `3rd`, `4th`, `11th`, `22nd`.
fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, hundredth) if hundredth != 11 => "st",
        (2, hundredth) if hundredth != 12 => "nd",
        (3, hundredth) if hundredth != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// `err`, an error of the axes `axes` of `figure`, naming the axes' top-left cell first on a
/// grid of more than one cell, as layout errors do.
fn in_cell<'a>(figure: &Figure, panel: impl Into<Layer<'a>>, err: Error) -> Error {
    let grid = figure.grid();
    if grid.rows() * grid.cols() == 1 {
        return err;
    }
    let cells = panel.into().cells();
    err.with_message_prefix(&format!("row {}, col {}: ", cells.row(), cells.col()))
}

#[cfg(test)]
#[path = "equal_aspect_tests.rs"]
mod equal_aspect_tests;

#[cfg(test)]
#[path = "label_cache_tests.rs"]
mod label_cache_tests;

#[cfg(test)]
#[path = "category_label_tests.rs"]
mod category_label_tests;

#[cfg(test)]
#[path = "header_tests.rs"]
mod header_tests;

#[cfg(test)]
#[path = "font_error_tests.rs"]
mod font_error_tests;

#[cfg(test)]
#[path = "line_break_tests.rs"]
mod line_break_tests;

#[cfg(test)]
#[path = "pie_grid_tests.rs"]
mod pie_grid_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Grid, GridIndex, LineOptions, Placement};
    use crate::scene::Item;
    use crate::{LegendStyle, ScaleKind};

    #[test]
    fn group_plot_uses_the_shortest_member_with_visible_labels_on_each_side() {
        let groups = ShareGroups {
            x: vec![0, 0, 0, 3],
            y: vec![0, 0, 0, 3],
            color: vec![None; 4],
        };
        let plots = [
            PlotSize {
                width: 100.0,
                height: 50.0,
            },
            PlotSize {
                width: 400.0,
                height: 200.0,
            },
            PlotSize {
                width: 300.0,
                height: 150.0,
            },
            PlotSize {
                width: 10.0,
                height: 10.0,
            },
        ];
        let show_labels = [[false, true], [true, false], [true, true], [true, true]];
        for i in 0..3 {
            assert_eq!(
                Side::X.group_plot(&groups, i, &plots, &show_labels),
                plots[2]
            );
            assert_eq!(
                Side::Y.group_plot(&groups, i, &plots, &show_labels),
                plots[0]
            );
        }
    }

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
    /// chrome and line marks, plus one with a legend.
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
    fn header_theme_shows_the_legend_after_marks() {
        let f = labelled_figure(Theme::pigment());
        assert_eq!(legend_and_groups(&f), (2, 3));
        let s = scene(&f, FontLibrary::global(), "Figure.to_scene()").unwrap();
        // Chrome, marks, legend (unclipped).
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
        assert_eq!(legend_and_groups(&labelled_figure(theme)), (0, 2));
    }

    #[test]
    fn axes_none_hides_the_legend_under_a_header_theme() {
        let mut f = labelled_figure(Theme::pigment());
        f.single_axes_mut()
            .legend(LegendStyle::None, crate::SizeSpec::Auto)
            .unwrap();
        assert_eq!(legend_and_groups(&f), (0, 2));
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
        f.single_axes_mut()
            .legend(LegendStyle::Header, crate::SizeSpec::Auto)
            .unwrap();
        assert_eq!(legend_and_groups(&f), (2, 3));
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
        assert_eq!(legend_and_groups(&f), (0, 3));
        assert_eq!(direct(&f), 2);
        // The axes ask for the header row.
        f.single_axes_mut()
            .legend(LegendStyle::Header, crate::SizeSpec::Auto)
            .unwrap();
        assert_eq!(legend_and_groups(&f), (2, 3));
        assert_eq!(direct(&f), 0);
        // Calling it again replaces the style.
        f.single_axes_mut()
            .legend(LegendStyle::None, crate::SizeSpec::Auto)
            .unwrap();
        assert_eq!(legend_and_groups(&f), (0, 2));
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

    fn color_grid(share: crate::Share, scale: crate::Transform, center: Option<f64>) -> Figure {
        let grid = Grid::new(2, 2).unwrap().with_share_color(share);
        let mut figure = Figure::with_grid(12.0, 8.0, Theme::pigment(), grid).unwrap();
        for row in (0..2).rev() {
            for col in (0..2).rev() {
                let low = 1.0 + (row * 10 + col * 3) as f64;
                figure
                    .add_axes_at(
                        Placement {
                            row: GridIndex::At(row),
                            col: GridIndex::At(col),
                            ..Placement::default()
                        },
                        ScaleKind::Linear,
                        ScaleKind::Linear,
                    )
                    .unwrap()
                    .scatter(
                        AxisData::Numbers(vec![0.0, 1.0]),
                        &[0.0, 1.0],
                        crate::ScatterOptions {
                            color: crate::PointColor::By(crate::ColorBy {
                                scale,
                                center,
                                ..crate::ColorBy::new(vec![low, low + 2.0])
                            }),
                            ..crate::ScatterOptions::default()
                        },
                    )
                    .unwrap();
            }
        }
        figure
    }

    #[test]
    fn shared_color_norms_use_only_each_groups_union() {
        use crate::color_scale::Norm;
        use crate::{LogBase, Share, Transform};

        for share in [Share::None, Share::Row, Share::Col, Share::All] {
            for (scale, center) in [
                (Transform::Linear, None),
                (Transform::Linear, Some(0.0)),
                (Transform::Log(LogBase::TEN), None),
            ] {
                let figure = color_grid(share, scale, center);
                let fonts = crate::test_support::figure_fonts(figure.theme());
                let panels = panels(&figure, &ShareGroups::new(&figure), &fonts).unwrap();
                for (i, panel) in panels.iter().enumerate() {
                    let (row, col) = ((i / 2) as f64, (i % 2) as f64);
                    let (low, high) = match share {
                        Share::None => (1.0 + row * 10.0 + col * 3.0, 3.0 + row * 10.0 + col * 3.0),
                        Share::Row => (1.0 + row * 10.0, 6.0 + row * 10.0),
                        Share::Col => (1.0 + col * 3.0, 13.0 + col * 3.0),
                        Share::All => (1.0, 16.0),
                    };
                    let expected = match (scale, center) {
                        (_, Some(center)) => Norm::Centered {
                            center,
                            radius: high,
                        },
                        (Transform::Log(base), _) => Norm::Log { low, high, base },
                        (Transform::Linear, _) => Norm::Linear { low, high },
                    };
                    assert_eq!(panel.color.as_ref().unwrap().norm, expected);
                }
            }
        }
    }

    #[test]
    fn shared_hexbin_colors_use_the_union_after_counting_and_recounting() {
        use crate::color_scale::Norm;
        use crate::{HexbinOptions, Paint, Share};

        for fixed_y in [false, true] {
            let grid = Grid::new(1, 2).unwrap().with_share_color(Share::All);
            let mut figure = Figure::with_grid(12.0, 4.0, Theme::pigment(), grid).unwrap();
            for col in 0..2 {
                let mut x = Vec::new();
                let mut y = Vec::new();
                for (value, count) in [(0.0, 1), (1.0, 4), (2.0, 9)] {
                    let count = count * if col == 0 { 1 } else { 3 };
                    x.extend(std::iter::repeat_n(value, count));
                    y.extend(std::iter::repeat_n(value, count));
                }
                let mut axes = figure
                    .add_axes_at(
                        Placement {
                            col: GridIndex::At(col),
                            ..Placement::default()
                        },
                        ScaleKind::Linear,
                        ScaleKind::Linear,
                    )
                    .unwrap();
                if fixed_y && col == 0 {
                    axes.ylim(-1.0, 3.0).unwrap();
                }
                axes.hexbin(&x, &y, HexbinOptions::default()).unwrap();
            }
            let lib = FontLibrary::global();
            let fonts = crate::test_support::figure_fonts(figure.theme());
            let groups = ShareGroups::new(&figure);
            let mut panels = panels(&figure, &groups, &fonts).unwrap();
            crate::bin2d::COUNT_CALLS.with(|calls| calls.set(0));
            let planned = plan_cells(
                &figure,
                &mut panels,
                &groups,
                &fonts,
                "Figure.to_scene()",
                &layout::LabelCache::new(&fonts),
            )
            .unwrap();
            let calls = crate::bin2d::COUNT_CALLS.with(|calls| calls.get());
            if fixed_y {
                assert!(
                    calls > 2,
                    "the fixed y window must cause a recount: {calls}"
                );
            } else {
                assert_eq!(calls, 2);
            }
            let counts: Vec<_> = panels
                .iter()
                .flat_map(|panel| &panel.marks)
                .filter_map(ResolvedMark::color_values)
                .flat_map(|(_, values)| values.iter().copied())
                .collect();
            let low = counts.iter().copied().fold(f64::INFINITY, f64::min);
            let high = counts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert_eq!((low, high), (1.0, 27.0));
            let expected = Norm::Linear { low, high };
            for (panel, (resolved, layout)) in panels.iter().zip(&planned.panels) {
                assert_eq!(panel.color.as_ref().unwrap().norm, expected);
                assert_eq!(resolved.color.as_ref().unwrap().norm, expected);
                assert!(layout.colorbar.is_none());
            }
            assert_eq!(planned.solution.colorbars.len(), 1);
            let source = planned.solution.colorbars[0].0;
            assert_eq!(
                planned.panels[source].0.color.as_ref().unwrap().norm,
                expected
            );

            let scene = figure.to_scene(lib).unwrap();
            let bars = scene
                .items
                .iter()
                .filter(|item| match item {
                    Item::Group { clip: None, items } => items.iter().any(|item| {
                        matches!(
                            item,
                            Item::Fill {
                                paint: Paint::Sampled { .. },
                                ..
                            }
                        )
                    }),
                    _ => false,
                })
                .count();
            assert_eq!(bars, 1);
            figure
                .axes_at_mut(GridIndex::At(0), GridIndex::At(0))
                .unwrap()
                .clim(low, high)
                .unwrap();
            assert_eq!(scene, figure.to_scene(lib).unwrap());
        }
    }

    #[test]
    fn shared_colorbars_preserve_plot_alignment() {
        use crate::{Share, Transform};

        for share in [Share::Row, Share::Col, Share::All] {
            let figure = color_grid(share, Transform::Linear, None);
            let fonts = crate::test_support::figure_fonts(figure.theme());
            let groups = ShareGroups::new(&figure);
            let panels = panels(&figure, &groups, &fonts).unwrap();
            let Ok(planned) = plan(
                &figure,
                &panels,
                &groups,
                &fonts,
                "Figure.to_scene()",
                &layout::LabelCache::new(&fonts),
                crate::pipeline::Rooms::Solved,
            ) else {
                panic!("the figure lays out")
            };
            assert_eq!(
                planned.solution.colorbars.len(),
                if share == Share::All { 1 } else { 2 }
            );
            let plots: Vec<_> = planned
                .panels
                .iter()
                .map(|(_, layout)| layout.plot)
                .collect();
            for plot in &plots[1..] {
                assert!((plot.width - plots[0].width).abs() < 1e-9);
                assert!((plot.height - plots[0].height).abs() < 1e-9);
            }
            assert_eq!(plots[0].x, plots[2].x);
            assert_eq!(plots[1].x, plots[3].x);
            assert_eq!(plots[0].y, plots[1].y);
            assert_eq!(plots[2].y, plots[3].y);
        }
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
            let title = l.title.as_ref().expect("a title").first();
            assert_eq!(title.x, l.plot.x);
            assert!(title.y < l.plot.y);
        }
        assert_eq!(a.header_bottom, b.header_bottom);
        // Neighbouring frames touch halfway between the cells.
        assert_eq!(a.frame.right(), b.frame.x);
        assert_eq!(a.frame.bottom(), c.frame.y);
        assert_eq!((a.frame.x, a.frame.y), (0.0, 0.0));
        // Chrome and line marks for each axes, in row-major order; nothing for the empty cell.
        let s = scene(&f, lib, "Figure.to_scene()").unwrap();
        let groups = s
            .items
            .iter()
            .filter(|item| matches!(item, Item::Group { .. }))
            .count();
        assert_eq!((s.items.len(), groups), (6, 6));
        let chrome_titles: Vec<bool> = s
            .items
            .iter()
            .step_by(2)
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

    /// A random walk of 97 steps, each up to half a unit either way, from a linear
    /// congruential generator seeded with `seed`, over x from 0 to 96.
    fn walk(seed: u32) -> (Vec<f64>, Vec<f64>) {
        let mut state = seed;
        let mut level = 0.0;
        let y = (0..97)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
                level += f64::from((state >> 8) & 0xffff) / 65536.0 - 0.5;
                level
            })
            .collect();
        ((0..97).map(f64::from).collect(), y)
    }

    /// Twenty titled walks on a 4 × 5 grid at 12 × 8 in, "Sensor 1" to "Sensor 20", seeded 50
    /// to 69.
    fn sensors() -> Figure {
        let mut f =
            Figure::with_grid(12.0, 8.0, Theme::pigment(), Grid::new(4, 5).unwrap()).unwrap();
        for i in 0..20_u32 {
            let at = Placement {
                row: GridIndex::At(i64::from(i / 5)),
                col: GridIndex::At(i64::from(i % 5)),
                ..Placement::default()
            };
            let (x, y) = walk(50 + i);
            f.add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .title(&format!("Sensor {}", i + 1))
                .line(AxisData::Numbers(x), &y, LineOptions::default())
                .unwrap();
        }
        f
    }

    /// The panels of `figure` resolved for a plot of `size` and the tick spacing `spacing`, with
    /// their tick rooms.
    fn resolved_at<'a>(
        figure: &Figure,
        size: PlotSize,
        spacing: TickSpacing,
        lib: &'a SceneFonts<'a>,
    ) -> Vec<(Resolved, layout::TickRoom<'a>, layout::TickRoom<'a>)> {
        panels(figure, &ShareGroups::new(figure), lib)
            .unwrap()
            .iter()
            .map(|panel| {
                let (x, y) = layout::tick_rooms(
                    size,
                    crate::test_support::tick_font(figure.theme()),
                    spacing,
                );
                let resolved = resolve::resolve(&panel.marks, &Limits::of(panel.axes), &x, &y);
                (resolved, x, y)
            })
            .collect()
    }

    #[test]
    fn crowded_y_ticks_keep_their_labels_apart() {
        // Data from −0.01 to 5.69 on a short plot: data-relative limits let the target
        // choose three round ticks whose labels fit the 60 pt height.
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let (x, y) = walk(54);
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(AxisData::Numbers(x), &y, LineOptions::default())
            .unwrap();
        let fonts = crate::test_support::figure_fonts(f.theme());
        let size = PlotSize {
            width: 126.0,
            height: 60.0,
        };
        let (old, _, _) = resolved_at(&f, size, TickSpacing::Target, &fonts).remove(0);
        assert_eq!(tick_labels(&old.y.ticks), ["0.0", "2.5", "5.0"]);
        let (new, _, room) = resolved_at(&f, size, TickSpacing::LabelsApart, &fonts).remove(0);
        assert!(room.slack(&old.y.ticks) >= 0.0);
        assert!(room.slack(&new.y.ticks) >= 0.0);
        // The label-constrained pass keeps an already fitting grid.
        assert_eq!(new.y.ticks, old.y.ticks);
        // The x ticks were apart already, and stay as they were.
        assert_eq!(new.x.ticks, old.x.ticks);
    }

    #[test]
    fn crowded_grid_panels_get_round_ticks() {
        let f = sensors();
        let lib = FontLibrary::global();
        let planned = try_plan_panels(&f, lib).unwrap();
        for (resolved, layout) in &planned {
            let (x, y) = layout::tick_rooms(
                layout.plot.size(),
                crate::test_support::tick_font(f.theme()),
                TickSpacing::LabelsApart,
            );
            assert!(y.slack(&resolved.y.ticks) >= 0.0, "{:?}", layout.plot);
            assert!(x.slack(&resolved.x.ticks) >= 0.0, "{:?}", layout.plot);
            // The drawn labels: baselines a line and the gap apart.
            let baselines: Vec<f64> = layout.y_tick_labels.iter().map(|l| l.y).collect();
            assert!(baselines.windows(2).all(|w| (w[0] - w[1]).abs() >= 15.0));
        }
        // The panels whose y labels crowded for the target alone get round steps, with 0 a
        // tick: before, 6, 8, 6 and 8 ticks one step apart.
        let y_labels = |sensor: usize| tick_labels(&planned[sensor - 1].0.y.ticks);
        let m = |n: &str| format!("\u{2212}{n}");
        assert_eq!(y_labels(2), [&m("4"), &m("2"), "0"]);
        assert_eq!(y_labels(5), ["0.0", "2.5", "5.0"]);
        assert_eq!(y_labels(15), ["0.0", "2.5"]);
        assert_eq!(y_labels(19), [&m("5.0"), &m("2.5"), "0.0"]);
    }

    /// The labels of `side` of the first panel of `figure` as laid out, and the room they
    /// leave on the plot drawn.
    fn final_ticks(figure: &Figure, side: Side) -> (Vec<String>, f64) {
        let lib = FontLibrary::global();
        let (resolved, layout) = try_plan_panels(figure, lib).unwrap().remove(0);
        let ticks = side.ticks(&resolved);
        let slack = side
            .room(
                layout.plot.size(),
                crate::test_support::tick_font(figure.theme()),
            )
            .slack(ticks);
        let labels = ticks.iter().map(|tick| tick.label.clone()).collect();
        (labels, slack)
    }

    /// The previous two-pass layout: target-only ticks on guessed sizes, then the constrained
    /// search (including target retries) on the first solve's sizes, with no repair pass.
    fn previous_plan(figure: &Figure, library: &FontLibrary) -> Result<Vec<(Resolved, Layout)>> {
        let fonts = layout::figure_fonts(library, figure.theme())?;
        let lib = &fonts;
        let groups = ShareGroups::new(figure);
        let panels = panels(figure, &groups, lib).unwrap();
        let theme = figure.theme();
        let inputs: Vec<_> = panels
            .iter()
            .map(|panel| LayoutInput {
                theme,
                figure_title_size: figure.title_size(),
                axes: panel.axes,
                legend: &panel.legend,
                column: panel.points.column(),
                lib,
                context: "Figure.to_scene()",
            })
            .collect();
        let grid = GridInput {
            width: figure.width_in() * 72.0,
            height: figure.height_in() * 72.0,
            caption: 0.0,
            top: figure_header(figure, &panels, lib).unwrap().bottom,
            grid: figure.grid(),
            theme,
            context: "Figure.to_scene()",
            panels: &inputs,
            colorbars: &[],
        };
        let pass = |sizes: Vec<PlotSize>, spacing| {
            let resolved: Vec<_> = panels
                .iter()
                .zip(sizes)
                .map(|(panel, size)| {
                    let (x, y) =
                        layout::tick_rooms(size, crate::test_support::tick_font(theme), spacing);
                    resolve::resolve(&panel.marks, &Limits::of(panel.axes), &x, &y)
                })
                .collect();
            let bands: Vec<_> = inputs
                .iter()
                .zip(&resolved)
                .map(|(input, resolved)| {
                    layout::measure_panel(
                        input,
                        resolved,
                        grid.span(input.axes.cells()),
                        [true; 2],
                        &layout::LabelCache::new(lib),
                    )
                    .unwrap()
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
        let (resolved, bands) = pass(sizes, TickSpacing::Deferred);
        let solution = layout::solve_grid(&grid, &bands)?;
        let layouts: Vec<_> = inputs
            .iter()
            .zip(&resolved)
            .zip(bands)
            .zip(&solution.panels)
            .map(|(((input, resolved), bands), slot)| {
                layout::place_panel(input, resolved, bands, slot)
            })
            .collect();
        layout::check_panel_contents(&grid, &layouts, &solution.panels)?;
        Ok(resolved.into_iter().zip(layouts).collect())
    }

    #[test]
    fn fitting_constrained_ticks_keep_their_final_layout() {
        let theme = Theme::pigment().with_font_scale(1.4).unwrap();
        let mut f = Figure::new(3.2, 5.35, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Numbers(vec![9000.0, 46500.0, 84000.0]),
                &[-7781.0, -7780.5, -7780.0],
                LineOptions::default(),
            )
            .unwrap()
            .xtick_format(crate::TickFormat::percent(1).unwrap())
            .unwrap();
        let lib = FontLibrary::global();
        let (previous, previous_layout) = previous_plan(&f, lib).unwrap().remove(0);
        let (resolved, layout) = try_plan_panels(&f, lib).unwrap().remove(0);
        assert_eq!(resolved, previous);
        assert_eq!(format!("{layout:?}"), format!("{previous_layout:?}"));
        let slack = Side::X
            .room(
                layout.plot.size(),
                crate::test_support::tick_font(f.theme()),
            )
            .slack(&resolved.x.ticks);
        // No whole-step pair fits this width; one grid label keeps the solved layout and
        // never crowds another label. A fitting pair must not be replaced by this fallback.
        assert_eq!(tick_labels(&resolved.x.ticks), ["5000000.0%"]);
        assert_eq!(slack, f64::INFINITY);
    }

    #[test]
    fn fitting_whole_step_pairs_are_not_reduced_to_one_tick() {
        let theme = Theme::pigment().with_font_scale(1.4).unwrap();
        let mut f = Figure::new(3.5, 5.35, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 37500.0, 75000.0]),
                &[-7781.0, -7780.5, -7780.0],
                LineOptions::default(),
            )
            .unwrap()
            .xtick_format(crate::TickFormat::percent(1).unwrap())
            .unwrap();
        let (labels, slack) = final_ticks(&f, Side::X);
        assert_eq!(labels, ["0.0%", "5000000.0%"]);
        assert!(slack >= 0.0, "{slack}");
    }

    #[test]
    fn x_labels_keep_apart_on_the_final_plot() {
        // 72 × 65 pt: the ticks are chosen for a 35.25 pt wide plot, where 0, 50, 100 fit;
        // the y tick labels chosen with them, 0 and 10, take 5.1 pt more than the first pass's,
        // so the plot drawn is 30.15 pt wide and those x labels would crowd by 1.9 pt.
        let mut f = Figure::new(1.0, 65.0 / 72.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 100.0]),
                &[0.0, 5.7],
                LineOptions::default(),
            )
            .unwrap();
        let (labels, slack) = final_ticks(&f, Side::X);
        assert_eq!(labels, ["0", "100"]);
        assert!(slack >= 0.0, "{slack}");
    }

    fn short_percent_scatter(x: [f64; 2], y: [f64; 2]) -> Figure {
        let theme = Theme::pigment().with_font_scale(1.4).unwrap();
        let mut f = Figure::new(2.5, 1.8, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                AxisData::Numbers(vec![x[0], (x[0] + x[1]) / 2.0, x[1]]),
                &[y[0], (y[0] + y[1]) / 2.0, y[1]],
                crate::ScatterOptions::default(),
            )
            .unwrap()
            .xtick_format(crate::TickFormat::percent(1).unwrap())
            .unwrap();
        f
    }

    #[test]
    fn percent_pair_is_rechecked_on_the_final_plot() {
        let f = short_percent_scatter(
            [0.07579250753325248, 0.14422238351939026],
            [0.08045950529471643, 0.08810734423115522],
        );
        let (labels, slack) = final_ticks(&f, Side::X);
        assert!(slack >= 0.0, "{labels:?}: {slack}");
    }

    #[test]
    fn long_percent_labels_retry_before_rejecting_the_figure() {
        let f = short_percent_scatter(
            [-4680165.074054506, -4023576.5694846567],
            [-3168066.8464300144, -2327272.4889245224],
        );
        let (resolved, layout) = try_plan_figure(&f, (0, 0), FontLibrary::global()).unwrap();
        assert!(
            Side::X
                .room(
                    layout.plot.size(),
                    crate::test_support::tick_font(f.theme())
                )
                .slack(&resolved.x.ticks)
                >= 0.0
        );
        assert_eq!(resolved.x.ticks.len(), 1);
        assert_eq!(tick_labels(&resolved.x.ticks), ["−450000000.0%"]);
        assert!(resolved.x.continuous().unwrap().domain().1 < 0.0);
        f.to_scene(FontLibrary::global()).unwrap();

        let mut explicit = f.clone();
        let (low, high) = resolved.x.continuous().unwrap().domain();
        explicit
            .single_axes_mut()
            .xlim(low, high)
            .unwrap()
            .xticks(&[-4600000.0, -4400000.0, -4200000.0, -4000000.0])
            .unwrap();
        let error = explicit.to_scene(FontLibrary::global()).unwrap_err();
        assert!(error.to_string().contains("x tick label"), "{error}");
    }

    #[test]
    fn single_ticks_do_not_replace_pairs_that_fit_their_own_layout() {
        let lib = FontLibrary::global();
        for (x, y) in [
            (
                [-0.0004267591936039441, 0.00038715044386950377],
                [0.001408778775249747, 0.005912739894112307],
            ),
            (
                [8084.741921390828, 70243.01067776655],
                [37488.93087405191, 115616.02260363969],
            ),
            (
                [479470.348278096, 653703.333820003],
                [-403678.5715343826, 425554.13491157634],
            ),
        ] {
            for (w, h) in [(2.5, 1.8), (3.2, 2.6)] {
                let theme = Theme::pigment().with_font_scale(1.4).unwrap();
                let mut f = Figure::new(w, h, theme).unwrap();
                f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                    .unwrap()
                    .scatter(
                        AxisData::Numbers(x.to_vec()),
                        &y,
                        crate::ScatterOptions::default(),
                    )
                    .unwrap();
                let (r, _) = try_plan_figure(&f, (0, 0), lib).unwrap();
                for side in Side::BOTH {
                    if side.ticks(&r).len() != 1 {
                        continue;
                    }
                    let (xlo, xhi) = r.x.continuous().unwrap().domain();
                    let (ylo, yhi) = r.y.continuous().unwrap().domain();
                    let (lo, hi) = match side {
                        Side::X => (xlo, xhi),
                        Side::Y => (ylo, yhi),
                    };
                    let exponent = (hi - lo).log10().floor() as i32;
                    for power in exponent - 2..=exponent + 1 {
                        for mantissa in [1.0, 2.0, 2.5, 5.0] {
                            let step = mantissa * 10f64.powi(power);
                            let first = (lo / step).ceil() as i64;
                            let last = (hi / step).floor() as i64;
                            for index in first..last {
                                let pair = [index as f64 * step, (index + 1) as f64 * step];
                                if lo <= 0.0 && hi >= 0.0 && !pair.contains(&0.0) {
                                    continue;
                                }
                                let mut trial = f.clone();
                                let mut a = trial.single_axes_mut();
                                a.xlim(xlo, xhi).unwrap().ylim(ylo, yhi).unwrap();
                                let xs: Vec<_> =
                                    r.x.ticks
                                        .iter()
                                        .map(|t| xlo + t.pos.get() * (xhi - xlo))
                                        .collect();
                                let ys: Vec<_> =
                                    r.y.ticks
                                        .iter()
                                        .map(|t| ylo + t.pos.get() * (yhi - ylo))
                                        .collect();
                                a.xticks_labeled(
                                    &xs,
                                    r.x.ticks.iter().map(|t| t.label.clone()).collect(),
                                )
                                .unwrap();
                                a.yticks_labeled(
                                    &ys,
                                    r.y.ticks.iter().map(|t| t.label.clone()).collect(),
                                )
                                .unwrap();
                                match side {
                                    Side::X => a.xticks(&pair),
                                    Side::Y => a.yticks(&pair),
                                }
                                .unwrap();
                                if let Ok((tr, layout)) = try_plan_figure(&trial, (0, 0), lib) {
                                    let slack = side
                                        .room(
                                            layout.plot.size(),
                                            crate::test_support::tick_font(trial.theme()),
                                        )
                                        .slack(side.ticks(&tr));
                                    assert!(
                                        slack < 0.0,
                                        "{x:?} {y:?} {w}x{h} {side:?}: fitting pair {pair:?}, slack {slack}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn explicit_log_limits_get_ticks_that_fit() {
        // Log limits 2 to 50 hold no two powers of 10, so the ticks are linear. Candidates
        // with fewer than two ticks inside the limits must not stand in for a pair that fits.
        let mut f = Figure::new(300.0 / 72.0, 80.0 / 72.0, Theme::pigment()).unwrap();
        let log = ScaleKind::Log {
            base: crate::LogBase::TEN,
        };
        f.add_axes(ScaleKind::Linear, log)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 10.0]),
                &[2.0, 50.0],
                LineOptions::default(),
            )
            .unwrap()
            .ylim(2.0, 50.0)
            .unwrap();
        let (labels, slack) = final_ticks(&f, Side::Y);
        assert_eq!(labels, ["10", "50"]);
        assert!(slack >= 0.0, "{labels:?} crowd by {}", -slack);
    }

    #[test]
    fn log_axes_keep_their_labels_apart() {
        let log = ScaleKind::Log {
            base: crate::LogBase::TEN,
        };
        let y_ticks = |f: &Figure, size: PlotSize, spacing| {
            let fonts = crate::test_support::figure_fonts(f.theme());
            let (resolved, _, room) = resolved_at(f, size, spacing, &fonts).remove(0);
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
            /// A line: exact x limits, padded y limits (rounded first on a log axis).
            Line,
            /// Points: padded limits on both axes (rounded first on log axes).
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

        /// One random axes: linear or log axes, a mark of `kind` over the `x` and `y` ranges,
        /// explicit limits on either axis, and a percent x format.
        #[derive(Debug, Clone)]
        struct Spec {
            /// Whether the x and the y axis are log axes.
            logs: (bool, bool),
            /// The x and the y data ranges.
            ranges: ((f64, f64), (f64, f64)),
            /// The mark.
            kind: Kind,
            /// Whether the x and the y axis have explicit limits at the data range.
            limits: (bool, bool),
            /// Whether the x axis has a percent format.
            percent: bool,
        }

        /// A random [`Spec`].
        fn spec() -> impl Strategy<Value = Spec> {
            (
                (any::<bool>(), any::<bool>())
                    .prop_flat_map(|(a, b)| (Just((a, b)), (range(a), range(b)))),
                prop_oneof![Just(Kind::Line), Just(Kind::Scatter), Just(Kind::Area)],
                (any::<bool>(), any::<bool>()),
                any::<bool>(),
            )
                .prop_map(|((logs, ranges), kind, limits, percent)| Spec {
                    logs,
                    ranges,
                    kind,
                    limits,
                    percent,
                })
        }

        /// The kind of an axis: log when `log`, else linear.
        fn scale(log: bool) -> ScaleKind {
            if log {
                ScaleKind::Log { base: LogBase::TEN }
            } else {
                ScaleKind::Linear
            }
        }

        /// Draws `spec` on `axes`.
        fn draw(axes: &mut crate::AxesMut<'_>, spec: &Spec) {
            let ((x, y), y_log) = (spec.ranges, spec.logs.1);
            let xs = AxisData::Numbers(vec![x.0, (x.0 + x.1) / 2.0, x.1]);
            let ys = [y.0, (y.0 + y.1) / 2.0, y.1];
            match spec.kind {
                Kind::Line => axes.line(xs, &ys, LineOptions::default()),
                Kind::Scatter => axes.scatter(xs, &ys, ScatterOptions::default()),
                Kind::Area if !y_log => axes.area(xs, &ys, AreaOptions::default()),
                Kind::Area => axes.line(xs, &ys, LineOptions::default()),
            }
            .unwrap();
            if spec.limits.0 {
                axes.xlim(x.0, x.1).unwrap();
            }
            if spec.limits.1 {
                axes.ylim(y.0, y.1).unwrap();
            }
            if spec.percent {
                axes.xtick_format(TickFormat::percent(1).unwrap()).unwrap();
            }
        }

        /// A `width` × `height` in figure of one axes drawn as `spec` says.
        fn figure(theme: Theme, (width, height): (f64, f64), spec: &Spec) -> Figure {
            let mut f = Figure::new(width, height, theme).unwrap();
            let mut axes = f.add_axes(scale(spec.logs.0), scale(spec.logs.1)).unwrap();
            draw(&mut axes, spec);
            f
        }

        /// A `rows × cols` figure at `width` × `height` in with one axes per cell, drawn as
        /// `specs` says, in row-major order.
        fn grid_figure(
            theme: Theme,
            (width, height): (f64, f64),
            (rows, cols): (i64, i64),
            specs: &[Spec],
        ) -> Figure {
            let grid = Grid::new(rows, cols).unwrap();
            let mut f = Figure::with_grid(width, height, theme, grid).unwrap();
            for (i, spec) in (0..rows * cols).zip(specs) {
                let at = Placement {
                    row: GridIndex::At(i / cols),
                    col: GridIndex::At(i % cols),
                    ..Placement::default()
                };
                let mut axes = f
                    .add_axes_at(at, scale(spec.logs.0), scale(spec.logs.1))
                    .unwrap();
                draw(&mut axes, spec);
            }
            f
        }

        /// Checks every side with automatic ticks of every panel of `figure` as laid out: its
        /// labels keep apart on the plot drawn, or it is a fallback of at most three ticks. A
        /// figure too small for its texts is skipped.
        fn check_final(figure: &Figure) -> std::result::Result<(), TestCaseError> {
            let lib = FontLibrary::global();
            let Ok(planned) = try_plan_panels(figure, lib) else {
                return Ok(());
            };
            for ((resolved, layout), axes) in planned.iter().zip(figure.layers()) {
                let limits = Limits::of(axes);
                for side in Side::BOTH {
                    if !side.is_automatic(&limits) {
                        continue;
                    }
                    let ticks = side.ticks(resolved);
                    let slack = side
                        .room(
                            layout.plot.size(),
                            crate::test_support::tick_font(figure.theme()),
                        )
                        .slack(ticks);
                    prop_assert!(
                        slack >= 0.0 || ticks.len() <= 3,
                        "{:?}: {} ticks crowd by {} pt on {:?}",
                        side,
                        ticks.len(),
                        -slack,
                        layout.plot
                    );
                }
            }
            Ok(())
        }

        /// Fitting multi-tick choices stay unchanged. A single tick can become a pair that
        /// fits its own layout; every domain and every other tick set stays unchanged.
        fn check_previous(figure: &Figure) -> std::result::Result<(), TestCaseError> {
            let lib = FontLibrary::global();
            let Ok(previous) = previous_plan(figure, lib) else {
                return Ok(());
            };
            if previous.iter().any(|(resolved, layout)| {
                Side::BOTH.into_iter().any(|side| {
                    side.room(
                        layout.plot.size(),
                        crate::test_support::tick_font(figure.theme()),
                    )
                    .slack(side.ticks(resolved))
                        < 0.0
                })
            }) {
                return Ok(());
            }
            let current = try_plan_panels(figure, lib)?;
            prop_assert_eq!(current.len(), previous.len());
            let upgraded = current.iter().zip(&previous).any(|((new, _), (old, _))| {
                Side::BOTH
                    .into_iter()
                    .any(|side| side.ticks(old).len() == 1 && side.ticks(new).len() == 2)
            });
            for ((current, layout), (previous, previous_layout)) in current.iter().zip(&previous) {
                for (side, new, old) in [
                    (Side::X, &current.x, &previous.x),
                    (Side::Y, &current.y, &previous.y),
                ] {
                    prop_assert_eq!(&new.scale, &old.scale);
                    if old.ticks.len() == 1 && new.ticks.len() == 2 {
                        prop_assert!(is_round(new.continuous().unwrap(), &new.ticks));
                        prop_assert!(
                            side.room(
                                layout.plot.size(),
                                crate::test_support::tick_font(figure.theme())
                            )
                            .slack(&new.ticks)
                                >= 0.0
                        );
                    } else {
                        prop_assert_eq!(new, old);
                    }
                }
                prop_assert_eq!(current.baseline, previous.baseline);
                prop_assert_eq!(&current.color, &previous.color);
                if !upgraded {
                    prop_assert_eq!(format!("{layout:?}"), format!("{previous_layout:?}"));
                }
            }
            Ok(())
        }

        /// One resolved axis: its domain and its ticks.
        type Axis<'a> = (Option<&'a crate::ContinuousScale>, &'a [AxisTick]);

        /// Whether the values of `ticks` on the linear `scale` are round: a step of 1, 2, 2.5
        /// or 5 times a power of ten, and every value a whole multiple of it.
        fn is_round(scale: &crate::ContinuousScale, ticks: &[AxisTick]) -> bool {
            let (lo, hi) = scale.domain();
            let values: Vec<f64> = ticks.iter().map(|t| lo + t.pos.get() * (hi - lo)).collect();
            let step = values[1] - values[0];
            let near = |a: f64, b: f64| (a - b).abs() <= 1e-6 * b.abs().max(1.0);
            let mantissa = step / 10f64.powf(step.log10().floor());
            [1.0, 2.0, 2.5, 5.0, 10.0]
                .iter()
                .any(|&m| near(mantissa, m))
                && values.iter().all(|v| near(v / step, (v / step).round()))
        }

        /// Checks one axis: its labels keep apart, or none of its candidates could and it
        /// fell back to a few ticks on a short axis; ticks whose labels were apart for the
        /// target alone stay as they were, on the same domain; and ticks chosen again on a
        /// linear axis are round, unless they are a fallback of two ticks, or three with the
        /// lowest moved below the data.
        fn check_axis(
            name: &str,
            room: &layout::TickRoom<'_>,
            new: Axis<'_>,
            old: Axis<'_>,
        ) -> std::result::Result<(), TestCaseError> {
            let ticks = new.1;
            let linear = new
                .0
                .is_some_and(|scale| scale.transform() == crate::Transform::Linear);
            if room.slack(ticks) < 0.0 {
                // The widest candidate: two ticks, or three with the lowest moved below the
                // data, on a linear axis so short that the outer labels crowd a quarter of it
                // apart. A log axis draws linear ticks unevenly, so its widest pair in value
                // need not be the widest on screen.
                prop_assert!(ticks.len() <= 3, "{}: {} ticks crowd", name, ticks.len());
                let at = |tick: &AxisTick, pos: f64| AxisTick {
                    pos: NormalizedPos::new(pos),
                    label: tick.label.clone(),
                };
                let outer = [at(&ticks[0], 0.0), at(&ticks[ticks.len() - 1], 0.25)];
                prop_assert!(!linear || room.slack(&outer) < 0.0, "{}: {:?}", name, ticks);
            }
            if room.slack(old.1) >= 0.0 {
                prop_assert_eq!(new, old, "{}", name);
            } else if let Some(scale) = new.0
                && linear
                && ticks.len() > 3
            {
                prop_assert!(is_round(scale, ticks), "{}: {:?}", name, ticks);
            }
            Ok(())
        }

        #[test]
        fn narrow_explicit_y_limits_keep_labels_apart() {
            // Step 0.0025 fits vertically, but its extra decimal makes the x percent label
            // too wide for the figure. Step 0.002 fits both axes on its own solved layout.
            let theme = Theme::midnight()
                .with_font_scale(2.024411942327032)
                .unwrap();
            let spec = Spec {
                logs: (false, false),
                ranges: (
                    (-6876.48483212639, -6875.48483212639),
                    (6768.551347618901, 6768.555197244765),
                ),
                kind: Kind::Line,
                limits: (false, true),
                percent: true,
            };
            let f = figure(theme, (2.3959016648743408, 2.31922920500325), &spec);
            let lib = FontLibrary::global();
            let (resolved, layout) = try_plan_panels(&f, lib).unwrap().remove(0);
            assert_eq!(resolved.y.continuous().unwrap().domain(), spec.ranges.1);
            for side in Side::BOTH {
                let ticks = side.ticks(&resolved);
                let slack = side
                    .room(
                        layout.plot.size(),
                        crate::test_support::tick_font(f.theme()),
                    )
                    .slack(ticks);
                assert!(slack >= 0.0, "{side:?}: {ticks:?}, slack {slack}");
            }
            assert_eq!(tick_labels(&resolved.y.ticks), ["6768.552", "6768.554"]);
            assert!(is_round(
                resolved.y.continuous().unwrap(),
                &resolved.y.ticks
            ));
            f.to_svg(crate::SvgText::Editable).unwrap();
        }

        #[test]
        fn narrow_explicit_y_limits_use_one_tick_when_pairs_do_not_fit() {
            // Step 0.002 still crowds vertically; step 0.0025 makes the x label too wide.
            // No whole-step pair fits, but one existing tick fits without moving the domain.
            let theme = Theme::midnight()
                .with_font_scale(1.1861975422164643)
                .unwrap();
            let spec = Spec {
                logs: (false, false),
                ranges: (
                    (9452.723644157226, 9453.723644157226),
                    (6356.58126608091, 6356.589845896476),
                ),
                kind: Kind::Line,
                limits: (false, true),
                percent: true,
            };
            let f = figure(theme, (1.4407630357200345, 1.6462928522871103), &spec);
            let lib = FontLibrary::global();
            let (resolved, layout) = try_plan_panels(&f, lib).unwrap().remove(0);
            assert_eq!(resolved.y.continuous().unwrap().domain(), spec.ranges.1);
            for side in Side::BOTH {
                let ticks = side.ticks(&resolved);
                let slack = side
                    .room(
                        layout.plot.size(),
                        crate::test_support::tick_font(f.theme()),
                    )
                    .slack(ticks);
                assert!(slack >= 0.0, "{side:?}: {ticks:?}, slack {slack}");
            }
            assert_eq!(tick_labels(&resolved.y.ticks), ["6356.586"]);
            f.to_svg(crate::SvgText::Editable).unwrap();
        }

        #[test]
        fn narrow_log_y_axis_keeps_labels_apart() {
            // Refitting fewer powers lowers the domain again and writes 0.001, which leaves
            // no room for the x percent label. Thin the existing powers without refitting.
            let theme = Theme::pigment()
                .with_font_scale(1.7418145436733836)
                .unwrap();
            let spec = Spec {
                logs: (false, true),
                ranges: ((0.0, 1.0), (0.10207785494925446, 2.2643981627494667)),
                kind: Kind::Line,
                limits: (false, false),
                percent: true,
            };
            let f = figure(theme, (1.04728763133498, 1.767157525467884), &spec);
            let lib = FontLibrary::global();
            let (resolved, layout) = try_plan_panels(&f, lib).unwrap().remove(0);
            let fonts = layout::figure_fonts(lib, f.theme()).unwrap();
            let (target, _, _) =
                resolved_at(&f, layout.plot.size(), TickSpacing::Target, &fonts).remove(0);
            assert_eq!(resolved.y.scale, target.y.scale);
            assert_eq!(tick_labels(&target.y.ticks), ["0.01", "0.1", "1", "10"]);
            for side in Side::BOTH {
                let ticks = side.ticks(&resolved);
                let slack = side
                    .room(
                        layout.plot.size(),
                        crate::test_support::tick_font(f.theme()),
                    )
                    .slack(ticks);
                assert!(slack >= 0.0, "{side:?}: {ticks:?}, slack {slack}");
            }
            assert_eq!(tick_labels(&resolved.y.ticks), ["0.01", "10"]);
            f.to_svg(crate::SvgText::Editable).unwrap();
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(192))]

            #[test]
            fn automatic_tick_labels_keep_apart(
                theme in 0usize..3,
                font_scale in 0.5f64..2.5,
                spec in spec(),
                width in 20.0f64..800.0,
                height in 20.0f64..800.0,
            ) {
                let theme = [Theme::pigment(), Theme::graphite(), Theme::midnight()][theme]
                    .with_font_scale(font_scale)
                    .unwrap();
                let f = figure(theme, (6.0, 4.0), &spec);
                let fonts = crate::test_support::figure_fonts(f.theme());
                let size = PlotSize { width, height };
                let (old, _, _) = resolved_at(&f, size, TickSpacing::Target, &fonts).remove(0);
                let (new, x_room, y_room) =
                    resolved_at(&f, size, TickSpacing::LabelsApart, &fonts).remove(0);
                let x_axis = |r: &Resolved| (r.x.continuous().copied(), r.x.ticks.clone());
                let y_axis = |r: &Resolved| (r.y.continuous().copied(), r.y.ticks.clone());
                for (name, room, new, old) in [
                    ("x", &x_room, x_axis(&new), x_axis(&old)),
                    ("y", &y_room, y_axis(&new), y_axis(&old)),
                ] {
                    check_axis(name, room, (new.0.as_ref(), &new.1), (old.0.as_ref(), &old.1))?;
                }
            }

            #[test]
            fn fitting_previous_choices_keep_ticks_and_layout(
                theme in 0usize..3,
                font_scale in 0.5f64..2.5,
                spec in spec(),
                width in 0.8f64..8.0,
                height in 0.6f64..6.0,
            ) {
                let theme = [Theme::pigment(), Theme::graphite(), Theme::midnight()][theme]
                    .with_font_scale(font_scale)
                    .unwrap();
                check_previous(&figure(theme, (width, height), &spec))?;
            }

            #[test]
            fn fitting_previous_grid_choices_keep_ticks_and_layout(
                theme in 0usize..3,
                font_scale in 0.5f64..1.5,
                (rows, cols) in (1i64..4, 1i64..4),
                specs in proptest::collection::vec(spec(), 9),
                width in 2.0f64..14.0,
                height in 1.5f64..10.0,
            ) {
                let theme = [Theme::pigment(), Theme::graphite(), Theme::midnight()][theme]
                    .with_font_scale(font_scale)
                    .unwrap();
                check_previous(&grid_figure(theme, (width, height), (rows, cols), &specs))?;
            }

            #[test]
            fn final_layouts_keep_tick_labels_apart(
                theme in 0usize..3,
                font_scale in 0.5f64..2.5,
                spec in spec(),
                width in 0.8f64..8.0,
                height in 0.6f64..6.0,
            ) {
                let theme = [Theme::pigment(), Theme::graphite(), Theme::midnight()][theme]
                    .with_font_scale(font_scale)
                    .unwrap();
                check_final(&figure(theme, (width, height), &spec))?;
            }

            #[test]
            fn final_grid_layouts_keep_tick_labels_apart(
                theme in 0usize..3,
                font_scale in 0.5f64..1.5,
                (rows, cols) in (1i64..4, 1i64..4),
                specs in proptest::collection::vec(spec(), 9),
                width in 2.0f64..14.0,
                height in 1.5f64..10.0,
            ) {
                let theme = [Theme::pigment(), Theme::graphite(), Theme::midnight()][theme]
                    .with_font_scale(font_scale)
                    .unwrap();
                check_final(&grid_figure(theme, (width, height), (rows, cols), &specs))?;
            }
        }
    }
}
