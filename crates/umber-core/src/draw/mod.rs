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
//! clipping of [`clip`], and receive their input as one [`MarkContext`]. Bars, boxes and violins
//! on a category y axis are horizontal: they are drawn by the same code as on a category x axis,
//! in a context whose axis maps are in their frame ([`MarkContext::oriented`]).

mod annotation;
mod area;
mod band;
mod bar;
pub(crate) mod bar_labels;
mod bar_shape;
mod box_plot;
mod bubbles;
mod cells;
mod chrome;
mod clip;
pub(crate) mod colorbar;
mod contour;
mod contrast;
mod ecdf;
mod error_bars;
pub(crate) mod fill;
mod heatmap;
mod hexbin;
mod hist;
mod kde;
pub(crate) mod legend;
mod line;
mod mapping;
mod marker;
mod mesh;
pub(crate) mod pie;
mod rule;
mod scatter;
pub(crate) use scatter::{MARKER_OUTLINE, marker_reach};
mod events;
mod stacked_area;
mod stem;
mod step;
mod strip;
mod stroke;
#[cfg(test)]
mod tests;
mod violin;
pub(crate) use strip::check as check_points;

pub(crate) use annotation::items as annotations;
pub(crate) use chrome::{baseline, chrome, figure_header};
pub(crate) use heatmap::annotations as heatmap_annotations;

use crate::Theme;
use crate::layout::{Area, Layout};
use crate::resolve::{Baseline, Orientation, Resolved, ResolvedMark};
use crate::scene::{Item, Rect};
use crate::text::SceneFonts;
use mapping::{AxisMaps, axis_values};

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
    /// The baseline, as resolved, on the value axis; bars on a log value axis start there. The
    /// marks that read it grow along y, except horizontal bars, which are drawn in their frame
    /// ([`MarkContext::oriented`]), whose second axis is the value axis x.
    baseline: Baseline,
    /// The plot coordinate of the baseline along its axis, as laid out; area fills end there.
    baseline_at: f64,
    /// The drawn stroke width of the widest line among the marks, areas included, in points; 0
    /// without one (see [`widest_line_width`]). Every line is clipped in axis space at least
    /// this far past the plot, since the marks clip is widened by half of it.
    widest_line: f32,
}

impl MarkContext<'_> {
    /// This context for marks of `orientation`, its axis maps in their frame
    /// ([`AxisMaps::oriented`]): unchanged for vertical marks.
    fn oriented(&self, orientation: Orientation) -> Self {
        MarkContext {
            maps: self.maps.oriented(orientation),
            ..*self
        }
    }
}

/// The context that `marks`, resolved as `resolved` on `layout`, are drawn in, in the plot's own
/// frame.
fn mark_context<'a>(
    theme: &'a Theme,
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
) -> MarkContext<'a> {
    let plot = layout.plot;
    MarkContext {
        theme,
        maps: AxisMaps::new(&resolved.x, &resolved.y, plot),
        plot,
        baseline: resolved.baseline,
        baseline_at: layout.baseline.at,
        widest_line: widest_line_width(marks),
    }
}

