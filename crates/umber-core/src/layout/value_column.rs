//! The room of a value column: the values that scatters with
//! [`Place::Column`](crate::Place::Column) write outside the plot, at the far end of the value
//! axis, and the column's title ([`ValueColumn`](crate::ValueColumn)).
//!
//! The values are measured before layout ([`crate::draw::point_labels`]), which gives the
//! layout the extent of each sub-column across the category axis ([`ColumnInput`]). The layout
//! reserves the column's room as it reserves a tick label column, so the plot narrows, or gets
//! shorter, by exactly that room:
//!
//! - **Right of the plot**, when x is the value axis: [`COLUMN_GAP`] and the column, as wide as
//!   its sub-columns side by side with [`SUB_COLUMN_GAP`] between them, or as its title when
//!   that is wider. The column sits outside everything else on that side of the panel: the
//!   room of direct labels and the colorbar; an outside legend on the right stays further
//!   out. A twin y axis, whose right tick labels would lie inside it too, never shares a panel
//!   with such a column: a twin needs numeric y axes, and the column comes with a category y
//!   axis. The sub-columns are centred in the column,
//!   and the title is centred over it, its baseline level with the y axis label's, in the band
//!   above the plot that the y axis label takes. That band grows to hold the title, under the
//!   panel's header, so the title never meets the panel title or a legend row above the plot.
//! - **Above the plot**, when y is the value axis: a row of sub-rows stacked top down with
//!   [`SUB_COLUMN_GAP`] between them, [`ROW_GAP`] above the highest point a y tick label
//!   reaches. The row sits under the header and the y axis labels, of a twin axis too, which
//!   move up by its height.
//!   Its title is written before it, right-aligned [`Y_LABEL_GAP`] left of the plot, as a y
//!   tick label is, and centred on the row vertically; the y tick label column widens to hold
//!   it.
//!
//! A figure caption stays under everything else, as it does without a column.

use super::{Area, Label, Y_LABEL_GAP, measure, metrics};
use crate::error::quoted;
use crate::text::{Lines, SceneFonts};
use crate::{Result, SizeSpec, TextStyle};

/// The gap between neighbouring sub-columns of a column right of the plot, and between
/// neighbouring sub-rows of a row above it, in points.
pub(crate) const SUB_COLUMN_GAP: f64 = 6.0;
/// The gap between a column and what lies inside it, right of the plot, in points: the gap
/// between the y tick labels and the plot.
pub(crate) const COLUMN_GAP: f64 = Y_LABEL_GAP;
/// The gap between a row above the plot and the highest point a y tick label reaches, in
/// points: the gap between the plot and the x tick labels.
pub(crate) const ROW_GAP: f64 = super::X_LABEL_GAP;

/// Column titles are one line.
const COLUMN_TITLES: Lines = Lines::One("value column titles");

/// Where a value column goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnSide {
    /// A column right of the plot, for values on the x axis.
    Right,
    /// A row above the plot, for values on the y axis.
    Above,
}

/// The value column of one axes as the layout needs it: its side, the extent of each of its
/// sub-columns across the category axis, in mark order (the width of each sub-column right of
/// the plot, the height of each sub-row above it), and its title.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnInput {
    /// Where the column goes.
    pub side: ColumnSide,
    /// The extent of each sub-column, in points.
    pub extents: Vec<f64>,
    /// The title; empty for none.
    pub title: String,
    /// The title's size.
    pub size: SizeSpec,
}

/// A value column, measured: its sub-columns and its title.
#[derive(Debug, Clone)]
pub(super) struct MeasuredColumn {
    /// Where the column goes.
    side: ColumnSide,
    /// The extent of each sub-column.
    extents: Vec<f64>,
    /// The sub-columns side by side, or the sub-rows stacked, with the gaps between them.
    block: f64,
    /// The title, unplaced; `None` when it is empty.
    title: Option<Label>,
    /// The ascent of the title style's primary font.
    title_ascent: f64,
    /// The descent of the title style's primary font.
    title_descent: f64,
    /// The gap between the title's baseline and the highest point a y tick label reaches, as
    /// the y axis label has it.
    title_gap: f64,
}

