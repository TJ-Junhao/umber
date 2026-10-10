//! Resolve: data extents to scales to ticks, computed at render time.
//!
//! Auto limits follow spec section 7. A continuous axis is widened to nice limits, except the x
//! axis of an axes whose marks are all lines, which keeps the exact data extent. Explicit
//! limits replace the auto range as given. A category axis lists the marks' categories in
//! order of first appearance. On a nice axis the ticks are the tick set that produced the
//! nice limits, so both ends carry a tick. Tick positions are normalized to `[0, 1]` along the axis, so
//! layout and drawing never touch data values again.

use crate::figure::{Axes, AxisData, Mark};
use crate::{CategoryScale, ContinuousScale, ScaleKind, Tick, Transform};

/// The context of errors that cannot happen here: the model validated every input already.
const CONTEXT: &str = "Figure.to_scene()";

/// The resolved scale of one axis.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AxisScale {
    /// A linear or log axis.
    Continuous(ContinuousScale),
    /// A category axis.
    Category(CategoryScale),
}

/// One tick: where it sits along the axis, in `[0, 1]`, and its label.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AxisTick {
    /// Position along the axis; 0 is the low end (left or bottom).
    pub pos: f64,
    /// The label text.
    pub label: String,
}

/// A resolved axis: its scale and its ticks.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedAxis {
    /// The scale.
    pub scale: AxisScale,
    /// The ticks, in increasing position.
    pub ticks: Vec<AxisTick>,
}

impl ResolvedAxis {
    /// Whether this is a category axis.
    pub fn is_category(&self) -> bool {
        matches!(self.scale, AxisScale::Category(_))
    }

    /// The continuous scale, if this is not a category axis.
    #[cfg(test)]
    pub fn continuous(&self) -> Option<&ContinuousScale> {
        match &self.scale {
            AxisScale::Continuous(scale) => Some(scale),
            AxisScale::Category(_) => None,
        }
    }
}

/// Both axes of an [`Axes`], resolved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Resolved {
    /// The x axis.
    pub x: ResolvedAxis,
    /// The y axis.
    pub y: ResolvedAxis,
    /// Where the baseline sits along the y axis, in `[0, 1]`: at y = 0 when 0 is inside the y
    /// range, otherwise at the bottom (0).
    pub baseline: f64,
}

/// Whether two normalized positions are the same, for matching ticks to the baseline.
pub(crate) fn coincides(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

/// The tick targets for a plot area of `plot_w` by `plot_h` points:
/// x `clamp(round(w / 100) + 1, 3, 8)`, y `clamp(round(h / 45) + 1, 3, 8)`.
///
/// These divisors were calibrated against the approved mocks (about 5 ticks per axis on a
/// 6 by 4 in figure: y 0/25/50/75/100, x 0/10/20/30). They replace the plan's original 72 and
/// 60 pt, which gave too few y ticks. The y divisor is 45 rather than 50 so that
/// `AboveGridline` layouts, whose label rows leave a shorter plot area (about 172 pt on a
/// 6 by 4 in PIGMENT figure), still get 5 y ticks.
pub(crate) fn tick_targets(plot_w: f64, plot_h: f64) -> (usize, usize) {
    fn target(size: f64, per_tick: f64, lo: usize, hi: usize) -> usize {
        let n = (size / per_tick).round() + 1.0;
        if n.is_nan() {
            return lo;
        }
        // The clamp below keeps the cast within range; NaN was handled above.
        (n.clamp(lo as f64, hi as f64)) as usize
    }
    (target(plot_w, 100.0, 3, 8), target(plot_h, 45.0, 3, 8))
}

fn mark_data(mark: &Mark) -> (&AxisData, &[f64]) {
    match mark {
        Mark::Line { x, y, .. } | Mark::Scatter { x, y, .. } => (x, y),
    }
}

/// The smallest and largest finite value, ignoring NaN; `None` when there is none.
fn extent<'a>(values: impl IntoIterator<Item = &'a f64>) -> Option<(f64, f64)> {
    values
        .into_iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold(None, |acc, v| match acc {
            None => Some((v, v)),
            Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
        })
}