/// The `marks` as one group clipped to [`marks_clip`], in the given order, each in its resolved
/// color, with the text they carry (the level labels of contours) shaped with the scene's
/// `fonts`. `Ok(None)` when nothing is visible.
///
/// # Errors
///
/// A level label that cannot be laid out fails as `fonts` reports it.
pub(crate) fn marks(
    theme: &Theme,
    fonts: &SceneFonts<'_>,
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
) -> crate::Result<Option<Item>> {
    let context = mark_context(theme, resolved, layout, marks);
    let mut items = Vec::new();
    // Reference spans lie under the data marks and reference lines over them, whatever the order
    // of the calls; the data marks keep theirs.
    let clip = marks_clip(layout, marks);
    let layer_clip = rule::baseline_clip(&context, resolved, clip);
    rule::span_layer(&context, resolved, marks, layer_clip, &mut items);
    for mark in marks {
        match *mark {
            ResolvedMark::Line {
                mark: line_mark,
                color,
                width,
                ..
            } => {
                let (xs, ys) = line::line_points(&context, resolved, line_mark);
                let style = line::line_style(width, color, line_mark.options().dash);
                line::line_items(&context, &xs, &ys, &style, &mut items);
            }
            ResolvedMark::Area {
                mark: area_mark,
                color,
                width,
                ..
            } => {
                let (xs, ys) = line::line_points(&context, resolved, area_mark.line());
                let style = line::line_style(width, color, area_mark.options().dash);
                area::area_items(&context, &xs, &ys, &style, &mut items);
            }
            ResolvedMark::Scatter {
                mark: scatter_mark,
                color,
                size,
                ref mapped,
                ..
            } => {
                let xs = axis_values(&resolved.x, scatter_mark.x());
                let ys = axis_values(&resolved.y, scatter_mark.y());
                let marker = scatter_mark.options().marker;
                if let Some(fill) = point_fill(resolved.color.as_ref(), color, mapped.as_ref()) {
                    scatter::scatter_items(&context, (&xs, &ys), fill, marker, size, &mut items);
                }
            }
            ResolvedMark::Bar {
                mark: bar_mark,
                color,
                orientation,
                ..
            } => {
                let (along, values) = resolved.along_and_values(orientation);
                bar::bar_items(
                    &context.oriented(orientation),
                    along,
                    values,
                    bar_mark,
                    color,
                    &mut items,
                );
            }
            ResolvedMark::Grouped {
                mark: group_mark,
                ref series,
                orientation,
                ..
            } => {
                let (along, values) = resolved.along_and_values(orientation);
                bar::grouped_items(
                    &context.oriented(orientation),
                    along,
                    values,
                    group_mark,
                    series,
                    &mut items,
                );
            }
            ResolvedMark::Stacked {
                mark: stack_mark,
                ref series,
                ref stacks,
                orientation,
                ..
            } => {
                let (along, values) = resolved.along_and_values(orientation);
                bar::stacked_items(
                    &context.oriented(orientation),
                    along,
                    values,
                    stack_mark,
                    series,
                    stacks,
                    &mut items,
                );
            }
            ResolvedMark::StackedArea {
                mark,
                ref series,
                ref stacks,
                width,
                ..
            } => {
                stacked_area::stacked_area_items(
                    &context, resolved, mark, series, stacks, width, &mut items,
                );
            }
            ResolvedMark::Mesh { mark, .. } => {
                if let Some(color) = &resolved.color {
                    mesh::mesh_items(&context, resolved, mark, color, &mut items);
                }
            }
            ResolvedMark::Hist2d { mark, .. } => {
                if let Some(color) = &resolved.color {
                    mesh::hist2d_items(&context, resolved, mark, color, &mut items);
                }
            }
            ResolvedMark::Hexbin { ref bins, .. } => {
                if let Some(color) = &resolved.color {
                    hexbin::hexbin_items(&context, resolved, bins, color, &mut items);
                }
            }
            ResolvedMark::Hist {
                ref bins,
                ref layers,
                style,
                width,
                ..
            } => {
                if let Some(bins) = bins {
                    hist::hist_items(
                        &context,
                        (&resolved.x, &resolved.y),
                        bins,
                        layers,
                        (style, width),
                        &mut items,
                    );
                }
            }
            ResolvedMark::Step {
                mark: step_mark,
                color,
                width,
                ..
            } => {
                step::step_items(&context, resolved, step_mark, (color, width), &mut items);
            }
            ResolvedMark::Band {
                mark: band_mark,
                color,
                ..
            } => {
                band::band_items(&context, resolved, band_mark, color, &mut items);
            }
            ResolvedMark::ErrorBars {
                mark: bars_mark,
                color,
                width,
                ..
            } => {
                error_bars::error_bar_items(
                    &context,
                    resolved,
                    bars_mark,
                    (color, width),
                    &mut items,
                );
            }
            ResolvedMark::Bubbles {
                mark,
                color,
                maximum,
                radius,
                ref mapped,
                ..
            } => {
                if let Some(fill) = point_fill(resolved.color.as_ref(), color, mapped.as_ref()) {
                    bubbles::bubble_items(
                        &context,
                        resolved,
                        mark,
                        (fill, maximum, radius),
                        &mut items,
                    );
                }
            }
            ResolvedMark::Stem {
                mark: stem_mark,
                color,
                size,
                ..
            } => {
                stem::stem_items(&context, resolved, stem_mark, (color, size), &mut items);
            }
            ResolvedMark::Box {
                categories,
                ref distributions,
                width,
                orientation,
                ..
            } => {
                box_plot::box_items(
                    &context.oriented(orientation),
                    resolved.along_and_values(orientation),
                    categories,
                    distributions,
                    width,
                    &mut items,
                );
            }
            ResolvedMark::Violin {
                categories,
                ref distributions,
                width,
                orientation,
                ..
            } => {
                violin::violin_items(
                    &context.oriented(orientation),
                    resolved.along_and_values(orientation),
                    categories,
                    distributions,
                    width,
                    &mut items,
                );
            }
            ResolvedMark::Heatmap {
                mark: heatmap_mark, ..
            } => {
                heatmap::heatmap_items(&context, resolved, heatmap_mark, &mut items);
            }
            ResolvedMark::Contour {
                mark,
                ref mapped,
                color,
                ref labels,
                ref shapes,
                ..
            } => {
                let contour = contour::Contour {
                    mark,
                    mapped: mapped.is_some(),
                    color,
                    labels,
                    shapes,
                };
                contour::contour_items(&context, fonts, resolved, contour, &mut items)?;
            }
            // Drawn in the layers below and above the data marks.
            ResolvedMark::HLine { .. }
            | ResolvedMark::VLine { .. }
            | ResolvedMark::HSpan { .. }
            | ResolvedMark::VSpan { .. } => {}
            ResolvedMark::Events {
                mark, color, width, ..
            } => {
                events::items(&context, resolved, mark, (color, width), &mut items);
            }
            ResolvedMark::Points {
                categories,
                groups,
                color,
                size,
                orientation,
                placement,
                ..
            } => {
                strip::items(
                    &context.oriented(orientation),
                    resolved.along_and_values(orientation),
                    categories,
                    groups,
                    (color, size, placement),
                    &mut items,
                );
            }
            ResolvedMark::Ecdf {
                mark: ecdf_mark,
                color,
                width,
                ..
            } => {
                ecdf::ecdf_items(&context, resolved, ecdf_mark, (color, width), &mut items);
            }
            ResolvedMark::Kde {
                mark: kde_mark,
                color,
                width,
                ..
            } => {
                kde::kde_items(&context, resolved, kde_mark, (color, width), &mut items);
            }
        }
    }
    rule::rule_layer(&context, resolved, marks, layer_clip, &mut items);
    if items.is_empty() {
        return Ok(None);
    }
    Ok(Some(Item::Group {
        clip: Some(clip),
        items,
    }))
}

