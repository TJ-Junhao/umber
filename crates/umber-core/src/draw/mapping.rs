//! Axis maps: mark data to axis space, axis space to plot points, and plot points to scene
//! points.
//!
//! Marks live in two coordinate spaces. [`AxisPoint`] is axis space, where marks are clipped;
//! [`PlotPoint`] is plot points, where they are emitted. [`AxisMaps::to_plot`] is the one
//! conversion from the first to the second.

use crate::geometry::Point;
use crate::layout::Area;
use crate::model::{AxisData, AxisValue, SharedX};
use crate::resolve::{AxisScale, NormalizedPos, ResolvedAxis};
use crate::scale::log_offset;
use crate::{ContinuousScale, Instant, Transform};

/// A point in axis space: the raw value on a linear axis, `ln(value / lower limit)` on a log
/// axis, the band center on a category axis, the seconds from the axis's origin on a time axis.
/// Validated data is finite here, and NaN marks a gap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AxisPoint {
    /// The x axis value.
    pub(super) x: f64,
    /// The y axis value.
    pub(super) y: f64,
}

impl AxisPoint {
    /// The point `(x, y)` in axis space.
    pub(super) fn new(x: f64, y: f64) -> AxisPoint {
        AxisPoint { x, y }
    }
}

/// A point in plot points: the layout's coordinates, with the origin at the canvas top-left
/// and y pointing down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PlotPoint {
    /// Points from the left edge.
    pub(super) x: f64,
    /// Points from the top edge.
    pub(super) y: f64,
}

impl PlotPoint {
    /// The scene point at this position; see [`point`].
    pub(super) fn to_scene(self) -> Point {
        point(self.x, self.y)
    }
}

/// A scene point from layout coordinates. Layout keeps every value finite and far below the
/// renderers' `MAX_COORD`, so narrowing to `f32` is safe.
pub(super) fn point(x: f64, y: f64) -> Point {
    Point {
        x: x as f32,
        y: y as f32,
    }
}

/// The maps of both axes, from axis space to plot points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AxisMaps {
    /// The x axis map.
    pub(super) x: AxisMap,
    /// The y axis map.
    pub(super) y: AxisMap,
}

impl AxisMaps {
    /// The maps of the resolved axes onto `plot`: x from left to right, y from the bottom up.
    pub(super) fn new(x: &ResolvedAxis, y: &ResolvedAxis, plot: Area) -> AxisMaps {
        AxisMaps {
            x: AxisMap::new(x, plot.x, plot.width),
            y: AxisMap::new(y, plot.bottom(), -plot.height),
        }
    }

    /// The plot point of axis-space point `p`. For `p` within a few points of the axis
    /// domain (all callers pass clipped or culled points) this cannot overflow.
    pub(super) fn to_plot(self, p: AxisPoint) -> PlotPoint {
        PlotPoint {
            x: self.x.plot_at(p.x),
            y: self.y.plot_at(p.y),
        }
    }
}

/// The affine map of one axis from axis space to plot points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AxisMap {
    /// The axis-space value at the low end of the axis (left or bottom).
    pub(super) lo: f64,
    /// The axis-space value at the high end of the axis.
    pub(super) hi: f64,
    /// The plot coordinate of `lo`.
    pub(super) start: f64,
    /// The plot coordinate distance from `lo` to `hi`, at least 1 pt; negative for y, which
    /// points down.
    pub(super) extent: f64,
}

impl AxisMap {
    /// The map of `axis` onto the plot coordinates from `start`, at its low end, to
    /// `start + extent`, at its high end. A category axis spans `0..1`, the band centers'
    /// range, and a time axis its domain in seconds from its origin.
    fn new(axis: &ResolvedAxis, start: f64, extent: f64) -> AxisMap {
        let (lo, hi) = match &axis.scale {
            AxisScale::Continuous(scale) => continuous_domain(scale),
            AxisScale::Category(_) => (0.0, 1.0),
            AxisScale::Time(scale) => scale.seconds_scale().domain(),
        };
        AxisMap {
            lo,
            hi,
            start,
            extent,
        }
    }

    /// The plot coordinate of axis-space value `u`. For `u` within a few points of the axis
    /// domain this cannot overflow.
    pub(super) fn plot_at(&self, u: f64) -> f64 {
        self.start + (u - self.lo) / (self.hi - self.lo) * self.extent
    }

    /// The axis-space value at the normalized position `pos` along the axis, from `lo` at the
    /// low end to `hi` at the high end. Axis space is affine in normalized positions on every
    /// axis, so this is the value a resolved position such as the baseline stands for.
    pub(super) fn at(&self, pos: NormalizedPos) -> f64 {
        self.lo + pos.get() * (self.hi - self.lo)
    }

    /// The axis-space range covered by the axis extended by `margin` points at both ends. The
    /// rounding of `lo - m` and `hi + m` is monotonic, so the range always contains the domain.
    pub(super) fn expanded(&self, margin: f64) -> (f64, f64) {
        let m = self.length(margin);
        (self.lo - m, self.hi + m)
    }

