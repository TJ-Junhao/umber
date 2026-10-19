//! Axis groups, their immutable settings and the panels that set them.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::axes::{scale_name, size_name};
use super::{Axes, AxisTicks, Cell, CellRange, Figure, Grid, Share};
use crate::error::number;
use crate::{ContinuousScale, Error, Instant, Result, ScaleKind, SizeSpec};

/// The explicit limits of an axis, validated: the scale they define on a linear or log axis,
/// the lower and the upper time on a time axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ExplicitLimits {
    /// The limits of a linear or log axis, as the scale they define.
    Continuous(ContinuousScale),
    /// The limits of a time axis, the lower below the upper.
    Time(Instant, Instant),
}

impl ExplicitLimits {
    /// The limits as messages write them, such as `(0, 10)` or `(2026-01-01, 2026-02-01)`.
    fn label(self) -> String {
        match self {
            ExplicitLimits::Continuous(scale) => {
                let (low, high) = scale.domain();
                format!("({}, {})", number(low), number(high))
            }
            ExplicitLimits::Time(low, high) => format!("({}, {})", low.label(), high.label()),
        }
    }
}

/// The values shared by one axis group. Axes borrow them through an immutable shared pointer;
/// replacing a group's pointer leaves copies of the figure and axes independent.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct AxisSettings {
    /// The explicit limits, already validated.
    pub limits: Option<ExplicitLimits>,
    /// The explicit ticks and their format.
    pub ticks: AxisTicks,
    /// The tick label size shared independently of ticks and their format.
    pub tick_size: SizeSpec,
}

/// A coordinate axis.
#[derive(Debug, Clone, Copy)]
pub(super) enum SharedAxis {
    /// Horizontal.
    X,
    /// Vertical.
    Y,
}

impl SharedAxis {
    /// The argument's name.
    pub fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
        }
    }

    /// The hint for this axis when it is a category axis but needs to be linear or log.
    pub const fn numeric_scale_hint(self) -> &'static str {
        match self {
            Self::X => "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().",
            Self::Y => "pass y=Scale.linear() or y=Scale.log() to Figure.add_axes().",
        }
    }

    /// The hint for this axis when it is linear or log but needs to be a category axis.
    pub const fn category_scale_hint(self) -> &'static str {
        match self {
            Self::X => "pass x=Scale.category() to Figure.add_axes().",
            Self::Y => "pass y=Scale.category() to Figure.add_axes().",
        }
    }

    /// The hint for this axis when it is not a time axis but needs to be one.
    pub const fn time_scale_hint(self) -> &'static str {
        match self {
            Self::X => "pass x=Scale.time() to Figure.add_axes().",
            Self::Y => "pass y=Scale.time() to Figure.add_axes().",
        }
    }

    /// The sharing mode of this dimension.
    pub fn mode(self, grid: &Grid) -> Share {
        match self {
            Self::X => grid.share_x(),
            Self::Y => grid.share_y(),
        }
    }

    /// The scale kind on `axes`.
    fn kind(self, axes: &Axes) -> ScaleKind {
        match self {
            Self::X => axes.x_scale(),
            Self::Y => axes.y_scale(),
        }
    }
}

/// A group's stable key, independent of panel insertion order.
pub(super) fn group_key(grid: &Grid, share: Share, row: usize, col: usize) -> usize {
    match share {
        Share::None => row * grid.cols() + col,
        Share::Row => row,
        Share::Col => col,
        Share::All => 0,
    }
}

/// One setting changed by a call; explicit positions and labeled positions are one setting.
#[derive(Debug, Clone, Copy)]
pub(super) enum Setting {
    /// Limits.
    Limits,
    /// Explicit ticks, with or without labels.
    Ticks,
    /// Tick format.
    Format,
    /// Tick label size.
    TickSize,
}