/// Checks that every mark of `marks` that draws something and whose options draw its cells as
/// one image ([`Render::Raster`](crate::Render::Raster)) can be drawn so on `layout`:
/// `Error::Style` under `context` naming the first mark, by its place among the axes' marks,
/// and the axis along which explicit limits cut its cells too far from their edges, or along
/// which an edge would lie more than 0.001 pt from its place in the image (see
/// [`mesh::unplaceable_image`]).
pub(crate) fn check_images(
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
    context: &'static str,
) -> crate::Result<()> {
    let maps = AxisMaps::new(&resolved.x, &resolved.y, layout.plot);
    let Some((index, kind, axis, why)) = mesh::unplaceable_image(&maps, resolved, marks) else {
        return Ok(());
    };
    let mark = index + 1;
    let error = match why {
        mesh::Unplaceable::Reach(reach) => crate::Error::style(
            context,
            format!(
                "mark {mark} of these axes, {kind}, has a cell cut by the {axis} limits that \
                 reaches {} pt past them; drawing the cells as one image needs every cut cell to \
                 end within {} pt of the limits.",
                crate::error::number(reach.round()),
                cells::MAX_IMAGE_REACH,
            ),
        )
        .with_hint("use render=ub.Render.VECTOR or ub.AUTO, or widen the limits."),
        mesh::Unplaceable::Edge { index, offset } => {
            let edge = if kind == "a mesh" {
                format!("{axis}_edges[{index}]")
            } else {
                format!("{axis} bin edge {index}")
            };
            crate::Error::style(
                context,
                format!(
                    "mark {mark} of these axes, {kind}, has {edge} {offset:.3} pt from its place \
                     in the image of the cells; drawing the cells as one image needs every edge \
                     within {} pt of its place.",
                    cells::MAX_EDGE_OFFSET,
                ),
            )
            .with_hint("use render=ub.Render.VECTOR or ub.AUTO.")
        }
    };
    Err(error)
}

/// How the points of a scatter or bubble mark in `color` are filled: in that color, or, when
/// the mark maps its colors from values (`mapped`), in the colors of the axes' color scale
/// `scale`. `None` for a color-mapped mark without a color scale (no finite mapped value),
/// which draws nothing.
fn point_fill<'a, 'm: 'a>(
    scale: Option<&'a crate::color_scale::ResolvedColorScale>,
    color: crate::Rgba,
    mapped: Option<&'a crate::color_scale::MappedColors<'m>>,
) -> Option<scatter::PointFill<'a>> {
    match (mapped, scale) {
        (None, _) => Some(scatter::PointFill::Uniform(color)),
        (Some(mapped), Some(scale)) => Some(scatter::PointFill::Mapped {
            scale,
            values: mapped.values,
        }),
        (Some(_), None) => None,
    }
}

/// Whether the point at `index` of a scatter or bubble mark in `color`, its colors mapped from
/// values when `mapped` is set, has a fill on the axes' color scale `scale`, as drawing it
/// needs ([`point_fill`] and the fill of each point): every point of one color does, a mapped
/// point only when its value maps to a color. Points without a fill are not drawn.
pub(crate) fn has_fill(
    theme: &Theme,
    scale: Option<&crate::color_scale::ResolvedColorScale>,
    color: crate::Rgba,
    mapped: Option<&crate::color_scale::MappedColors<'_>>,
    index: usize,
) -> bool {
    point_fill(scale, color, mapped).is_some_and(|fill| fill.at(theme, index).is_some())
}

/// The outline width, in points, of the markers of a scatter or bubble mark: the contrasting
/// mapped outline when its colors are mapped from values (`mapped`), else the background
/// outline of one color.
pub(crate) fn marker_outline(mapped: bool) -> f32 {
    if mapped {
        scatter::MAPPED_MARKER_OUTLINE
    } else {
        MARKER_OUTLINE
    }
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