fn transform_of(kind: ScaleKind) -> Option<Transform> {
    match kind {
        ScaleKind::Linear => Some(Transform::Linear),
        ScaleKind::Log { base } => Some(Transform::Log(base)),
        _ => None,
    }
}

/// A continuous axis from explicit limits, or from the data extent widened to nice limits
/// unless `exact`. Nice axes take the ticks that produced their limits; the others take
/// `ticks()`.
fn continuous(
    transform: Transform,
    limits: Option<(f64, f64)>,
    data: Option<(f64, f64)>,
    exact: bool,
    target: usize,
) -> ResolvedAxis {
    let (scale, ticks) = if let Some((lo, hi)) = limits {
        let scale = ContinuousScale::new(transform, lo, hi, CONTEXT)
            .unwrap_or_else(|_| ContinuousScale::unit(transform));
        (scale, scale.ticks(target))
    } else {
        let scale = ContinuousScale::from_extent(transform, data, CONTEXT)
            .unwrap_or_else(|_| ContinuousScale::unit(transform));
        if exact {
            (scale, scale.ticks(target))
        } else {
            scale.nice_with_ticks(target)
        }
    };
    ResolvedAxis {
        ticks: positioned(&scale, ticks),
        scale: AxisScale::Continuous(scale),
    }
}

/// Ticks with their positions along `scale`; ticks that do not map are dropped.
fn positioned(scale: &ContinuousScale, ticks: Vec<Tick>) -> Vec<AxisTick> {
    ticks
        .into_iter()
        .filter_map(|tick| {
            let pos = scale.normalize(tick.value);
            pos.is_finite().then_some(AxisTick {
                pos,
                label: tick.label,
            })
        })
        .collect()
}

fn category_ticks(scale: &CategoryScale) -> Vec<AxisTick> {
    scale
        .categories()
        .iter()
        .enumerate()
        .map(|(i, name)| AxisTick {
            pos: scale.center(i),
            label: name.clone(),
        })
        .collect()
}

/// The x categories of every mark, in mark order, then in data order.
fn categories(marks: &[Mark]) -> CategoryScale {
    CategoryScale::from_first_appearance(marks.iter().flat_map(|mark| match mark_data(mark).0 {
        AxisData::Categories(names) => names.iter().map(String::as_str).collect::<Vec<_>>(),
        AxisData::Numbers(_) => Vec::new(),
    }))
}

