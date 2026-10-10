//! Resolve: data extents to scales to ticks, computed at render time.
//!
//! Auto limits: a continuous axis is widened to nice limits, except the x axis of an axes whose
//! marks are all lines, which keeps the exact data extent. Explicit limits replace the auto
//! range as given. A category axis lists the marks' categories in order of first appearance. On
//! a nice axis the ticks are the tick set that produced the nice limits, so both ends carry a
//! tick. Tick positions are normalized to `[0, 1]` along the axis, so
//! layout and drawing never touch data values again.
//!
//! Marks are resolved here too, once per render: each mark becomes a [`ResolvedMark`], a typed
//! record that carries the mark's data and its [`ColorSpec`] resolved against the theme's
//! palette. Axes are resolved from these records, once per layout pass, and drawing reads them
//! instead of the marks.

use crate::model::{Axes, AxisData, LineMark, Mark, ScatterMark};
use crate::{
    CategoryScale, ColorSpec, ContinuousScale, Dash, Marker, Rgba, ScaleKind, Tick, Transform,
};

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

/// A position along an axis as a share of its length: 0 is the low end (left or bottom), 1 the
/// high end. Ticks and the baseline lie in `[0, 1]`; the type does not enforce that, so a
/// position can also describe a point outside the axis.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct NormalizedPos(f64);

impl NormalizedPos {
    /// The low end of the axis.
    pub const LOW: NormalizedPos = NormalizedPos(0.0);

    /// The position `share` of the axis length from the low end. `share` is stored as given:
    /// neither its range nor its finiteness is checked, so positions extrapolated beyond the
    /// axis ends and NaN pass through unchanged.
    pub fn new(share: f64) -> NormalizedPos {
        NormalizedPos(share)
    }

    /// The share of the axis length from the low end.
    pub fn get(self) -> f64 {
        self.0
    }
}

/// One tick: where it sits along the axis, in `[0, 1]`, and its label.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AxisTick {
    /// Position along the axis.
    pub pos: NormalizedPos,
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
    pub baseline: NormalizedPos,
}

/// Whether two normalized positions are the same, for matching ticks to the baseline.
pub(crate) fn coincides(a: NormalizedPos, b: NormalizedPos) -> bool {
    (a.0 - b.0).abs() <= 1e-9
}

/// How a continuous axis without explicit limits fits its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoLimits {
    /// The limits are the data extent.
    Exact,
    /// The data extent is widened to nice limits, which carry a tick at both ends.
    Nice,
}

/// What an axis fixes before any data is seen: whether it lists categories, and for a
/// continuous axis its transform and its explicit limits, if set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AxisLimits {
    /// A category axis: its categories come from the marks.
    Category,
    /// A linear or log axis.
    Continuous {
        /// The transform of the axis.
        transform: Transform,
        /// The scale of the explicit limits; `None` to fit the data.
        explicit: Option<ContinuousScale>,
    },
}

impl AxisLimits {
    /// The limits of an axis of scale `kind` with the explicit limits `explicit`.
    fn new(kind: ScaleKind, explicit: Option<ContinuousScale>) -> AxisLimits {
        match kind.transform() {
            Some(transform) => AxisLimits::Continuous {
                transform,
                explicit,
            },
            None => AxisLimits::Category,
        }
    }
}

/// The limits of both axes of an [`Axes`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Limits {
    /// The x axis.
    pub x: AxisLimits,
    /// The y axis.
    pub y: AxisLimits,
}

impl Limits {
    /// The scale kinds and explicit limits of `axes`.
    pub fn of(axes: &Axes) -> Limits {
        Limits {
            x: AxisLimits::new(axes.x_scale(), axes.x_limit_scale()),
            y: AxisLimits::new(axes.y_scale(), axes.y_limit_scale()),
        }
    }
}

/// The smallest and largest finite value, ignoring NaN; `None` when there is none.
fn extent(values: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    values
        .into_iter()
        .filter(|v| v.is_finite())
        .fold(None, |acc, v| match acc {
            None => Some((v, v)),
            Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
        })
}