/// Records an explicit setter unless it changes a value set by another member. Inheriting a
/// value does not record a setter; an error returns the first conflicting cell without mutation.
fn record_setter(by: &mut Vec<Cell>, cell: Cell, same: bool) -> std::result::Result<(), Cell> {
    if !same && let Some(other) = by.iter().find(|other| **other != cell) {
        return Err(*other);
    }
    if !by.contains(&cell) {
        by.push(cell);
    }
    Ok(())
}

/// One group stored by the figure, with a single allocation for its values and the cells of
/// members that explicitly set each value. Inheriting a value does not count as setting it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Group {
    /// The values; every member points to this same allocation.
    pub settings: Arc<AxisSettings>,
    /// Members that set limits.
    limits_by: Vec<Cell>,
    /// Members that set ticks.
    ticks_by: Vec<Cell>,
    /// Members that set the format, including explicitly setting AUTO.
    format_by: Vec<Cell>,
    /// Members that set an explicit tick label size; AUTO removes a member's setting.
    tick_size_by: Vec<Cell>,
}

impl Group {
    /// Checks a proposed value, then records it. A member may replace its own value until
    /// another member explicitly sets the same value. Errors change nothing.
    pub fn set(
        &mut self,
        axis: SharedAxis,
        setting: Setting,
        cell: Cell,
        mut proposed: AxisSettings,
        context: &'static str,
    ) -> Result<()> {
        if matches!(setting, Setting::TickSize) && proposed.tick_size == SizeSpec::Auto {
            self.tick_size_by.retain(|other| *other != cell);
            if !self.tick_size_by.is_empty() {
                proposed.tick_size = self.settings.tick_size;
            }
            self.settings = Arc::new(proposed);
            return Ok(());
        }
        let (same, by, what, previous, parameter) = match setting {
            Setting::Limits => {
                let previous = self
                    .settings
                    .limits
                    .map(ExplicitLimits::label)
                    .unwrap_or_default();
                (
                    self.settings.limits == proposed.limits,
                    &mut self.limits_by,
                    "limits",
                    previous,
                    "low",
                )
            }
            Setting::Ticks => (
                self.settings.ticks.explicit() == proposed.ticks.explicit(),
                &mut self.ticks_by,
                "ticks",
                "other ticks".to_owned(),
                "positions",
            ),
            Setting::Format => (
                self.settings.ticks.format() == proposed.ticks.format(),
                &mut self.format_by,
                "tick format",
                "another format".to_owned(),
                "format",
            ),
            Setting::TickSize => (
                self.settings.tick_size == proposed.tick_size,
                &mut self.tick_size_by,
                "tick label size",
                size_name(self.settings.tick_size),
                "size",
            ),
        };
        if let Err(other) = record_setter(by, cell, same) {
            if matches!(setting, Setting::TickSize) {
                return Err(Error::usage(
                    context,
                    format!(
                        "size is {}; row {}, col {} uses {previous}, and the {} axis is shared.",
                        size_name(proposed.tick_size),
                        other.row,
                        other.col,
                        axis.name(),
                    ),
                )
                .with_parameter("size")
                .with_hint("set the same size on every panel."));
            }
            let verb = if matches!(setting, Setting::Format) {
                "is"
            } else {
                "are"
            };
            return Err(Error::usage(
                context,
                format!(
                    "{} {what} {verb} shared with row {}, col {}, which set {previous}.",
                    axis.name(),
                    other.row,
                    other.col,
                ),
            )
            .with_parameter(parameter)
            .with_hint("set it once for the group, or share fewer axes."));
        }
        self.settings = Arc::new(proposed);
        Ok(())
    }
}

/// A color group's limits and the members that explicitly set them.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ColorGroup {
    /// Validated color limits, also inherited by panels added later.
    pub limits: Option<(f64, f64)>,
    /// Inheritance does not count as explicitly setting limits.
    by: Vec<Cell>,
}