impl MeasuredColumn {
    /// Measures the column `input` with its title in `style`, the axis label style at the
    /// title's size, over y tick labels whose font descends `tick_descent`.
    ///
    /// # Errors
    ///
    /// `Error::Style` for a title that holds a line break or another control character; then
    /// as [`measure`], for a title that cannot be laid out.
    pub(super) fn measure(
        input: &ColumnInput,
        style: TextStyle,
        tick_descent: f64,
        lib: &SceneFonts<'_>,
    ) -> Result<MeasuredColumn> {
        lib.check_lines(
            &format!("the value column title {}", quoted(&input.title)),
            &input.title,
            COLUMN_TITLES,
        )?;
        let title = (!input.title.is_empty())
            .then(|| measure(lib, &input.title, style))
            .transpose()?;
        let (title_ascent, title_descent) = metrics(lib, &style)?;
        let gaps = SUB_COLUMN_GAP * input.extents.len().saturating_sub(1) as f64;
        Ok(MeasuredColumn {
            side: input.side,
            extents: input.extents.clone(),
            block: input.extents.iter().sum::<f64>() + gaps,
            title,
            title_ascent,
            title_descent,
            title_gap: super::title_tick_gap(title_descent, tick_descent),
        })
    }

    /// The width of the title; 0 without one.
    fn title_width(&self) -> f64 {
        self.title.as_ref().map_or(0.0, Label::width)
    }

    /// The width of a column right of the plot: its sub-columns side by side, or its title
    /// when that is wider.
    fn width(&self) -> f64 {
        self.block.max(self.title_width())
    }

    /// The room a column right of the plot takes there, its gap included; 0 for a row.
    pub(super) fn right_band(&self) -> f64 {
        match self.side {
            ColumnSide::Right => COLUMN_GAP + self.width(),
            ColumnSide::Above => 0.0,
        }
    }

    /// The room a row above the plot takes there, its gap included; 0 for a column.
    pub(super) fn row_band(&self) -> f64 {
        match self.side {
            ColumnSide::Above => ROW_GAP + self.block,
            ColumnSide::Right => 0.0,
        }
    }

    /// The band above the plot that the title of a column right of the plot needs: its ascent
    /// and its gap; 0 for a row or without a title.
    pub(super) fn title_band(&self) -> f64 {
        match (self.side, &self.title) {
            (ColumnSide::Right, Some(_)) => self.title_ascent + self.title_gap,
            (ColumnSide::Right | ColumnSide::Above, _) => 0.0,
        }
    }

    /// The width the title of a row above the plot takes in the y tick label column; 0 for a
    /// column or without a title.
    pub(super) fn row_title_width(&self) -> f64 {
        match self.side {
            ColumnSide::Above => self.title_width(),
            ColumnSide::Right => 0.0,
        }
    }

    /// The column placed for the plot `plot`: a column right of the plot in the room that
    /// starts `inside` right of the plot's right edge, after what lies inside it; a row above
    /// the plot over y tick labels that rise `label_rise` above it.
    pub(super) fn place(self, plot: Area, inside: f64, label_rise: f64) -> PlacedColumn {
        let mut cells = Vec::with_capacity(self.extents.len());
        let (start, title) = match self.side {
            ColumnSide::Right => {
                let left = plot.right() + inside + COLUMN_GAP;
                let width = self.width();
                let title = self.title.map(|mut title| {
                    title.x = left + (width - title.width()) / 2.0;
                    title.y = plot.y - label_rise - self.title_gap;
                    title
                });
                (left + (width - self.block) / 2.0, title)
            }
            ColumnSide::Above => {
                let top = plot.y - label_rise - ROW_GAP - self.block;
                let centre = top + self.block / 2.0;
                let title = self.title.map(|mut title| {
                    title.x = plot.x - Y_LABEL_GAP - title.width();
                    title.y = centre + (self.title_ascent - self.title_descent) / 2.0;
                    title
                });
                (top, title)
            }
        };
        let mut at = start;
        for extent in self.extents {
            cells.push((at, extent));
            at += extent + SUB_COLUMN_GAP;
        }
        PlacedColumn {
            side: self.side,
            cells,
            title,
        }
    }
}

/// A value column, placed.
#[derive(Debug, Clone)]
pub(crate) struct PlacedColumn {
    /// Where the column goes.
    pub side: ColumnSide,
    /// Each sub-column, in mark order, as where it starts and its extent across the category
    /// axis: its left edge and width right of the plot, its top edge and height above it.
    pub cells: Vec<(f64, f64)>,
    /// The title, placed; `None` without one.
    pub title: Option<Label>,
}