/// A continuous axis: the scale of the explicit limits when set; otherwise the empty-data
/// range (0 to 1, or 1 to 10 on log) when there is no finite data; otherwise the data extent,
/// fitted as `auto` says. Nice axes take the ticks that produced their limits; the others take
/// `ticks()`.
fn continuous(
    transform: Transform,
    limits: Option<ContinuousScale>,
    data: Option<(f64, f64)>,
    auto: AutoLimits,
    target: usize,
) -> ResolvedAxis {
    let (scale, ticks) = if let Some(scale) = limits {
        (scale, scale.ticks(target))
    } else if data.is_none() {
        // The fallback range is final on every axis: never widened with `nice`.
        let scale = ContinuousScale::unit(transform);
        (scale, scale.ticks(target))
    } else {
        let scale = ContinuousScale::from_extent(transform, data, CONTEXT)
            .unwrap_or_else(|_| ContinuousScale::unit(transform));
        match auto {
            AutoLimits::Exact => (scale, scale.ticks(target)),
            AutoLimits::Nice => scale.nice_with_ticks(target),
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
                pos: NormalizedPos::new(pos),
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
            pos: NormalizedPos::new(scale.center(i)),
            label: name.clone(),
        })
        .collect()
}

/// The x categories of every mark, in mark order, then in data order.
fn categories(marks: &[ResolvedMark<'_>]) -> CategoryScale {
    CategoryScale::from_first_appearance(marks.iter().flat_map(|mark| mark.x_categories()))
}

/// Resolves both axes from the resolved `marks` and the axes' `limits`, for about `x_target`
/// and `y_target` ticks.
pub(crate) fn resolve(
    marks: &[ResolvedMark<'_>],
    limits: &Limits,
    x_target: usize,
    y_target: usize,
) -> Resolved {
    let x = match limits.x {
        AxisLimits::Continuous {
            transform,
            explicit,
        } => {
            let data = extent(marks.iter().flat_map(|mark| mark.x_numbers()));
            let all_lines = marks
                .iter()
                .all(|mark| matches!(mark, ResolvedMark::Line { .. }));
            let auto = if all_lines {
                AutoLimits::Exact
            } else {
                AutoLimits::Nice
            };
            continuous(transform, explicit, data, auto, x_target)
        }
        AxisLimits::Category => {
            let scale = categories(marks);
            ResolvedAxis {
                ticks: category_ticks(&scale),
                scale: AxisScale::Category(scale),
            }
        }
    };
    let (y, baseline) = match limits.y {
        AxisLimits::Continuous {
            transform,
            explicit,
        } => {
            let data = extent(marks.iter().flat_map(|mark| mark.y_values()));
            let axis = continuous(transform, explicit, data, AutoLimits::Nice, y_target);
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
            let baseline = if baseline.is_finite() { baseline } else { 0.0 };
            (axis, NormalizedPos::new(baseline))
        }
        AxisLimits::Category => {
            // Marks always hold numeric y, so a category y axis never has categories.
            let scale = CategoryScale::from_first_appearance(std::iter::empty());
            let axis = ResolvedAxis {
                ticks: category_ticks(&scale),
                scale: AxisScale::Category(scale),
            };
            (axis, NormalizedPos::LOW)
        }
    };
    Resolved { x, y, baseline }
}

/// A mark resolved for drawing: one variant per mark kind, each holding the mark and its color
/// resolved against the theme's palette. Axes, the legend and drawing read marks through these
/// records only.
///
/// A record may own data computed from its mark, such as bins or stacks, so the type is not
/// `Copy`, and its accessors borrow from the record rather than from the mark.
///
/// The axis-domain accessors ([`x_numbers`](Self::x_numbers),
/// [`x_categories`](Self::x_categories) and [`y_values`](Self::y_values)) each yield what the
/// mark contributes to an axis, in a fixed order, borrowed from the record. A mark with raw data
/// yields its data in data order. A mark with computed data yields values computed from what
/// the record owns, such as the ends of its stacked extent or the range of its bin counts, never
/// a copy of raw data shaped to fit. Non-finite numbers are allowed; the extent ignores them.
/// Axis resolution scans the marks in order, and each mark's values in the order given, so that
/// order is part of the output.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedMark<'a> {
    /// A line mark.
    Line {
        /// The mark.
        mark: &'a LineMark,
        /// The line color.
        color: Rgba,
    },
    /// A scatter mark.
    Scatter {
        /// The mark.
        mark: &'a ScatterMark,
        /// The marker color.
        color: Rgba,
    },
}

impl<'a> ResolvedMark<'a> {
    /// The values the mark contributes to a continuous x domain, in data order; none when its
    /// x values are categories. See [`ResolvedMark`] for the contract.
    pub fn x_numbers(&self) -> impl Iterator<Item = f64> + '_ {
        match self {
            ResolvedMark::Line { mark, .. } => numbers(mark.x()),
            ResolvedMark::Scatter { mark, .. } => numbers(mark.x()),
        }
        .iter()
        .copied()
    }

    /// The categories the mark contributes to a category x axis, in data order, repeats
    /// included; none when its x values are numbers. See [`ResolvedMark`] for the contract.
    pub fn x_categories(&self) -> impl Iterator<Item = &str> + '_ {
        match self {
            ResolvedMark::Line { mark, .. } => category_names(mark.x()),
            ResolvedMark::Scatter { mark, .. } => category_names(mark.x()),
        }
        .iter()
        .map(String::as_str)
    }

    /// The values the mark contributes to the y domain, in data order. See [`ResolvedMark`]
    /// for the contract.
    pub fn y_values(&self) -> impl Iterator<Item = f64> + '_ {
        match self {
            ResolvedMark::Line { mark, .. } => mark.y(),
            ResolvedMark::Scatter { mark, .. } => mark.y(),
        }
        .iter()
        .copied()
    }

    /// The mark's resolved color.
    #[cfg(test)]
    pub fn color(&self) -> Rgba {
        match *self {
            ResolvedMark::Line { color, .. } | ResolvedMark::Scatter { color, .. } => color,
        }
    }

    /// The series the mark shows, in order, one key each: what a legend lists for the mark.
    #[allow(dead_code, reason = "the legend will read it")]
    pub fn keys(&self) -> Vec<SeriesKey<'a>> {
        match *self {
            ResolvedMark::Line { mark, color } => {
                let options = mark.options();
                vec![SeriesKey {
                    label: &options.label,
                    color,
                    kind: SeriesKind::Line { dash: options.dash },
                }]
            }
            ResolvedMark::Scatter { mark, color } => {
                let options = mark.options();
                vec![SeriesKey {
                    label: &options.label,
                    color,
                    kind: SeriesKind::Points {
                        marker: options.marker,
                    },
                }]
            }
        }
    }
}