/// Resolves both axes of `axes` for about `x_target` and `y_target` ticks.
pub(crate) fn resolve(axes: &Axes, x_target: usize, y_target: usize) -> Resolved {
    let marks = axes.marks();
    let x = match transform_of(axes.x_scale()) {
        Some(transform) => {
            let data = extent(marks.iter().flat_map(|mark| match mark_data(mark).0 {
                AxisData::Numbers(values) => values.as_slice(),
                AxisData::Categories(_) => &[],
            }));
            let all_lines = marks.iter().all(|mark| matches!(mark, Mark::Line { .. }));
            continuous(transform, axes.x_limits(), data, all_lines, x_target)
        }
        None => {
            let scale = categories(marks);
            ResolvedAxis {
                ticks: category_ticks(&scale),
                scale: AxisScale::Category(scale),
            }
        }
    };
    let (y, baseline) = match transform_of(axes.y_scale()) {
        Some(transform) => {
            let data = extent(marks.iter().flat_map(|mark| mark_data(mark).1));
            let axis = continuous(transform, axes.y_limits(), data, false, y_target);
            let baseline = match &axis.scale {
                AxisScale::Continuous(scale) => {
                    let (lo, hi) = scale.domain();
                    if transform == Transform::Linear && lo <= 0.0 && 0.0 <= hi {
                        scale.normalize(0.0)
                    } else {
                        0.0
                    }
                }
                AxisScale::Category(_) => 0.0,
            };
            (axis, if baseline.is_finite() { baseline } else { 0.0 })
        }
        None => {
            // Marks always hold numeric y, so a category y axis never has categories.
            let scale = CategoryScale::from_first_appearance(std::iter::empty());
            let axis = ResolvedAxis {
                ticks: category_ticks(&scale),
                scale: AxisScale::Category(scale),
            };
            (axis, 0.0)
        }
    };
    Resolved { x, y, baseline }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{LineOptions, ScatterOptions};
    use crate::{Figure, LogBase, Theme};

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn cats(v: &[&str]) -> AxisData {
        AxisData::Categories(v.iter().map(|s| (*s).to_owned()).collect())
    }

    fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        figure.add_axes(x, y).unwrap();
        figure
    }

    fn axes(figure: &mut Figure) -> &mut Axes {
        figure.axes_mut().unwrap()
    }

    fn domains(figure: &Figure) -> ((f64, f64), (f64, f64)) {
        let r = resolve(figure.axes().unwrap(), 5, 4);
        (
            r.x.continuous().unwrap().domain(),
            r.y.continuous().unwrap().domain(),
        )
    }

    const LIN: ScaleKind = ScaleKind::Linear;

    #[test]
    fn all_line_marks_use_exact_x_extent() {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .line(
                nums(&[0.3, 1.0, 9.7]),
                &[1.0, 2.0, 3.0],
                LineOptions::default(),
            )
            .unwrap()
            .line(
                nums(&[f64::NAN, 2.0, 7.1]),
                &[0.5, 2.5, 3.3],
                LineOptions::default(),
            )
            .unwrap();
        let ((x0, x1), (y0, y1)) = domains(&f);
        assert_eq!((x0, x1), (0.3, 9.7));
        // y is still widened to nice limits.
        assert!(y0 <= 0.5 && y1 >= 3.3);
        assert_ne!((y0, y1), (0.5, 3.3));

        // Adding a scatter turns the exception off.
        axes(&mut f)
            .scatter(nums(&[0.3]), &[1.0], ScatterOptions::default())
            .unwrap();
        let ((x0, x1), _) = domains(&f);
        assert_eq!((x0, x1), (0.0, 10.0));
    }

    #[test]
    fn scatter_uses_nice_limits() {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(
                nums(&[12.0, 47.0, 88.0]),
                &[3.2, 18.0, 36.5],
                ScatterOptions::default(),
            )
            .unwrap();
        let ((x0, x1), (y0, y1)) = domains(&f);
        assert_eq!((x0, x1), (10.0, 90.0));
        assert_eq!((y0, y1), (0.0, 40.0));
        // Nice limits are the outer ticks on both axes: x is 10, 30, ..., 90, not 25, 50, 75.
        let r = resolve(f.axes().unwrap(), 5, 4);
        for axis in [&r.x, &r.y] {
            assert_eq!(axis.ticks.first().map(|t| t.pos), Some(0.0));
            assert_eq!(axis.ticks.last().map(|t| t.pos), Some(1.0));
        }
        let labels: Vec<&str> = r.x.ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["10", "30", "50", "70", "90"]);

        // The same on a log axis.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(log, LIN);
        axes(&mut f)
            .scatter(nums(&[12.0, 88.0]), &[1.0, 2.0], ScatterOptions::default())
            .unwrap();
        let r = resolve(f.axes().unwrap(), 5, 4);
        assert_eq!(r.x.continuous().unwrap().domain(), (10.0, 100.0));
        let labels: Vec<&str> = r.x.ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["10", "100"]);
        assert_eq!(r.x.ticks.first().map(|t| t.pos), Some(0.0));
        assert_eq!(r.x.ticks.last().map(|t| t.pos), Some(1.0));
    }

    #[test]
    fn explicit_limits_win() {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[12.0, 88.0]), &[3.2, 36.5], ScatterOptions::default())
            .unwrap()
            .xlim(0.5, 9.5)
            .unwrap()
            .ylim(-1.3, 2.7)
            .unwrap();
        let ((x0, x1), (y0, y1)) = domains(&f);
        assert_eq!((x0, x1), (0.5, 9.5));
        assert_eq!((y0, y1), (-1.3, 2.7));
        // Every tick lies inside the explicit limits.
        let r = resolve(f.axes().unwrap(), 5, 4);
        assert!(r.x.ticks.iter().all(|t| (0.0..=1.0).contains(&t.pos)));
        assert!(r.y.ticks.iter().all(|t| (0.0..=1.0).contains(&t.pos)));
        // 0 is inside -1.3..2.7, so the baseline sits at its normalized position.
        assert!((r.baseline - 1.3 / 4.0).abs() < 1e-12);
    }

    #[test]
    fn empty_axes_fall_back_to_unit_range() {
        let f = figure(LIN, LIN);
        assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));

        // All-NaN data counts as empty.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[f64::NAN]), &[f64::NAN], ScatterOptions::default())
            .unwrap();
        assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));

        let log = ScaleKind::Log { base: LogBase::TEN };
        let f = figure(log, log);
        assert_eq!(domains(&f), ((1.0, 10.0), (1.0, 10.0)));
        let r = resolve(f.axes().unwrap(), 5, 4);
        assert_eq!(r.baseline, 0.0);
    }

    #[test]
    fn log_axis_limits() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(LIN, log);
        axes(&mut f)
            .scatter(nums(&[1.0, 2.0]), &[3.0, 4.2e4], ScatterOptions::default())
            .unwrap();
        let (_, (y0, y1)) = domains(&f);
        assert_eq!((y0, y1), (1.0, 1e5));
        let r = resolve(f.axes().unwrap(), 5, 4);
        // 0 is never inside a log range: the baseline sits at the bottom.
        assert_eq!(r.baseline, 0.0);
        let labels: Vec<&str> = r.y.ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels.first(), Some(&"1"));
        assert!(r.y.ticks.windows(2).all(|w| w[0].pos < w[1].pos));

        // Explicit log limits are used as given.
        axes(&mut f).ylim(2.0, 500.0).unwrap();
        let (_, (y0, y1)) = domains(&f);
        assert_eq!((y0, y1), (2.0, 500.0));

        // A log x axis of lines keeps the exact extent.
        let mut f = figure(log, LIN);
        axes(&mut f)
            .line(nums(&[3.0, 700.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap();
        let ((x0, x1), _) = domains(&f);
        assert_eq!((x0, x1), (3.0, 700.0));
    }

    #[test]
    fn category_order_is_first_appearance_across_marks() {
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .line(
                cats(&["b", "a", "b"]),
                &[1.0, 2.0, 3.0],
                LineOptions::default(),
            )
            .unwrap()
            .scatter(
                cats(&["c", "a", "d"]),
                &[1.0, 2.0, 3.0],
                ScatterOptions::default(),
            )
            .unwrap();
        let r = resolve(f.axes().unwrap(), 5, 4);
        assert!(r.x.is_category());
        let labels: Vec<&str> = r.x.ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["b", "a", "c", "d"]);
        let pos: Vec<f64> = r.x.ticks.iter().map(|t| t.pos).collect();
        assert_eq!(pos, [0.125, 0.375, 0.625, 0.875]);
    }

    #[test]
    fn baseline_sits_at_zero_or_bottom() {
        let mut f = figure(LIN, LIN);
        axes(&mut f).ylim(-10.0, 30.0).unwrap();
        assert_eq!(resolve(f.axes().unwrap(), 5, 4).baseline, 0.25);
        axes(&mut f).ylim(5.0, 30.0).unwrap();
        assert_eq!(resolve(f.axes().unwrap(), 5, 4).baseline, 0.0);
        axes(&mut f).ylim(-30.0, -5.0).unwrap();
        assert_eq!(resolve(f.axes().unwrap(), 5, 4).baseline, 0.0);
    }

    #[test]
    fn tick_targets_follow_plot_size() {
        assert_eq!(tick_targets(0.0, 0.0), (3, 3));
        assert_eq!(tick_targets(1.0, 1.0), (3, 3));
        // 6 by 4 in plot areas: 408 / 100 + 1 = 5; PIGMENT 172 / 45 + 1 = 5, GRAPHITE 178 / 45 + 1 = 5.
        assert_eq!(tick_targets(408.0, 172.0), (5, 5));
        assert_eq!(tick_targets(408.0, 178.0), (5, 5));
        // Rounding: 3.5 rounds up, just below 3.5 rounds down (3.5 * 45 = 157.5).
        assert_eq!(tick_targets(350.0, 157.0), (5, 4));
        assert_eq!(tick_targets(349.0, 158.0), (4, 5));
        // Upper clamps.
        assert_eq!(tick_targets(10_000.0, 10_000.0), (8, 8));
        assert_eq!(tick_targets(f64::NAN, f64::INFINITY), (3, 8));
    }
}
