//! Scatter marks: one batch of markers, culled in axis space by the marker's true extent.

use super::MarkContext;
use super::clip::Bounds;
use super::fill::marker_alpha;
use super::mapping::AxisPoint;
use super::marker::{extent, marker_shape};
use crate::color_scale::ResolvedColorScale;
use crate::geometry::Point;
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle};
use crate::{Marker, Rgba, Theme};

/// The uniform-color scatter outline width, in points.
pub(crate) const MARKER_OUTLINE: f32 = 0.8;
/// The color-mapped scatter and bubble outline width, in points.
pub(super) const MAPPED_MARKER_OUTLINE: f32 = 0.5;
/// The Oklab distance at which a mapped outline becomes the background color.
const MAPPED_OUTLINE_DISTANCE: f64 = 0.25;
/// The Oklab lightness shift of a mapped outline away from the background.
const MAPPED_OUTLINE_LIGHTNESS: f64 = 0.25;
/// Scatter points whose marker (shape extent plus outline) lies further than this many points
/// outside the plot area are dropped.
const MARKER_SLACK: f64 = 1.0;

/// How far a marker of `shape` and nominal `diameter` reaches from its centre along x and along
/// y, outline included, in points: the shape's extent on each axis plus half the outline for a
/// circle, or the whole outline for a polygon, whose miter joins reach at most that far past a
/// vertex for these shapes (a triangle's 60° apex reaches exactly that far).
pub(crate) fn marker_reach(shape: Marker, diameter: f32) -> (f64, f64) {
    let (half_w, half_h) = extent(&marker_shape(shape, diameter));
    let outline = f64::from(MARKER_OUTLINE);
    let join = match shape {
        Marker::Circle => outline / 2.0,
        Marker::Square | Marker::Triangle | Marker::Diamond | Marker::Cross => outline,
    };
    (half_w + join, half_h + join)
}

/// The scene item of one scatter: one batch of `shape` markers of diameter `size` (the mark's
/// drawn size, see [`ResolvedMark::Scatter`](crate::resolve::ResolvedMark::Scatter)), filled
/// with `color` at the theme's marker alpha times the color's own, outlined 0.8 pt in the
/// background color. Points whose marker cannot reach the plot area are dropped.
pub(super) fn scatter_item(
    context: &MarkContext<'_>,
    xs: &[f64],
    ys: &[f64],
    color: Rgba,
    shape: Marker,
    size: f32,
) -> Option<Item> {
    let fill = color.scale_alpha(marker_alpha(context.theme));
    markers_item(context, (xs, ys), fill, shape, size, false)
}

/// The opaque outline color for a mapped point, computed from its fill before marker alpha.
/// In Oklab, let `d` be the Euclidean distance from `fill` to the background `bg`, and
/// `w = clamp(1 - d / T, 0, 1)`, where `T = MAPPED_OUTLINE_DISTANCE = 0.25`. Move the fill's
/// L by `MAPPED_OUTLINE_LIGHTNESS = 0.25`, down when the background's L is at least 0.5 and
/// up otherwise, leaving a and b unchanged, to get `ring`. Mix `(1 - w) * bg + w * ring`
/// in Oklab, then clamp to the sRGB gamut as colormaps do. Far fills use exactly `bg`.
/// Scatter and bubbles use this color with `MAPPED_MARKER_OUTLINE = 0.5` pt; their fills
/// and the colorbar are unchanged.
pub(super) fn mapped_outline_color(theme: &Theme, fill: Rgba) -> Rgba {
    let bg = crate::cmap::to_oklab(theme.background());
    let mut ring = crate::cmap::to_oklab(fill);
    let distance = ring
        .iter()
        .zip(bg)
        .map(|(fill, background)| (fill - background).powi(2))
        .sum::<f64>()
        .sqrt();
    let weight = (1.0 - distance / MAPPED_OUTLINE_DISTANCE).clamp(0.0, 1.0);
    if weight == 0.0 {
        return Rgba {
            a: 255,
            ..theme.background()
        };
    }
    ring[0] += if bg[0] >= 0.5 {
        -MAPPED_OUTLINE_LIGHTNESS
    } else {
        MAPPED_OUTLINE_LIGHTNESS
    };
    crate::cmap::from_oklab(std::array::from_fn(|i| {
        (1.0 - weight) * bg[i] + weight * ring[i]
    }))
}