impl ColorGroup {
    /// Records limits only after checking the other setters in the group.
    pub fn set(&mut self, cell: Cell, limits: (f64, f64)) -> Result<()> {
        if let Err(other) = record_setter(&mut self.by, cell, self.limits == Some(limits)) {
            let previous = self.limits.map_or_else(String::new, |(low, high)| {
                format!("({}, {})", number(low), number(high))
            });
            return Err(Error::usage(
                "Axes.clim()",
                format!(
                    "color limits are shared with row {}, col {}, which set {previous}.",
                    other.row, other.col
                ),
            )
            .with_parameter("low")
            .with_hint("set it once for the group, or share colors across fewer panels."));
        }
        self.limits = Some(limits);
        Ok(())
    }
}

/// Checks scale kinds first, x before y, then spans, before a new panel or group is stored.
pub(super) fn check_add(
    figure: &Figure,
    cells: CellRange,
    x: ScaleKind,
    y: ScaleKind,
) -> Result<()> {
    let grid = figure.grid();
    for (axis, given) in [(SharedAxis::X, x), (SharedAxis::Y, y)] {
        let mode = axis.mode(grid);
        let key = group_key(grid, mode, cells.row(), cells.col());
        if let Some(other) = figure.primary_cartesian_axes().find(|axes| {
            let c = axes.cells();
            group_key(grid, mode, c.row(), c.col()) == key && axis.kind(axes) != given
        }) {
            return Err(Error::usage(
                "Figure.add_axes()",
                format!(
                    "{} is {}; the axes at row {}, col {} shares its {} axis and has {}.",
                    axis.name(),
                    scale_name(given),
                    other.cells().row(),
                    other.cells().col(),
                    axis.name(),
                    scale_name(axis.kind(other)),
                ),
            )
            .with_parameter(axis.name())
            .with_hint("axes that share an axis need the same scale."));
        }
    }
    for (mode, name) in [(grid.share_x(), "x"), (grid.share_y(), "y")] {
        let crossed = match mode {
            Share::Row if cells.row_span() > 1 => {
                Some(("rows", "row_span", cells.row(), cells.row_span()))
            }
            Share::Col if cells.col_span() > 1 => {
                Some(("columns", "col_span", cells.col(), cells.col_span()))
            }
            _ => None,
        };
        if let Some((dimension, parameter, start, span)) = crossed {
            return Err(Error::usage(
                "Figure.add_axes()",
                format!(
                    "the axes spans {dimension} {start} to {}, which share {} in different groups.",
                    start + span - 1,
                    name,
                ),
            )
            .with_parameter(parameter)
            .with_hint("span within one shared group, or share by ALL or NONE."));
        }
    }
    Ok(())
}

/// Panel index to group representative for each dimension. A representative is the first
/// participating panel in row-major order, not insertion order.
#[derive(Debug, Clone)]
pub(crate) struct ShareGroups {
    /// The representative of each panel's x group.
    pub x: Vec<usize>,
    /// The representative of each panel's y group.
    pub y: Vec<usize>,
    /// The first color-mapped member of each color group; unmapped panels do not join.
    pub color: Vec<Option<usize>>,
}

impl ShareGroups {
    /// The first color-mapped member of each color group; panels without a mapping do not join.
    fn colors(figure: &Figure) -> Vec<Option<usize>> {
        let mut first = BTreeMap::new();
        figure
            .layers()
            .enumerate()
            .map(|(i, panel)| {
                let axes = panel.cartesian()?;
                if !axes.marks().iter().any(|mark| mark.color_scale().is_some()) {
                    return None;
                }
                if figure.grid().share_color() == Share::None {
                    return Some(i);
                }
                let c = axes.cells();
                let key = figure.color_group_of(c.row(), c.col());
                Some(*first.entry(key).or_insert(i))
            })
            .collect()
    }

