//! Drawing: turns a [`Layout`] into scene items.
//!
//! The axes chrome (gridlines, tick labels, the header and the axis labels) is one unclipped group,
//! drawn by [`chrome()`]. The marks follow in a second group, drawn in insertion order by
//! [`marks()`] and clipped to the plot area. When the axes hold marks drawn along a line (lines and
//! areas), that clip is widened horizontally on each side by half of the widest line stroke among
//! them, each line at its own drawn width, as far
//! as the room beside the plot area allows (see [`marks_clip`]), so the round end of a line that
//! starts or ends on the left or right edge stays whole; the top and bottom of the clip are the
//! plot area's. The baseline comes last, in an unclipped group of its own drawn by [`baseline()`],
//! so marks never cover it.
//!
//! Marks are clipped in *axis space*, before they are mapped to plot coordinates: the raw value
//! on a linear axis, `ln(value / lower limit)` on a log axis, the band center (in `0..=1`) on a
//! category axis.
//! Validated data is finite there, while mapping it to points first can overflow (data at 1e300
//! with limits 0 to 1e-10), and saturating an overflowed coordinate would change the line's
//! direction. Lines and the lines of areas are clipped against the axis-space domain expanded by
//! `max(own width, widest width, 2)` pt, where the widest width is that of the widest line among
//! the axes' marks, so every line reaches past the widened marks clip and that clip is its only
//! visible cut; scatter points against the domain expanded by each marker shape's extent (at the
//! scatter's own drawn size) plus its outline and 1 pt, bars against the y domain expanded by `max(corner radius, 2)` pt, and
//! histogram bins against both domains expanded by 2 pt (all converted to axis units). Only the
//! clipped or kept points are mapped to plot coordinates, so every mark coordinate in the scene
//! is within a few points of the plot area, and nothing is narrowed to `f32` before that.
//!
//! Each mark kind has its own module: [`line`](mod@line), [`area`], which fills under a line drawn
//! as [`line`](mod@line) draws it, [`scatter`] with the [`marker`] shapes, and [`bar`] and [`hist`]
//! with the [`bar_shape`] outlines they share. They share the axis maps of [`mapping`] and the
//! clipping of [`clip`], and receive their input as one [`MarkContext`].

mod area;
mod bar;
mod bar_shape;
mod chrome;
mod clip;
mod hist;
pub(crate) mod legend;
mod line;
mod mapping;
mod marker;
mod scatter;
mod stroke;
#[cfg(test)]
mod tests;

pub(crate) use chrome::{baseline, chrome};

use crate::Theme;
use crate::layout::{Area, Layout};
use crate::resolve::{NormalizedPos, Resolved, ResolvedMark};
use crate::scene::{Item, Rect};
use mapping::{AxisMaps, axis_x, axis_y};

/// What every mark implementation draws with: the theme, the axis maps and the plot geometry.
/// The mark's own data and resolved color are passed alongside.
#[derive(Debug, Clone, Copy)]
struct MarkContext<'a> {
    /// The theme.
    theme: &'a Theme,
    /// The x and y axis maps, from axis space to plot points.
    maps: AxisMaps,
    /// The plot area.
    plot: Area,
    /// Where the baseline sits along the y axis, as resolved; bars on a log y axis start there.
    baseline: NormalizedPos,
    /// The plot y of the baseline, as laid out; area fills end there.
    baseline_y: f64,
    /// The drawn stroke width of the widest line among the marks, areas included, in points; 0
    /// without one (see [`widest_line_width`]). Every line is clipped in axis space at least
    /// this far past the plot, since the marks clip is widened by half of it.
    widest_line: f32,
}

