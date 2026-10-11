//! Area marks: the region between a line and the baseline, split where the line crosses the
//! baseline and painted as the theme's [`FillStyle`] says, under the line itself; and the
//! vertical fade gradient, which histogram bars share.

use super::MarkContext;
use super::clip::crossing;
use super::line::{fill_boundaries, plot_subpaths, stroke};
use super::mapping::PlotPoint;
use crate::geometry::Path;
use crate::scene::{FillRule, GradientStop, Item, LinearGradient, Paint};
use crate::{Dash, FillStyle, Rgba};

/// The opacity of an area fill, as a share of the line's own opacity: all over under
/// [`FillStyle::Solid`], at the line's extreme under [`FillStyle::Gradient`].
const AREA_ALPHA: f32 = 0.32;
/// Parts of an area less tall than this, in points, are not filled (the part is empty, and a
/// gradient across it would have no length).
const MIN_AREA_HEIGHT: f64 = 0.01;

/// The scene items of one area mark, its line given in axis space: the fill of the region
/// between the line and the baseline (see [`area_fills`]), then the line's stroke over it,
/// exactly as a line mark of the same `color` and `dash` draws it.
///
/// The fill and the stroke are clipped separately. The stroke is the line's visible part. The
/// fill is bounded by the line clamped into the clip (see [`fill_boundaries`]), so the region
/// still shows where the line runs above or below the plot: a line far above a plot whose y
/// range holds the baseline fills the whole plot. Nothing is drawn when neither is visible.
pub(super) fn area_items(
    context: &MarkContext<'_>,
    xs: &[f64],
    ys: &[f64],
    color: Rgba,
    dash: Dash,
    items: &mut Vec<Item>,
) {
    let style = context.theme.fill_style();
    for boundary in &fill_boundaries(context, xs, ys) {
        items.extend(area_fills(boundary, context.baseline_y, color, style));
    }
    let subpaths = plot_subpaths(context, xs, ys);
    if !subpaths.is_empty() {
        items.push(stroke(context.theme, &subpaths, color, dash));
    }
}

/// Where a plot y lies relative to the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaselineSide {
    /// Above the baseline: a smaller y, since y points down.
    Above,
    /// On the baseline, or NaN.
    On,
    /// Below the baseline.
    Below,
}

/// Which side of the baseline `y` lies on.
fn side(y: f64, baseline_y: f64) -> BaselineSide {
    if y < baseline_y {
        BaselineSide::Above
    } else if y > baseline_y {
        BaselineSide::Below
    } else {
        BaselineSide::On
    }
}

/// The area between one fill boundary `subpath` and `baseline_y`, split where the boundary
/// crosses the baseline. Each part is its own fill, so parts never cancel each other: a closed
/// polygon (the boundary, then down to the baseline and back). Under [`FillStyle::Gradient`] it
/// is painted with a vertical gradient from the part's extreme y, at 0.32 of the line's
/// opacity, to the baseline, at 0: parts above the baseline fade from their top, parts below
/// from their bottom. The extreme is the boundary's, so where the line runs beyond the plot the
/// gradient starts at the clip edge, at most the line margin past the plot, exactly where a
/// clipped line part starts. Under [`FillStyle::Solid`] it is painted flat at 0.32 of the
/// line's opacity.
fn area_fills(subpath: &[PlotPoint], baseline_y: f64, color: Rgba, style: FillStyle) -> Vec<Item> {
    // Split into parts that each stay on one side of the baseline.
    let mut parts: Vec<(BaselineSide, Vec<PlotPoint>)> = Vec::new();
    let mut points = subpath.iter().copied();
    let Some(first) = points.next() else {
        return Vec::new();
    };
    let mut current = (side(first.y, baseline_y), vec![first]);
    let mut prev = first;
    for p in points {
        let s = side(p.y, baseline_y);
        if current.0 == BaselineSide::On {
            current.0 = s;
        } else if s != BaselineSide::On && s != current.0 {
            // Where the line crosses the baseline: the crossing of the horizontal line
            // `y = baseline_y`, with the coordinates swapped.
            let x = crossing((prev.y, prev.x), (p.y, p.x), baseline_y, prev.x).unwrap_or(prev.x);
            let c = PlotPoint { x, y: baseline_y };
            current.1.push(c);
            parts.push(std::mem::replace(&mut current, (s, vec![c])));
        }
        current.1.push(p);
        prev = p;
    }
    parts.push(current);

    parts
        .into_iter()
        .filter_map(|(side, part)| {
            let (Some(&first), Some(&last)) = (part.first(), part.last()) else {
                return None;
            };
            let ys = part.iter().map(|p| p.y);
            let extreme = match side {
                BaselineSide::Above => ys.fold(f64::INFINITY, f64::min),
                BaselineSide::Below => ys.fold(f64::NEG_INFINITY, f64::max),
                BaselineSide::On => return None,
            };
            if part.len() < 2 || (extreme - baseline_y).abs() < MIN_AREA_HEIGHT {
                return None;
            }
            let mut path = Path::new();
            path.move_to(first.to_scene());
            for &p in part.iter().skip(1) {
                path.line_to(p.to_scene());
            }
            let at_baseline = |x: f64| PlotPoint { x, y: baseline_y }.to_scene();
            path.line_to(at_baseline(last.x));
            path.line_to(at_baseline(first.x));
            path.close();
            let paint = match style {
                FillStyle::Solid => Paint::Solid(color.scale_alpha(AREA_ALPHA)),
                FillStyle::Gradient => vertical_fade(
                    color,
                    FadeStop {
                        y: extreme,
                        share: AREA_ALPHA,
                    },
                    FadeStop {
                        y: baseline_y,
                        share: 0.0,
                    },
                ),
            };
            Some(Item::Fill {
                path,
                paint,
                rule: FillRule::NonZero,
            })
        })
        .collect()
}

/// One end of a [`vertical_fade`]: a plot y and the share of the color's own opacity there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FadeStop {
    /// The plot y, in points.
    pub(super) y: f64,
    /// The share of the color's opacity, in `0..=1`.
    pub(super) share: f32,
}

/// A vertical gradient of `color` from `start` to `end`, the opacity at each end being its
/// share of the color's own (rounded by [`Rgba::scale_alpha`]). Beyond both ends the gradient
/// keeps its end colors. Areas fade from their extreme to the baseline; histograms from a point
/// on their own ramp to another.
pub(super) fn vertical_fade(color: Rgba, start: FadeStop, end: FadeStop) -> Paint {
    Paint::Linear(LinearGradient {
        start: PlotPoint { x: 0.0, y: start.y }.to_scene(),
        end: PlotPoint { x: 0.0, y: end.y }.to_scene(),
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: color.scale_alpha(start.share),
            },
            GradientStop {
                offset: 1.0,
                color: color.scale_alpha(end.share),
            },
        ],
    })
}
