//! Axis groups, their immutable settings and the panels that set them.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::axes::scale_name;
use super::{Axes, AxisTicks, Cell, CellRange, Figure, Grid, Share};
use crate::error::number;
use crate::{ContinuousScale, Error, Result, ScaleKind};

/// The values shared by one axis group. Axes borrow them through an immutable shared pointer;
/// replacing a group's pointer leaves copies of the figure and axes independent.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct AxisSettings {
    /// The explicit limits, already validated as a scale.
    pub limits: Option<ContinuousScale>,
    /// The explicit ticks and their format.
    pub ticks: AxisTicks,
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
}

impl Group {
    /// Checks a proposed value, then records it. A member may replace its own value until
    /// another member explicitly sets the same value. Errors change nothing.
    pub fn set(
        &mut self,
        axis: SharedAxis,
        setting: Setting,
        cell: Cell,
        proposed: AxisSettings,
        context: &'static str,
    ) -> Result<()> {
        let (same, by, what, previous, parameter) = match setting {
            Setting::Limits => {
                let previous = self
                    .settings
                    .limits
                    .map(|s| {
                        let (low, high) = s.domain();
                        format!("({}, {})", number(low), number(high))
                    })
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
        };
        if !same && let Some(other) = by.iter().find(|other| **other != cell) {
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
        if !by.contains(&cell) {
            by.push(cell);
        }
        self.settings = Arc::new(proposed);
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
        if let Some(other) = figure.panels().find(|axes| {
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
    for axis in [SharedAxis::X, SharedAxis::Y] {
        let crossed = match axis.mode(grid) {
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
                    axis.name(),
                ),
            )
            .with_parameter(parameter)
            .with_hint("span within one shared group, or share by ALL or NONE."));
        }
    }
    Ok(())
}

/// Panel index to group representative, independently for x and y. A representative is the
/// first panel in row-major order, not insertion order.
#[derive(Debug, Clone)]
pub(crate) struct ShareGroups {
    /// The representative of each panel's x group.
    pub x: Vec<usize>,
    /// The representative of each panel's y group.
    pub y: Vec<usize>,
}

impl ShareGroups {
    /// Builds the maps for the figure's placed axes.
    pub fn new(figure: &Figure) -> Self {
        let representatives = |axis: SharedAxis| {
            let mut first = BTreeMap::new();
            figure
                .panels()
                .enumerate()
                .map(|(i, axes)| {
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
        }
    }
}

/// Whether the panel shows x and y tick labels. A span counts in every covered column/row;
/// its labels remain if it reaches the occupied outer edge in any of them.
pub(crate) fn label_visibility(figure: &Figure, cells: CellRange) -> [bool; 2] {
    let show_x = !matches!(figure.grid().share_x(), Share::Col | Share::All)
        || (cells.col()..cells.col() + cells.col_span()).any(|col| {
            !figure.panels().any(|a| {
                let c = a.cells();
                c.col() <= col
                    && col < c.col() + c.col_span()
                    && c.row() >= cells.row() + cells.row_span()
            })
        });
    let show_y = !matches!(figure.grid().share_y(), Share::Row | Share::All)
        || (cells.row()..cells.row() + cells.row_span()).any(|row| {
            !figure.panels().any(|a| {
                let c = a.cells();
                c.row() <= row
                    && row < c.row() + c.row_span()
                    && c.col() + c.col_span() <= cells.col()
            })
        });
    [show_x, show_y]
}