/// The `marks` as one group clipped to [`marks_clip`], in the given order, each in its resolved
/// color. `None` when nothing is visible.
pub(crate) fn marks(
    theme: &Theme,
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
) -> Option<Item> {
    let plot = layout.plot;
    let context = MarkContext {
        theme,
        maps: AxisMaps::new(&resolved.x, &resolved.y, plot),
        plot,
        baseline: resolved.baseline,
        baseline_y: layout.baseline_y,
        widest_line: widest_line_width(marks),
    };
    let mut items = Vec::new();
    for mark in marks {
        match *mark {
            ResolvedMark::Line {
                mark: line_mark,
                color,
                width,
            } => {
                let (xs, ys) = line::line_points(&context, resolved, line_mark);
                let style = line::line_style(width, color, line_mark.options().dash);
                line::line_items(&context, &xs, &ys, &style, &mut items);
            }
            ResolvedMark::Area {
                mark: area_mark,
                color,
                width,
            } => {
                let (xs, ys) = line::line_points(&context, resolved, area_mark.line());
                let style = line::line_style(width, color, area_mark.options().dash);
                area::area_items(&context, &xs, &ys, &style, &mut items);
            }
            ResolvedMark::Scatter {
                mark: scatter_mark,
                color,
                size,
            } => {
                let xs = axis_x(&resolved.x, scatter_mark.x());
                let ys = axis_y(&resolved.y, scatter_mark.y());
                let marker = scatter_mark.options().marker;
                items.extend(scatter::scatter_item(
                    &context, &xs, &ys, color, marker, size,
                ));
            }
            ResolvedMark::Bar {
                mark: bar_mark,
                color,
            } => {
                bar::bar_items(
                    &context,
                    &resolved.x,
                    &resolved.y,
                    bar_mark,
                    color,
                    &mut items,
                );
            }
            ResolvedMark::Grouped {
                mark: group_mark,
                ref series,
            } => {
                bar::grouped_items(
                    &context,
                    &resolved.x,
                    &resolved.y,
                    group_mark,
                    series,
                    &mut items,
                );
            }
            ResolvedMark::Stacked {
                mark: stack_mark,
                ref series,
                ref stacks,
            } => {
                bar::stacked_items(
                    &context,
                    &resolved.x,
                    &resolved.y,
                    stack_mark,
                    series,
                    stacks,
                    &mut items,
                );
            }
            ResolvedMark::Hist {
                ref histogram,
                color,
                ..
            } => {
                if let Some(histogram) = histogram {
                    hist::hist_items(
                        &context,
                        &resolved.x,
                        &resolved.y,
                        histogram,
                        color,
                        &mut items,
                    );
                }
            }
        }
    }
    if items.is_empty() {
        return None;
    }
    Some(Item::Group {
        clip: Some(marks_clip(layout, marks)),
        items,
    })
}

/// The clip rectangle of the marks: the plot area, widened horizontally on each side by half of
/// the widest stroke among the `marks` drawn along a line, each at its own drawn width
/// ([`ResolvedMark::line_width`]); the plot area itself when there is no such mark.
///
/// The widening never takes more than the free room beside the plot area
/// ([`Layout::room_left`], [`Layout::room_right`]): on the left it stops at the y tick labels
/// (or the frame's edge without them), on the right at the frame's edge. The frame is the
/// canvas for a figure of one axes; on a grid it ends halfway into the gap next to a
/// neighbouring cell, so a line's cap never reaches into another panel's cells, and at the
/// canvas edge on the grid's outer sides. The layout does not make room for the caps, so a
/// line wide enough for half its stroke to exceed the gap between the plot and the y tick
/// labels, or the room up to the frame's edge on the right, still has its cap cut there.
///
/// Every line is clipped in axis space against a margin of at least the widest stroke width
/// and its own (see the module documentation), so each line, thin ones next to a thick one
/// included, reaches past this rectangle and its edge is the only cut.
pub(super) fn marks_clip(layout: &Layout, marks: &[ResolvedMark<'_>]) -> Rect {
    let plot = layout.plot;
    let half = f64::from(widest_line_width(marks)) / 2.0;
    let (left, right) = (half.min(layout.room_left()), half.min(layout.room_right()));
    Rect {
        x: (plot.x - left) as f32,
        y: plot.y as f32,
        width: (plot.width + (left + right)) as f32,
        height: plot.height as f32,
    }
}

/// The drawn stroke width of the widest of the `marks` drawn along a line
/// ([`ResolvedMark::line_width`]), in points; 0 when there is no such mark. It widens the
/// marks clip ([`marks_clip`]) and sets the smallest axis-space clip margin of every line.
fn widest_line_width(marks: &[ResolvedMark<'_>]) -> f32 {
    marks
        .iter()
        .filter_map(ResolvedMark::line_width)
        .fold(0.0, f32::max)
}
