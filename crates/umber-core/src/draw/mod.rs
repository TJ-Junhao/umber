//! Drawing: turns a [`Layout`] into scene items.
//!
//! The axes chrome (gridlines, the baseline, tick labels, the header and the axis labels) is
//! one unclipped group, drawn by [`chrome()`]. The marks follow in a second group, clipped to the
//! plot area, drawn in insertion order by [`marks()`].
//!
//! Marks are clipped in *axis space*, before they are mapped to plot coordinates: the raw value
//! on a linear axis, `ln(value / lower limit)` on a log axis, the band center (in `0..=1`) on a
//! category axis.
//! Validated data is finite there, while mapping it to points first can overflow (data at 1e300
//! with limits 0 to 1e-10), and saturating an overflowed coordinate would change the line's
//! direction. Lines are clipped against the axis-space domain expanded by `max(line_width, 2)`
//! pt, scatter points against the domain expanded by each marker shape's extent plus its
//! outline and 1 pt (both converted to axis units). Only the clipped or kept points are mapped
//! to plot coordinates, so every mark coordinate in the scene is within a few points of the
//! plot area, and nothing is narrowed to `f32` before that.
//!
//! Each mark kind has its own module: [`line`](mod@line) with the [`area`] fills under lines,
//! and [`scatter`] with the [`marker`] shapes. They share the axis maps of [`mapping`] and the
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

pub(crate) use chrome::chrome;

use crate::Theme;
use crate::layout::{Area, Layout};
use crate::resolve::{Resolved, ResolvedMark};
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
    /// The y of the baseline, where area fills end.
    baseline_y: f64,
}

/// The `marks` as one group clipped to the plot area, in the given order, each in its resolved
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
        baseline_y: layout.baseline_y,
    };
    let mut items = Vec::new();
    for mark in marks {
        match *mark {
            ResolvedMark::Line {
                mark: line_mark,
                color,
            } => {
                let mut xs = axis_x(&resolved.x, line_mark.x());
                let mut ys = axis_y(&resolved.y, line_mark.y());
                if !resolved.x.is_category() {
                    (xs, ys) = line::downsample(&context, xs, ys);
                }
                line::line_items(
                    &context,
                    &xs,
                    &ys,
                    color,
                    line_mark.options().dash,
                    &mut items,
                );
            }
            ResolvedMark::Scatter {
                mark: scatter_mark,
                color,
            } => {
                let xs = axis_x(&resolved.x, scatter_mark.x());
                let ys = axis_y(&resolved.y, scatter_mark.y());
                let marker = scatter_mark.options().marker;
                items.extend(scatter::scatter_item(&context, &xs, &ys, color, marker));
            }
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