/// The numbers of `data`; empty for categories.
fn numbers(data: &AxisData) -> &[f64] {
    match data {
        AxisData::Numbers(values) => values,
        AxisData::Categories(_) => &[],
    }
}

/// The categories of `data`; empty for numbers.
fn category_names(data: &AxisData) -> &[String] {
    match data {
        AxisData::Categories(names) => names,
        AxisData::Numbers(_) => &[],
    }
}

/// One series as a legend shows it: its label, its resolved color and how it is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SeriesKey<'a> {
    /// The label; empty for an unlabelled series.
    pub label: &'a str,
    /// The resolved color.
    pub color: Rgba,
    /// How the series is drawn.
    pub kind: SeriesKind,
}

/// How a series is drawn, as far as its legend swatch needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeriesKind {
    /// A line with its dash style.
    Line {
        /// The dash style.
        dash: Dash,
    },
    /// Individual markers of one shape.
    Points {
        /// The marker shape.
        marker: Marker,
    },
    /// Filled bars.
    #[allow(dead_code, reason = "bar marks will produce it")]
    Bar,
}

/// Each mark resolved for drawing, in mark order, with its color resolved against `palette`.
/// `Auto` colors take palette slots in series order; `Palette` and `Fixed` colors do not
/// consume a slot.
pub(crate) fn resolve_marks<'a>(marks: &'a [Mark], palette: &[Rgba]) -> Vec<ResolvedMark<'a>> {
    let mut next_auto = 0_usize;
    marks
        .iter()
        .map(|mark| {
            let spec = mark.color();
            let color = spec.resolve(palette, next_auto);
            if matches!(spec, ColorSpec::Auto(_)) {
                next_auto += 1;
            }
            match mark {
                Mark::Line(mark) => ResolvedMark::Line { mark, color },
                Mark::Scatter(mark) => ResolvedMark::Scatter { mark, color },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Figure, LineOptions, ScatterOptions};
    use crate::{LogBase, ScaleKind, Theme};

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

    /// Both axes of `axes`, resolved from its marks.
    fn resolve_axes(axes: &Axes, x_target: usize, y_target: usize) -> Resolved {
        let marks = resolve_marks(axes.marks(), &[]);
        resolve(&marks, &Limits::of(axes), x_target, y_target)
    }

    fn domains(figure: &Figure) -> ((f64, f64), (f64, f64)) {
        let r = resolve_axes(figure.axes().unwrap(), 5, 4);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        for axis in [&r.x, &r.y] {
            assert_eq!(axis.ticks.first().map(|t| t.pos.get()), Some(0.0));
            assert_eq!(axis.ticks.last().map(|t| t.pos.get()), Some(1.0));
        }
        let labels: Vec<&str> = r.x.ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["10", "30", "50", "70", "90"]);

        // The same on a log axis.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(log, LIN);
        axes(&mut f)
            .scatter(nums(&[12.0, 88.0]), &[1.0, 2.0], ScatterOptions::default())
            .unwrap();
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(r.x.continuous().unwrap().domain(), (10.0, 100.0));
        let labels: Vec<&str> = r.x.ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["10", "100"]);
        assert_eq!(r.x.ticks.first().map(|t| t.pos.get()), Some(0.0));
        assert_eq!(r.x.ticks.last().map(|t| t.pos.get()), Some(1.0));
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert!(r.x.ticks.iter().all(|t| (0.0..=1.0).contains(&t.pos.get())));
        assert!(r.y.ticks.iter().all(|t| (0.0..=1.0).contains(&t.pos.get())));
        // 0 is inside -1.3..2.7, so the baseline sits at its normalized position.
        assert!((r.baseline.get() - 1.3 / 4.0).abs() < 1e-12);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(r.baseline.get(), 0.0);

        // Empty scatter data is empty data too.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[]), &[], ScatterOptions::default())
            .unwrap();
        assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));
    }

    #[test]
    fn empty_log_axes_keep_the_fallback_range() {
        // The fallback is not widened with `nice`, so both axes agree whatever the base.
        for base in [2.0, 100.0] {
            let log = ScaleKind::Log {
                base: LogBase::new(base).unwrap(),
            };
            let f = figure(log, log);
            assert_eq!(domains(&f), ((1.0, 10.0), (1.0, 10.0)), "base {base}");

            // All-NaN data, on a line and on a scatter.
            let mut f = figure(log, log);
            axes(&mut f)
                .line(nums(&[f64::NAN]), &[f64::NAN], LineOptions::default())
                .unwrap();
            assert_eq!(domains(&f), ((1.0, 10.0), (1.0, 10.0)), "base {base}");
            axes(&mut f)
                .scatter(
                    nums(&[f64::NAN, 2.0]),
                    &[f64::NAN, f64::NAN],
                    ScatterOptions::default(),
                )
                .unwrap();
            let ((_, _), (y0, y1)) = domains(&f);
            assert_eq!((y0, y1), (1.0, 10.0), "base {base}");

            // Empty scatter.
            let mut f = figure(log, log);
            axes(&mut f)
                .scatter(nums(&[]), &[], ScatterOptions::default())
                .unwrap();
            assert_eq!(domains(&f), ((1.0, 10.0), (1.0, 10.0)), "base {base}");
        }
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        // 0 is never inside a log range: the baseline sits at the bottom.
        assert_eq!(r.baseline.get(), 0.0);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert!(r.x.is_category());
        let labels: Vec<&str> = r.x.ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["b", "a", "c", "d"]);
        let pos: Vec<f64> = r.x.ticks.iter().map(|t| t.pos.get()).collect();
        assert_eq!(pos, [0.125, 0.375, 0.625, 0.875]);
    }

    #[test]
    fn resolved_marks_pair_each_mark_with_its_color_in_order() {
        use crate::{Alpha, Rgba};
        let mut f = figure(LIN, LIN);
        let palette = f.theme().palette().to_vec();
        let fixed = Rgba::opaque(9, 8, 7);
        let half = Alpha::new(0.5).unwrap();
        let specs = [
            ColorSpec::Palette {
                index: 3,
                alpha: Alpha::OPAQUE,
            },
            ColorSpec::AUTO,
            ColorSpec::Fixed(fixed),
            ColorSpec::AUTO.with_alpha(half),
            ColorSpec::Palette {
                index: 0,
                alpha: half,
            },
            ColorSpec::AUTO,
        ];
        let a = axes(&mut f);
        for (i, color) in specs.into_iter().enumerate() {
            // Lines and scatters alternate; both take Auto slots from the same sequence.
            if i % 2 == 0 {
                let options = ScatterOptions {
                    color,
                    ..ScatterOptions::default()
                };
                a.scatter(nums(&[i as f64]), &[1.0], options).unwrap();
            } else {
                let options = LineOptions {
                    color,
                    ..LineOptions::default()
                };
                a.line(nums(&[i as f64]), &[1.0], options).unwrap();
            }
        }
        let marks = f.axes().unwrap().marks();
        let resolved = resolve_marks(marks, &palette);
        assert_eq!(resolved.len(), marks.len());
        for (record, mark) in resolved.iter().zip(marks) {
            let same = match (record, mark) {
                (ResolvedMark::Line { mark: record, .. }, Mark::Line(mark)) => {
                    std::ptr::eq(*record, mark)
                }
                (ResolvedMark::Scatter { mark: record, .. }, Mark::Scatter(mark)) => {
                    std::ptr::eq(*record, mark)
                }
                _ => false,
            };
            assert!(same, "{record:?} does not hold {mark:?}");
        }
        let colors: Vec<Rgba> = resolved.iter().map(ResolvedMark::color).collect();
        assert_eq!(
            colors,
            [
                palette[3],
                palette[0],
                fixed,
                palette[1].with_alpha(half),
                palette[0].with_alpha(half),
                palette[2],
            ]
        );
        assert!(resolve_marks(&[], &palette).is_empty());
    }

    #[test]
    fn keys_carry_label_color_and_drawing() {
        use crate::{Dash, Marker};
        let mut f = figure(LIN, LIN);
        let palette = f.theme().palette().to_vec();
        axes(&mut f)
            .line(
                nums(&[0.0]),
                &[1.0],
                LineOptions {
                    label: "trend".to_owned(),
                    dash: Dash::Dashed,
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .scatter(
                nums(&[0.0]),
                &[1.0],
                ScatterOptions {
                    marker: Marker::Square,
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        let resolved = resolve_marks(f.axes().unwrap().marks(), &palette);
        let keys: Vec<SeriesKey<'_>> = resolved.iter().flat_map(ResolvedMark::keys).collect();
        assert_eq!(
            keys,
            [
                SeriesKey {
                    label: "trend",
                    color: palette[0],
                    kind: SeriesKind::Line { dash: Dash::Dashed },
                },
                SeriesKey {
                    label: "",
                    color: palette[1],
                    kind: SeriesKind::Points {
                        marker: Marker::Square,
                    },
                },
            ]
        );
    }

    #[test]
    fn baseline_sits_at_zero_or_bottom() {
        let mut f = figure(LIN, LIN);
        axes(&mut f).ylim(-10.0, 30.0).unwrap();
        assert_eq!(resolve_axes(f.axes().unwrap(), 5, 4).baseline.get(), 0.25);
        axes(&mut f).ylim(5.0, 30.0).unwrap();
        assert_eq!(resolve_axes(f.axes().unwrap(), 5, 4).baseline.get(), 0.0);
        axes(&mut f).ylim(-30.0, -5.0).unwrap();
        assert_eq!(resolve_axes(f.axes().unwrap(), 5, 4).baseline.get(), 0.0);
    }
}
