//! Area fills under lines: a gradient from the line to the baseline, split where the line
//! crosses the baseline.

use super::clip::{Pt, crossing};
use super::mapping::{point, scene_point};
use crate::Rgba;
use crate::geometry::Path;
use crate::scene::{FillRule, GradientStop, Item, LinearGradient, Paint};

/// The area gradient's opacity at the top of the line, as a share of the line's own opacity.
const AREA_TOP_ALPHA: f32 = 0.32;
/// Gradient vectors shorter than this give no area fill (the area is empty).
const MIN_AREA_HEIGHT: f64 = 0.01;

/// Which side of the baseline `y` lies on: -1 above (smaller y), 1 below, 0 on it.
fn side(y: f64, baseline_y: f64) -> i8 {
    if y < baseline_y {
        -1
    } else if y > baseline_y {
        1
    } else {
        0
    }
}

/// The area between one clipped line `subpath` and `baseline_y`, split where the line crosses
/// the baseline. Each part is its own fill, so parts never cancel each other: a closed polygon
/// (the line, then down to the baseline and back) painted with a vertical gradient from the
/// part's extreme y, at 0.32 of the line's opacity, to the baseline, at 0. Parts above the
/// baseline fade from their top, parts below from their bottom.
pub(super) fn area_fills(subpath: &[Pt], baseline_y: f64, color: Rgba) -> Vec<Item> {
    // Split into parts that each stay on one side of the baseline.
    let mut parts: Vec<(i8, Vec<Pt>)> = Vec::new();
    let mut points = subpath.iter().copied();
    let Some(first) = points.next() else {
        return Vec::new();
    };
    let mut current = (side(first.1, baseline_y), vec![first]);
    let mut prev = first;
    for p in points {
        let s = side(p.1, baseline_y);
        if current.0 == 0 {
            current.0 = s;
        } else if s != 0 && s != current.0 {
            // Where the line crosses the baseline.
            let x = crossing((prev.1, prev.0), (p.1, p.0), baseline_y, prev.0).unwrap_or(prev.0);
            let c = (x, baseline_y);
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
            let ys = part.iter().map(|p| p.1);
            let extreme = match side {
                -1 => ys.fold(f64::INFINITY, f64::min),
                1 => ys.fold(f64::NEG_INFINITY, f64::max),
                _ => return None,
            };
            if part.len() < 2 || (extreme - baseline_y).abs() < MIN_AREA_HEIGHT {
                return None;
            }
            let mut path = Path::new();
            path.move_to(scene_point(first));
            for &p in part.iter().skip(1) {
                path.line_to(scene_point(p));
            }
            path.line_to(point(last.0, baseline_y));
            path.line_to(point(first.0, baseline_y));
            path.close();
            Some(Item::Fill {
                path,
                paint: Paint::Linear(LinearGradient {
                    start: point(0.0, extreme),
                    end: point(0.0, baseline_y),
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