/// The scene item of one batch of `shape` markers of diameter `size`, filled with `fill` as
/// given. Mapped fills get a 0.5 pt [`mapped_outline_color`]; uniform fills keep the 0.8 pt
/// background outline. Culling includes the outline's width.
pub(super) fn markers_item(
    context: &MarkContext<'_>,
    (xs, ys): (&[f64], &[f64]),
    fill: Rgba,
    shape: Marker,
    size: f32,
    mapped: bool,
) -> Option<Item> {
    let MarkContext { theme, maps, .. } = *context;
    let outline = if mapped {
        MAPPED_MARKER_OUTLINE
    } else {
        MARKER_OUTLINE
    };
    let shape_path = marker_shape(shape, size);
    // Cull in axis space by the shape's own extent on each axis, plus the outline (whose miter
    // joins reach at most its width beyond the path for these shapes) and the slack. The maps
    // may be in the frame of horizontal marks, as for the outliers of horizontal boxes, whose
    // first axis is the plot's y axis.
    let (half_w, half_h) = extent(&shape_path);
    let reach = f64::from(outline) + MARKER_SLACK;
    let (half_first, half_second) = maps.in_frame((half_w, half_h));
    let ((left, right), (top, bottom)) = (
        maps.x.expanded(half_first + reach),
        maps.y.expanded(half_second + reach),
    );
    let bounds = Bounds {
        left,
        top,
        right,
        bottom,
    };
    let positions: Vec<Point> = xs
        .iter()
        .zip(ys)
        .map(|(&x, &y)| AxisPoint::new(x, y))
        .filter(|&p| bounds.contains(p))
        .map(|p| maps.to_plot(p).to_scene())
        .collect();
    if positions.is_empty() {
        return None;
    }
    Some(Item::Markers {
        shape: shape_path,
        fill: Some(fill),
        stroke: Some(StrokeStyle {
            color: if mapped {
                mapped_outline_color(theme, fill)
            } else {
                theme.background()
            },
            width: outline,
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }),
        positions,
    })
}

/// The fill colors of the points of a mark: one color at the theme's marker alpha, or the
/// opaque color each point's value maps to on the axes' color scale, so a mapped point shows
/// exactly the colormap's color whatever lies under it.
#[derive(Debug, Clone, Copy)]
pub(super) enum PointFill<'a> {
    /// Every point in this color.
    Uniform(Rgba),
    /// Each point in the color its value maps to.
    Mapped {
        /// The axes' color scale.
        scale: &'a ResolvedColorScale,
        /// One value per point.
        values: &'a [f64],
    },
}

impl PointFill<'_> {
    /// The fill of the point at `index` under `theme`: the uniform color at the theme's marker
    /// alpha, or the mapped color, opaque. `None` for a point whose value maps to no color
    /// (NaN), which is not drawn.
    pub(super) fn at(self, theme: &Theme, index: usize) -> Option<Rgba> {
        match self {
            PointFill::Uniform(color) => Some(color.scale_alpha(marker_alpha(theme))),
            PointFill::Mapped { scale, values } => {
                values.get(index).and_then(|&value| scale.color(value))
            }
        }
    }
}

/// The scene items of one scatter whose points are filled by `fill`: uniform colors use
/// [`scatter_item`], mapped colors use a 0.5 pt [`mapped_outline_color`]. One batch per run of
/// consecutive points of one fill also shares one outline color, keeping data order so later
/// points still cover earlier ones. Points whose value maps to no color are not drawn.
pub(super) fn scatter_items(
    context: &MarkContext<'_>,
    (xs, ys): (&[f64], &[f64]),
    fill: PointFill<'_>,
    shape: Marker,
    size: f32,
    items: &mut Vec<Item>,
) {
    if let PointFill::Uniform(color) = fill {
        items.extend(scatter_item(context, xs, ys, color, shape, size));
        return;
    }
    let mut run: Option<(Rgba, Vec<f64>, Vec<f64>)> = None;
    for (index, (&x, &y)) in xs.iter().zip(ys).enumerate() {
        let Some(color) = fill.at(context.theme, index) else {
            continue;
        };
        match &mut run {
            Some((current, run_xs, run_ys)) if *current == color => {
                run_xs.push(x);
                run_ys.push(y);
            }
            _ => {
                if let Some((color, run_xs, run_ys)) = run.take() {
                    items.extend(markers_item(
                        context,
                        (&run_xs, &run_ys),
                        color,
                        shape,
                        size,
                        true,
                    ));
                }
                run = Some((color, vec![x], vec![y]));
            }
        }
    }
    if let Some((color, run_xs, run_ys)) = run {
        items.extend(markers_item(
            context,
            (&run_xs, &run_ys),
            color,
            shape,
            size,
            true,
        ));
    }
}