    /// Builds the maps for the figure's placed axes.
    pub fn new(figure: &Figure) -> Self {
        let representatives = |axis: SharedAxis| {
            let mut first = BTreeMap::new();
            figure
                .layers()
                .enumerate()
                .map(|(i, layer)| {
                    let axes = match layer {
                        super::Layer::Primary(panel) => match panel.cartesian() {
                            Some(axes) => axes,
                            None => return i,
                        },
                        super::Layer::Twin { parent, .. } => {
                            return match axis {
                                SharedAxis::X => parent,
                                SharedAxis::Y => i,
                            };
                        }
                    };
                    let c = axes.cells();
                    let key = match axis {
                        SharedAxis::X => figure.x_group_of(c.row(), c.col()),
                        SharedAxis::Y => figure.y_group_of(c.row(), c.col()),
                    };
                    *first.entry(key).or_insert(i)
                })
                .collect()
        };
        Self {
            x: representatives(SharedAxis::X),
            y: representatives(SharedAxis::Y),
            color: Self::colors(figure),
        }
    }

    /// The cells spanned by each shared bar, stored only at its group's representative.
    /// Unmapped panels still extend the occupied row or column past which its bar sits.
    pub fn colorbar_cells(&self, figure: &Figure) -> Vec<Option<CellRange>> {
        match figure.grid().share_color() {
            Share::None => vec![None; self.color.len()],
            Share::Row => self.color_extents(figure, SharedAxis::Y),
            Share::Col => self.color_extents(figure, SharedAxis::X),
            Share::All => self
                .color
                .iter()
                .enumerate()
                .map(|(i, first)| {
                    (*first == Some(i))
                        .then(|| CellRange::new(0, 0, figure.grid().rows(), figure.grid().cols()))
                })
                .collect(),
        }
    }

    /// Row/column groups include every occupied panel overlapping that row/column, even a
    /// non-member that spans several groups. The other dimension ends at the last such panel.
    fn color_extents(&self, figure: &Figure, axis: SharedAxis) -> Vec<Option<CellRange>> {
        let dimensions = |cells: CellRange| match axis {
            SharedAxis::X => (
                cells.col(),
                cells.col_span(),
                cells.row() + cells.row_span(),
            ),
            SharedAxis::Y => (
                cells.row(),
                cells.row_span(),
                cells.col() + cells.col_span(),
            ),
        };
        figure
            .layers()
            .enumerate()
            .map(|(i, axes)| {
                if self.color[i] != Some(i) {
                    return None;
                }
                let (start, span, own_end) = dimensions(axes.cells());
                let end = figure
                    .layers()
                    .map(|other| dimensions(other.cells()))
                    .filter(|(other, size, _)| *other < start + span && start < other + size)
                    .map(|(_, _, end)| end)
                    .max()
                    .unwrap_or(own_end);
                Some(match axis {
                    SharedAxis::X => CellRange::new(0, start, end, span),
                    SharedAxis::Y => CellRange::new(start, 0, span, end),
                })
            })
            .collect()
    }
}

/// Whether the panel shows x and y tick labels. Shared x labels hide only behind an occupied
/// panel below covering exactly the same columns; shared y labels likewise to the left with
/// exactly the same rows. Matching columns/rows also put the panels in the same shared group.
pub(crate) fn label_visibility(figure: &Figure, cells: CellRange) -> [bool; 2] {
    let show_x = !matches!(figure.grid().share_x(), Share::Col | Share::All)
        || !figure.primary_cartesian_axes().any(|a| {
            let c = a.cells();
            c.col() == cells.col()
                && c.col_span() == cells.col_span()
                && c.row() >= cells.row() + cells.row_span()
        });
    let show_y = !matches!(figure.grid().share_y(), Share::Row | Share::All)
        || !figure.primary_cartesian_axes().any(|a| {
            let c = a.cells();
            c.row() == cells.row()
                && c.row_span() == cells.row_span()
                && c.col() + c.col_span() <= cells.col()
        });
    [show_x, show_y]
}
