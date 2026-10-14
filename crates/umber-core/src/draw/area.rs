//! Area marks: the region between a line and the baseline, split where the line crosses the
//! baseline and painted as the theme's [`FillStyle`] says, under the line itself; and the
//! vertical fade gradient, which histogram bars share.

use super::MarkContext;
use super::clip::{clamp_polyline, crossing};
use super::line::{LineClip, plot_subpaths, stroke};
use super::mapping::PlotPoint;
use crate::geometry::Path;
use crate::scene::{FillRule, GradientStop, Item, LinearGradient, Paint, StrokeStyle};
use crate::{FillStyle, Rgba};

/// The opacity of an area fill, as a share of the line's own opacity: all over under
/// [`FillStyle::Solid`], at the line's extreme under [`FillStyle::Gradient`], from which the
/// gradient ramps down to 0 at the baseline.
pub(super) const AREA_ALPHA: f32 = 0.32;
/// Parts of an area less tall than this, in points, are not filled (the part is empty, and a
/// gradient across it would have no length).
const MIN_AREA_HEIGHT: f64 = 0.01;

/// The scene items of one area mark, its line given in axis space and stroked with `style`:
/// the fill of the region between the line and the baseline in the stroke's color (see
/// [`area_fills`]), then the line's stroke over it, exactly as a line mark with the same stroke
/// draws it.
///
/// The fill and the stroke are clipped separately. The stroke is the line's visible part. The
/// fill is bounded by the line clamped into the line's clip (see [`clamp_polyline`]), so the
/// region still shows where the line runs above or below the plot: a line far above a plot
/// whose y range holds the baseline fills the whole plot. Nothing is drawn when neither is
/// visible.
pub(super) fn area_items(
    context: &MarkContext<'_>,
    xs: &[f64],
    ys: &[f64],
    style: &StrokeStyle,
    items: &mut Vec<Item>,
) {
    let fill_style = context.theme.fill_style();
    let baseline = Baseline {
        y: context.baseline_y,
        value: context.maps.y.at(context.baseline),
    };
    let clip = LineClip::of(context, style.width);
    for piece in clamp_polyline(xs, ys, &clip.bounds) {
        let boundary: Vec<FillPoint> = piece
            .into_iter()
            .map(|p| FillPoint {
                at: clip.plot_point(p.at),
                value: p.at.y,
                line_value: p.line_y,
            })
            .collect();
        items.extend(area_fills(&boundary, baseline, style.color, fill_style));
    }
    let subpaths = plot_subpaths(context, style.width, xs, ys);
    if !subpaths.is_empty() {
        items.push(stroke(&subpaths, style));
    }
}

/// A point of a fill boundary: where it is drawn, and in axis space the y it is drawn at and
/// the y of the line itself, which lies beyond the clip where the line runs past the plot.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FillPoint {
    /// The plot point.
    at: PlotPoint,
    /// The axis-space y of `at`, clamped into the line's clip.
    value: f64,
    /// The line's own axis-space y there.
    line_value: f64,
}

/// The baseline, as laid out in plot points and as its axis-space value.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Baseline {
    /// The plot y.
    y: f64,
    /// The axis-space value.
    value: f64,
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

/// The area between one fill `boundary` and the baseline, split where the boundary crosses
/// the baseline. Each part is its own fill, so parts never cancel each other: a closed polygon
/// (the boundary, then down to the baseline and back).
///
/// Under [`FillStyle::Gradient`] a part is painted with the vertical ramp from the line's
/// extreme on its side of the baseline, at 0.32 of the line's opacity, to the baseline, at 0:
/// parts above the baseline fade from their top, parts below from their bottom. The gradient
/// runs from the part's drawn extreme to the baseline, so its ends stay inside the clip, and
/// each end has the ramp's opacity there (see [`ramp_share`]): where the line runs beyond the
/// plot, the drawn extreme is the clip edge, which gets the share of its distance to the
/// baseline, not the full 0.32. Under [`FillStyle::Solid`] a part is painted flat at 0.32 of the
/// line's opacity.
fn area_fills(
    boundary: &[FillPoint],
    baseline: Baseline,
    color: Rgba,
    style: FillStyle,
) -> Vec<Item> {
    let baseline_y = baseline.y;
    // Split into parts that each stay on one side of the baseline.
    let mut parts: Vec<(BaselineSide, Vec<FillPoint>)> = Vec::new();
    let mut points = boundary.iter().copied();
    let Some(first) = points.next() else {
        return Vec::new();
    };
    let mut current = (side(first.at.y, baseline_y), vec![first]);
    let mut prev = first.at;
    for p in points {
        let s = side(p.at.y, baseline_y);
        if current.0 == BaselineSide::On {
            current.0 = s;
        } else if s != BaselineSide::On && s != current.0 {
            // Where the line crosses the baseline: the crossing of the horizontal line
            // `y = baseline_y`, with the coordinates swapped.
            let x =
                crossing((prev.y, prev.x), (p.at.y, p.at.x), baseline_y, prev.x).unwrap_or(prev.x);
            let c = FillPoint {
                at: PlotPoint { x, y: baseline_y },
                value: baseline.value,
                line_value: baseline.value,
            };
            current.1.push(c);
            parts.push(std::mem::replace(&mut current, (s, vec![c])));
        }
        current.1.push(p);
        prev = p.at;
    }
    parts.push(current);

    parts
        .into_iter()
        .filter_map(|(side, part)| {
            let (Some(first), Some(last)) = (part.first().map(|p| p.at), part.last().map(|p| p.at))
            else {
                return None;
            };
            let ys = part.iter().map(|p| p.at.y);
            let values = part.iter().map(|p| (p.value, p.line_value));
            // The drawn extreme in plot points, and in axis space the drawn and the line's
            // extremes, away from the baseline.
            let (extreme, (drawn, line)) = match side {
                BaselineSide::Above => (
                    ys.fold(f64::INFINITY, f64::min),
                    values.fold((f64::NEG_INFINITY, f64::NEG_INFINITY), |(d, l), (v, w)| {
                        (d.max(v), l.max(w))
                    }),
                ),
                BaselineSide::Below => (
                    ys.fold(f64::NEG_INFINITY, f64::max),
                    values.fold((f64::INFINITY, f64::INFINITY), |(d, l), (v, w)| {
                        (d.min(v), l.min(w))
                    }),
                ),
                BaselineSide::On => return None,
            };
            if part.len() < 2 || (extreme - baseline_y).abs() < MIN_AREA_HEIGHT {
                return None;
            }
            let mut path = Path::new();
            path.move_to(first.to_scene());
            for p in part.iter().skip(1) {
                path.line_to(p.at.to_scene());
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
                        share: ramp_share(drawn, line, baseline.value),
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

/// The share of the line's opacity that an area's gradient ramp gives the axis-space value
/// `drawn`, for a ramp from `line`, the line's extreme on one side of the baseline, at 0.32, to
/// the baseline `base`, at 0: `0.32 × clamp((drawn − base) / (line − base), 0, 1)`. When the
/// line is drawn up to its extreme (`drawn == line`, the line stays inside the clip), the share
/// is exactly 0.32.
fn ramp_share(drawn: f64, line: f64, base: f64) -> f32 {
    if drawn == line {
        return AREA_ALPHA;
    }
    let ratio = (drawn - base) / (line - base);
    if ratio.is_nan() {
        return 0.0;
    }
    AREA_ALPHA * ratio.clamp(0.0, 1.0) as f32
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
