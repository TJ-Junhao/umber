//! Area fills under lines: a gradient from the line to the baseline, split where the line
//! crosses the baseline.

use super::clip::crossing;
use super::mapping::PlotPoint;
use crate::Rgba;
use crate::geometry::Path;
use crate::scene::{FillRule, GradientStop, Item, LinearGradient, Paint};

/// The area gradient's opacity at the top of the line, as a share of the line's own opacity.
const AREA_TOP_ALPHA: f32 = 0.32;
/// Gradient vectors shorter than this give no area fill (the area is empty).
const MIN_AREA_HEIGHT: f64 = 0.01;

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

/// The area between one clipped line `subpath` and `baseline_y`, split where the line crosses
/// the baseline. Each part is its own fill, so parts never cancel each other: a closed polygon
/// (the line, then down to the baseline and back) painted with a vertical gradient from the
/// part's extreme y, at 0.32 of the line's opacity, to the baseline, at 0. Parts above the
/// baseline fade from their top, parts below from their bottom.
pub(super) fn area_fills(subpath: &[PlotPoint], baseline_y: f64, color: Rgba) -> Vec<Item> {
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

    let top_alpha = (f32::from(color.a) * AREA_TOP_ALPHA)
        .round()
        .clamp(0.0, 255.0) as u8;
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
            Some(Item::Fill {
                path,
                paint: Paint::Linear(LinearGradient {
                    start: PlotPoint { x: 0.0, y: extreme }.to_scene(),
                    end: at_baseline(0.0),
                    stops: vec![
                        GradientStop {
                            offset: 0.0,
                            color: Rgba {
                                a: top_alpha,
                                ..color
                            },
                        },
                        GradientStop {
                            offset: 1.0,
                            color: Rgba { a: 0, ..color },
                        },
                    ],
                }),
                rule: FillRule::NonZero,
            })
        })
        .collect()
}