    /// The axis-space length of `points` plot points along the axis; the map is affine, so it
    /// is the same everywhere on the axis.
    pub(super) fn length(&self, points: f64) -> f64 {
        points / self.extent.abs() * (self.hi - self.lo)
    }
}

/// The axis-space domain of the continuous `scale`: the limits on a linear axis, and
/// `0..ln(hi / lo)` on a log axis (with [`log_offset`], exactly as `ContinuousScale::normalize`
/// measures, so marks and gridlines agree).
fn continuous_domain(scale: &ContinuousScale) -> (f64, f64) {
    let (lo, hi) = scale.domain();
    match scale.transform() {
        Transform::Linear => (lo, hi),
        Transform::Log(_) => (0.0, log_offset(hi, lo)),
    }
}

/// The axis-space values of `data` on `axis`. Category names map to their band center and
/// times to their seconds from the axis's origin; NaN, missing times, and names the scale does
/// not know (impossible after resolve), map to NaN.
pub(super) fn axis_values(axis: &ResolvedAxis, data: &AxisData) -> Vec<f64> {
    match (&axis.scale, data) {
        (AxisScale::Continuous(scale), AxisData::Numbers(values)) => {
            continuous_values(scale, values)
        }
        (AxisScale::Category(scale), AxisData::Categories(names)) => names
            .iter()
            .map(|name| scale.index_of(name).map_or(f64::NAN, |i| scale.center(i)))
            .collect(),
        (AxisScale::Time(_), AxisData::Times(times)) => axis_times(axis, times),
        // The model rejects data of the wrong kind for the axis.
        (_, data) => vec![f64::NAN; data.len()],
    }
}

/// The axis-space values of the numbers `values` on `axis`, each as [`axis_value`] maps it.
pub(super) fn axis_numbers(axis: &ResolvedAxis, values: &[f64]) -> Vec<f64> {
    match &axis.scale {
        AxisScale::Continuous(scale) => continuous_values(scale, values),
        AxisScale::Category(_) | AxisScale::Time(_) => vec![f64::NAN; values.len()],
    }
}

/// The axis-space value of the number `v` on `axis`, as [`axis_values`] maps numbers; NaN on a
/// category or a time axis, which have no numbers.
pub(super) fn axis_value(axis: &ResolvedAxis, v: f64) -> f64 {
    match &axis.scale {
        AxisScale::Continuous(scale) => continuous_value(scale, v),
        AxisScale::Category(_) | AxisScale::Time(_) => f64::NAN,
    }
}

/// The axis-space value of the time `t` on `axis`: its seconds from the origin of a time axis;
/// NaN on any other axis, which has no times.
pub(super) fn axis_time(axis: &ResolvedAxis, t: Instant) -> f64 {
    match &axis.scale {
        AxisScale::Time(scale) => scale.seconds(t),
        AxisScale::Continuous(_) | AxisScale::Category(_) => f64::NAN,
    }
}

/// The axis-space values of the times `times` on `axis`, each as [`axis_time`] maps it; a
/// missing time maps to NaN, a gap.
pub(super) fn axis_times(axis: &ResolvedAxis, times: &[Option<Instant>]) -> Vec<f64> {
    times
        .iter()
        .map(|t| t.map_or(f64::NAN, |t| axis_time(axis, t)))
        .collect()
}

/// The axis-space values of the shared x values `x` of a band or a stacked area on `axis`.
pub(super) fn axis_shared(axis: &ResolvedAxis, x: &SharedX) -> Vec<f64> {
    match x {
        SharedX::Numbers(values) => values.iter().map(|&v| axis_value(axis, v)).collect(),
        SharedX::Times(times) => axis_times(axis, times),
    }
}

/// The axis-space position of the value `v` on `axis`: a number as [`axis_value`] maps it, a
/// time as [`axis_time`] does, and a category name at its band centre; NaN for a name the axis
/// does not have.
pub(super) fn axis_position(axis: &ResolvedAxis, v: &AxisValue) -> f64 {
    match v {
        AxisValue::Number(v) => axis_value(axis, *v),
        AxisValue::Time(t) => axis_time(axis, *t),
        AxisValue::Category(name) => match &axis.scale {
            AxisScale::Category(scale) => {
                scale.index_of(name).map_or(f64::NAN, |i| scale.center(i))
            }
            AxisScale::Continuous(_) | AxisScale::Time(_) => f64::NAN,
        },
    }
}

/// `values` in the axis space of `scale`, each as [`continuous_value`] maps it.
fn continuous_values(scale: &ContinuousScale, values: &[f64]) -> Vec<f64> {
    values.iter().map(|&v| continuous_value(scale, v)).collect()
}

/// `v` in the axis space of `scale`: the value on a linear axis, `ln(v / lo)` on a log axis.
/// Log values are positive (validated), so those are finite; anything else maps to NaN, a gap.
fn continuous_value(scale: &ContinuousScale, v: f64) -> f64 {
    match scale.transform() {
        Transform::Linear => v,
        Transform::Log(_) => {
            if v > 0.0 {
                log_offset(v, scale.domain().0)
            } else {
                f64::NAN
            }
        }
    }
}
