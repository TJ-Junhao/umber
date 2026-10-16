//! Color scales: the one color mapping of an axes. Its color-mapped marks fix a
//! [`ColorScaleSpec`] (colormap, scale and center) and give the values; the explicit color
//! limits or the extent of the finite values give the range. Resolve turns them into a
//! [`ResolvedColorScale`]: the colormap, the [`Norm`] that maps a value to a position on it,
//! and the ticks of the range.
//!
//! The model checks every color-mapped mark against the spec of the marks before it and
//! against the color limits, so all marks of an axes agree; resolve reads the first spec.

use crate::error::number;
use crate::model::{CmapChoice, ColorBy};
use crate::resolve::{AxisTick, NormalizedPos, ResolvedMark};
use crate::scale::{log_offset, too_narrow};
use crate::ticks::{self, TickValues, format_given};
use crate::{
    Cmap, ContinuousScale, Error, LogBase, MAX_MAGNITUDE, MIN_LOG_VALUE, Result, Rgba, Theme, Tick,
    TickFormat, Transform,
};

/// The context of errors that cannot happen here: the model validated every input already.
const CONTEXT: &str = "Figure.to_scene()";

/// The number of ticks a color range aims for.
const TICK_TARGET: usize = 5;

/// The hint of a color-mapped mark that disagrees with the color scale of its axes.
const SAME_SCALE_HINT: &str = "use the same cmap, scale and center for every color-mapped mark \
                               of an axes.";

/// How the color-mapped marks of an axes map values to colors: the colormap, the scale and
/// the center. Every color-mapped mark of an axes has the same.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColorScaleSpec {
    /// The colormap.
    pub cmap: CmapChoice,
    /// Linear or log.
    pub scale: Transform,
    /// The value at the middle of a diverging colormap; `None` for a sequential mapping.
    pub center: Option<f64>,
}

impl ColorScaleSpec {
    /// The spec of the color mapping `by`.
    pub fn of(by: &ColorBy) -> ColorScaleSpec {
        ColorScaleSpec {
            cmap: by.cmap.clone(),
            scale: by.scale,
            center: by.center,
        }
    }

    /// Checks that this spec, of a new mark, is the spec `existing` of the axes' color scale:
    /// `Error::Usage` under `context` naming the first of colormap, scale and center that
    /// differs, about `parameter`.
    pub fn check_same(
        &self,
        existing: &ColorScaleSpec,
        context: &'static str,
        parameter: &'static str,
    ) -> Result<()> {
        let Some(differs) = self.first_difference(existing) else {
            return Ok(());
        };
        Err(Error::usage(
            context,
            format!(
                "{parameter} uses {}; this axes already maps colors with {}.",
                self.describe(differs),
                existing.describe(differs)
            ),
        )
        .with_parameter(parameter)
        .with_hint(SAME_SCALE_HINT))
    }

    /// Checks a shared member's mapping, naming the panel that owns the conflicting mapping.
    pub fn check_shared(
        &self,
        existing: &ColorScaleSpec,
        cells: crate::CellRange,
        context: &'static str,
        parameter: &'static str,
    ) -> Result<()> {
        let Some(differs) = self.first_difference(existing) else {
            return Ok(());
        };
        Err(Error::usage(
            context,
            format!(
                "{parameter} uses {}; row {}, col {} maps colors with {}.",
                self.describe(differs),
                cells.row(),
                cells.col(),
                existing.describe(differs),
            ),
        )
        .with_parameter(parameter)
        .with_hint(
            "use the same cmap, scale and center for every color-mapped mark in the shared group.",
        ))
    }

    /// Checks that this spec, of a new mark that takes its colormap, scale and center as the
    /// arguments `cmap`, `scale` and `center`, is the spec `existing` of the axes' color
    /// scale: `Error::Usage` under `context` about the first of them that differs.
    pub fn check_same_arguments(
        &self,
        existing: &ColorScaleSpec,
        context: &'static str,
    ) -> Result<()> {
        let Some(differs) = self.first_difference(existing) else {
            return Ok(());
        };
        // The argument is named by itself: `center is 0`, not `center is center 0`.
        let given = match (differs, self.center) {
            (Part::Center, Some(center)) => number(center).to_string(),
            (Part::Center, None) => "not given".to_owned(),
            (part, _) => self.describe(part),
        };
        Err(Error::usage(
            context,
            format!(
                "{} is {given}; this axes already maps colors with {}.",
                differs.argument(),
                existing.describe(differs)
            ),
        )
        .with_parameter(differs.argument())
        .with_hint(SAME_SCALE_HINT))
    }

    /// The first of colormap, scale and center in which this spec differs from `existing`;
    /// `None` when the specs are the same.
    fn first_difference(&self, existing: &ColorScaleSpec) -> Option<Part> {
        if self.cmap != existing.cmap {
            Some(Part::Cmap)
        } else if self.scale != existing.scale {
            Some(Part::Scale)
        } else if self.center != existing.center {
            Some(Part::Center)
        } else {
            None
        }
    }

    /// The `part` of this spec as messages write it: `Cmap.VIRIDIS`, `Scale.log(10)`,
    /// `center 0` or `no center`.
    fn describe(&self, part: Part) -> String {
        match part {
            Part::Cmap => cmap_name(&self.cmap),
            Part::Scale => scale_name(self.scale),
            Part::Center => center_name(self.center),
        }
    }

    /// Checks the color limits `(low, high)`, which [`check_limits`] accepted, against this
    /// spec: positive (at least 1e-300) on a log scale, and holding the center. `Error::Style`
    /// under `context` about `parameter`, or about `low` or `high` when `parameter` is `None`
    /// (the limits are the arguments of the call).
    pub fn check_range(
        &self,
        (low, high): (f64, f64),
        context: &'static str,
        parameter: Option<&'static str>,
    ) -> Result<()> {
        let blame =
            |error: Error, side: &'static str| error.with_parameter(parameter.unwrap_or(side));
        if matches!(self.scale, Transform::Log(_)) && low < MIN_LOG_VALUE {
            let need = if low <= 0.0 {
                "positive limits".to_owned()
            } else {
                format!("limits of at least {MIN_LOG_VALUE:e}")
            };
            return Err(blame(
                Error::style(
                    context,
                    format!(
                        "the color limits are {} and {}; a log color scale needs {need}.",
                        number(low),
                        number(high)
                    ),
                ),
                "low",
            )
            .with_hint("set positive color limits with clim(), or use a linear scale."));
        }
        if let Some(center) = self.center
            && !(low..=high).contains(&center)
        {
            return Err(blame(
                Error::style(
                    context,
                    format!(
                        "center is {}; it must lie inside the color range {} to {}.",
                        number(center),
                        number(low),
                        number(high)
                    ),
                ),
                if center < low { "low" } else { "high" },
            )
            .with_hint("set color limits that hold center, or choose another center."));
        }
        Ok(())
    }

    /// The colormap this spec resolves to under `theme`.
    fn cmap<'a>(&'a self, theme: &'a Theme) -> &'a Cmap {
        match &self.cmap {
            CmapChoice::Fixed(cmap) => cmap,
            CmapChoice::Theme if self.center.is_some() => theme.diverging_cmap(),
            CmapChoice::Theme => theme.cmap(),
        }
    }
}

/// `choice` as the argument that selects it, for messages.
fn cmap_name(choice: &CmapChoice) -> String {
    match choice {
        CmapChoice::Theme => "the theme's colormap".to_owned(),
        CmapChoice::Fixed(cmap) => cmap.name(),
    }
}

/// `scale` as the Python `Scale` call that makes it.
fn scale_name(scale: Transform) -> String {
    match scale {
        Transform::Linear => "Scale.linear()".to_owned(),
        Transform::Log(base) => format!("Scale.log({})", number(base.get())),
    }
}

/// `center` for messages.
fn center_name(center: Option<f64>) -> String {
    match center {
        Some(center) => format!("center {}", number(center)),
        None => "no center".to_owned(),
    }
}

/// One of the three parts of a [`ColorScaleSpec`], each set by the argument of its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// The colormap.
    Cmap,
    /// The scale.
    Scale,
    /// The center.
    Center,
}

impl Part {
    /// The argument that sets this part.
    fn argument(self) -> &'static str {
        match self {
            Part::Cmap => "cmap",
            Part::Scale => "scale",
            Part::Center => "center",
        }
    }
}

/// Checks the explicit color limits `(low, high)` of the setter `context`, before any spec is
/// known: `Error::Style` unless both are finite, at most 1e300 in magnitude, `low < high`, and
/// the range is at least `max(1e-10 * max(|low|, |high|), 1e-290)` wide.
pub(crate) fn check_limits(context: &'static str, (low, high): (f64, f64)) -> Result<()> {
    for (name, value) in [("low", low), ("high", high)] {
        if !value.is_finite() || value.abs() > MAX_MAGNITUDE {
            return Err(Error::style(
                context,
                format!(
                    "{name} is {}; color limits must be finite and at most {MAX_MAGNITUDE:e} \
                     in magnitude.",
                    number(value)
                ),
            )
            .with_parameter(name));
        }
    }
    if low == high {
        return Err(Error::style(
            context,
            format!(
                "low and high are both {}; low must be below high.",
                number(low)
            ),
        )
        .with_parameter("high")
        .with_hint("pass two different values."));
    }
    if low > high {
        return Err(Error::style(
            context,
            format!(
                "low is {}, high is {}; low must be below high.",
                number(low),
                number(high)
            ),
        )
        .with_parameter("low")
        .with_hint("swap the two values."));
    }
    if too_narrow(low, high) {
        return Err(Error::style(
            context,
            format!(
                "the color limits {} and {} are too close; the range must be at least 1e-10 of \
                 their magnitude (and at least 1e-290).",
                number(low),
                number(high)
            ),
        )
        .with_parameter("high"));
    }
    Ok(())
}

/// How values map to positions on a colormap, `0..=1`. Positions outside the range clamp to
/// its ends. A range of one value maps every value to the middle.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Norm {
    /// Linearly from `low` to `high`.
    Linear {
        /// The value at 0.
        low: f64,
        /// The value at 1.
        high: f64,
    },
    /// By the logarithm from `low` to `high`, both positive.
    Log {
        /// The value at 0.
        low: f64,
        /// The value at 1.
        high: f64,
        /// The base the ticks are powers of.
        base: LogBase,
    },
    /// Linearly over `center ± radius`, with `center` at 0.5. The position is computed from
    /// the center and the radius, never from the ends, which can round onto the center when
    /// the radius is tiny next to it.
    Centered {
        /// The value at 0.5.
        center: f64,
        /// The distance from the center to either end; 0 maps every value to 0.5.
        radius: f64,
    },
}

impl Norm {
    /// The norm of `spec` over the range `(low, high)` (`low <= high`): symmetric about the
    /// center of a centered spec, with the larger distance from the center to an end on both
    /// sides. Only a range whose ends are equal is one value; a narrow range keeps its ends, so
    /// close values still get distinct colors.
    fn new(spec: &ColorScaleSpec, (low, high): (f64, f64)) -> Norm {
        match (spec.center, spec.scale) {
            (Some(center), _) => {
                let radius = (center - low).abs().max((high - center).abs());
                Norm::Centered { center, radius }
            }
            (None, Transform::Linear) => Norm::Linear { low, high },
            (None, Transform::Log(base)) => Norm::Log { low, high, base },
        }
    }

    /// The range, low end first.
    fn range(&self) -> (f64, f64) {
        match *self {
            Norm::Linear { low, high } | Norm::Log { low, high, .. } => (low, high),
            Norm::Centered { center, radius } => (center - radius, center + radius),
        }
    }

    /// The position of `v` on the colormap, clamped to `0..=1`: 0.5 for every value when the
    /// range is one value. `None` for NaN, and for a value that is not positive on a log norm.
    pub(crate) fn unit(&self, v: f64) -> Option<f64> {
        if v.is_nan() {
            return None;
        }
        let (low, high) = self.range();
        let share = match *self {
            Norm::Centered { radius: 0.0, .. } => 0.5,
            Norm::Centered { center, radius } => 0.5 + (v - center) / (2.0 * radius),
            Norm::Log { .. } if v <= 0.0 => return None,
            _ if low == high => 0.5,
            Norm::Log { .. } => log_offset(v, low) / log_offset(high, low),
            Norm::Linear { .. } => (v - low) / (high - low),
        };
        Some(share.clamp(0.0, 1.0))
    }

    /// The ticks of the range: about five, the center among them on a centered norm, and one
    /// tick at the value when the range is one value.
    fn ticks(&self) -> Vec<AxisTick> {
        self.ticks_for(TICK_TARGET, &crate::TickFormat::default())
    }

    /// The step between the automatic ticks of the range at the default density, on a linear
    /// or centered norm whose range gives at least two; `None` on a log norm, whose ticks are
    /// not evenly spaced, and for a range of one value.
    fn tick_step(&self) -> Option<f64> {
        if matches!(self, Norm::Log { .. }) {
            return None;
        }
        let (low, high) = self.range();
        let scale = ContinuousScale::new(Transform::Linear, low, high, CONTEXT).ok()?;
        match scale.ticks(TICK_TARGET).as_slice() {
            [first, second, ..] => Some(second.value - first.value),
            _ => None,
        }
    }

    /// Ticks for the requested density, labelled before their on-screen fit is checked.
    fn ticks_for(&self, target: usize, format: &crate::TickFormat) -> Vec<AxisTick> {
        let (low, high) = self.range();
        let transform = match *self {
            Norm::Log { base, .. } => Transform::Log(base),
            Norm::Linear { .. } | Norm::Centered { .. } => Transform::Linear,
        };
        let middle = match *self {
            Norm::Centered { center, .. } => center,
            Norm::Linear { .. } | Norm::Log { .. } => low + (high - low) / 2.0,
        };
        let mut ticks = match ContinuousScale::new(transform, low, high, CONTEXT) {
            Ok(scale) => scale.ticks(target),
            Err(_) => labelled(vec![middle]),
        };
        if let Norm::Centered { center, radius } = *self
            && !ticks
                .iter()
                .any(|tick| (tick.value - center).abs() <= 2e-9 * radius)
        {
            let mut values: Vec<f64> = ticks.iter().map(|tick| tick.value).collect();
            values.push(center);
            values.sort_by(f64::total_cmp);
            ticks = labelled(values);
        }
        format
            .relabel(ticks, crate::ticks::TickValues::Exact)
            .into_iter()
            .filter_map(|tick| {
                Some(AxisTick {
                    pos: NormalizedPos::new(self.unit(tick.value)?),
                    label: tick.label,
                })
            })
            .collect()
    }
}

/// Ticks at `values`, labelled as values a caller chose.
fn labelled(values: Vec<f64>) -> Vec<Tick> {
    let labels = format_given(&values);
    values
        .into_iter()
        .zip(labels)
        .map(|(value, label)| Tick { value, label })
        .collect()
}

/// The values of one color-mapped mark and the spec they map with, as a resolved mark holds
/// them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MappedColors<'a> {
    /// The colormap, scale and center.
    pub spec: ColorScaleSpec,
    /// One value per point; NaN leaves the point undrawn.
    pub values: &'a [f64],
}

impl<'a> MappedColors<'a> {
    /// The mapped colors of `by`.
    pub fn of(by: &'a ColorBy) -> MappedColors<'a> {
        MappedColors {
            spec: ColorScaleSpec::of(by),
            values: &by.values,
        }
    }
}

/// The color scale of an axes, resolved: its colormap, the norm of its range and the ticks of
/// that range, in increasing position.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedColorScale {
    /// The colormap.
    pub cmap: Cmap,
    /// How values map to positions on the colormap.
    pub norm: Norm,
    /// The ticks of the range, positioned on `0..=1` along it.
    pub ticks: Vec<AxisTick>,
}

impl ResolvedColorScale {
    /// The color scale of an axes whose resolved marks are `marks` and whose explicit color
    /// limits are `limits`, under `theme`: the spec of the first color-mapped mark, over the
    /// limits, or else over the extent of the finite mapped values (positive ones on a log
    /// scale). `None` when no mark is color-mapped or no mapped value is finite, even with
    /// limits: then nothing color-mapped is drawn.
    pub fn resolve<'a, 'm: 'a>(
        marks: impl Iterator<Item = &'a ResolvedMark<'m>> + Clone,
        limits: Option<(f64, f64)>,
        theme: &Theme,
    ) -> Option<ResolvedColorScale> {
        let (spec, _) = marks.clone().find_map(ResolvedMark::color_values)?;
        let log = matches!(spec.scale, Transform::Log(_));
        let extent = marks
            .filter_map(ResolvedMark::color_values)
            .flat_map(|(_, values)| values.iter().copied())
            .filter(|v| v.is_finite() && (!log || *v > 0.0))
            .fold(None, |extent: Option<(f64, f64)>, v| {
                Some(extent.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))))
            })?;
        let norm = Norm::new(spec, limits.unwrap_or(extent));
        Some(ResolvedColorScale {
            cmap: spec.cmap(theme).clone(),
            ticks: norm.ticks(),
            norm,
        })
    }

    /// Chooses colorbar ticks against the same measured room as axis ticks. Every candidate
    /// includes the center; reducing density never discards that required tick.
    pub(crate) fn with_ticks(
        &self,
        room: &crate::layout::TickRoom<'_>,
        format: &crate::TickFormat,
    ) -> Self {
        let mut ticks = self.norm.ticks_for(room.target, format);
        if room.slack(&ticks) < 0.0 {
            for target in (2..room.target).rev() {
                let candidate = self.norm.ticks_for(target, format);
                if candidate.len() <= ticks.len() {
                    ticks = candidate;
                }
                if room.slack(&ticks) >= 0.0 {
                    break;
                }
            }
        }
        // An off-grid center can be close to a regular tick at every density. Keep the
        // center and remove the tick whose removal gives the most room until labels fit.
        while ticks.len() > 1 && room.slack(&ticks) < 0.0 {
            let centered = matches!(self.norm, Norm::Centered { .. });
            let remove = (0..ticks.len())
                .filter(|&i| !centered || (ticks[i].pos.get() - 0.5).abs() > 1e-12)
                .max_by(|&a, &b| {
                    let without = |index| {
                        let mut candidate = ticks.clone();
                        candidate.remove(index);
                        room.slack(&candidate)
                    };
                    without(a).total_cmp(&without(b))
                });
            let Some(index) = remove else { break };
            ticks.remove(index);
        }
        Self {
            ticks,
            ..self.clone()
        }
    }

    /// One label for each of the finite `values`, such as the values written on heatmap cells,
    /// in `format`. Automatic labels write each value as the automatic ticks of the color range
    /// are written ([`ticks::format_annotations`]): with the decimals of the step between the
    /// ticks at the default density on a linear or centered scale, and with up to 12
    /// significant digits on a log scale or a range of one value.
    pub(crate) fn value_labels(&self, values: &[f64], format: &TickFormat) -> Vec<String> {
        let labels = ticks::format_annotations(values, self.norm.tick_step());
        let ticks = values
            .iter()
            .zip(labels)
            .map(|(&value, label)| Tick { value, label })
            .collect();
        format
            .relabel(ticks, TickValues::Exact)
            .into_iter()
            .map(|tick| tick.label)
            .collect()
    }

    /// The color of the value `v`; `None` for a value that maps to no color (NaN).
    pub fn color(&self, v: f64) -> Option<Rgba> {
        self.norm.unit(v).map(|t| self.cmap.at(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(scale: Transform, center: Option<f64>) -> ColorScaleSpec {
        ColorScaleSpec {
            cmap: CmapChoice::Theme,
            scale,
            center,
        }
    }

    fn labels(ticks: &[AxisTick]) -> Vec<&str> {
        ticks.iter().map(|tick| tick.label.as_str()).collect()
    }

    #[test]
    fn linear_norm_clamps_outside_the_range() {
        let norm = Norm::new(&spec(Transform::Linear, None), (0.0, 10.0));
        assert_eq!(
            norm,
            Norm::Linear {
                low: 0.0,
                high: 10.0
            }
        );
        assert_eq!(norm.unit(2.5), Some(0.25));
        assert_eq!(norm.unit(-5.0), Some(0.0));
        assert_eq!(norm.unit(20.0), Some(1.0));
        assert_eq!(norm.unit(f64::NAN), None);
    }

    #[test]
    fn centered_norm_is_symmetric_and_linear() {
        // Data from -2 to 8 about 0: the range is 0 ± 8, so -2 does not reach the low end.
        let norm = Norm::new(&spec(Transform::Linear, Some(0.0)), (-2.0, 8.0));
        assert_eq!(
            norm,
            Norm::Centered {
                center: 0.0,
                radius: 8.0
            }
        );
        assert_eq!(norm.unit(0.0), Some(0.5));
        assert_eq!(norm.unit(-2.0), Some(0.375));
        assert_eq!(norm.unit(4.0), Some(0.75));
        assert_eq!(norm.unit(8.0), Some(1.0));
        // Equal steps in value are equal steps on the colormap.
        let steps: Vec<f64> = [-8.0, -4.0, 0.0, 4.0, 8.0]
            .iter()
            .map(|&v| norm.unit(v).unwrap())
            .collect();
        assert_eq!(steps, [0.0, 0.25, 0.5, 0.75, 1.0]);
        // The center is always a tick.
        let ticks = norm.ticks();
        assert!(ticks.iter().any(|tick| tick.pos.get() == 0.5), "{ticks:?}");
        // A center at the data maximum gives a one-sided map.
        let one_sided = Norm::new(&spec(Transform::Linear, Some(10.0)), (4.0, 10.0));
        assert_eq!(one_sided.range(), (4.0, 16.0));
        assert_eq!(one_sided.unit(10.0), Some(0.5));
        assert_eq!(one_sided.unit(4.0), Some(0.0));
        // A center off the grid of ticks is added among them.
        let off_grid = Norm::new(&spec(Transform::Linear, Some(0.3)), (0.0, 10.0));
        assert!(
            off_grid
                .ticks()
                .iter()
                .any(|tick| (tick.pos.get() - 0.5).abs() < 1e-12 && tick.label == "0.3"),
            "{:?}",
            off_grid.ticks()
        );
    }

    #[test]
    fn a_tiny_radius_keeps_the_center_in_the_middle() {
        // The radius is 2^-53, so `center + radius` rounds back to the center.
        let low = 1.0 - f64::EPSILON / 2.0;
        let norm = Norm::new(&spec(Transform::Linear, Some(1.0)), (low, 1.0));
        assert_eq!(1.0 + (1.0 - low), 1.0);
        assert_eq!(norm.unit(1.0), Some(0.5));
        assert_eq!(norm.unit(low), Some(0.0));
        let ticks = norm.ticks();
        assert!(
            ticks
                .iter()
                .any(|tick| tick.pos.get() == 0.5 && tick.label == "1"),
            "{ticks:?}"
        );
    }

    #[test]
    fn one_value_range_maps_to_middle() {
        for scale in [Transform::Linear, Transform::Log(LogBase::TEN)] {
            let norm = Norm::new(&spec(scale, None), (4.0, 4.0));
            assert_eq!(norm.range(), (4.0, 4.0));
            for v in [1.0, 4.0, 7.0] {
                assert_eq!(norm.unit(v), Some(0.5), "{scale:?} {v}");
            }
            let ticks = norm.ticks();
            assert_eq!(labels(&ticks), ["4"], "{scale:?}");
            assert_eq!(ticks[0].pos.get(), 0.5);
        }
        let centered = Norm::new(&spec(Transform::Linear, Some(3.0)), (3.0, 3.0));
        assert_eq!(centered.unit(3.0), Some(0.5));
        assert_eq!(labels(&centered.ticks()), ["3"]);
    }

    #[test]
    fn log_norm_maps_by_the_logarithm() {
        let norm = Norm::new(&spec(Transform::Log(LogBase::TEN), None), (1.0, 100.0));
        assert_eq!(norm.unit(10.0), Some(0.5));
        assert_eq!(norm.unit(0.0), None);
        assert_eq!(norm.unit(-1.0), None);
        assert_eq!(labels(&norm.ticks()), ["1", "10", "100"]);
    }

    #[test]
    fn limits_are_checked_alone_and_against_the_spec() {
        let err = check_limits("Axes.clim()", (5.0, 2.0)).unwrap_err();
        assert!(matches!(err, Error::Style { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.clim(): low is 5, high is 2; low must be below high."
        );
        assert_eq!(err.hint(), Some("swap the two values."));
        assert_eq!(err.parameter(), Some("low"));
        let err = check_limits("Axes.clim()", (f64::NAN, 2.0)).unwrap_err();
        assert_eq!(err.parameter(), Some("low"));
        assert!(check_limits("Axes.clim()", (0.0, 10.0)).is_ok());

        let log = spec(Transform::Log(LogBase::TEN), None);
        let err = log
            .check_range((0.0, 10.0), "Axes.clim()", None)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Axes.clim(): the color limits are 0 and 10; a log color scale needs positive limits."
        );
        assert_eq!(err.parameter(), Some("low"));
        let centered = spec(Transform::Linear, Some(12.0));
        let err = centered
            .check_range((0.0, 10.0), "Axes.scatter()", Some("color"))
            .unwrap_err();
        assert!(matches!(err, Error::Style { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.scatter(): center is 12; it must lie inside the color range 0 to 10."
        );
        assert_eq!(err.parameter(), Some("color"));
        assert!(
            centered
                .check_range((0.0, 12.0), "Axes.clim()", None)
                .is_ok()
        );
    }

    #[test]
    fn mismatches_name_the_first_difference() {
        let viridis = ColorScaleSpec {
            cmap: CmapChoice::Fixed(Cmap::viridis()),
            ..spec(Transform::Linear, None)
        };
        let magma = ColorScaleSpec {
            cmap: CmapChoice::Fixed(Cmap::magma()),
            ..viridis.clone()
        };
        let err = magma
            .check_same(&viridis, "Axes.scatter()", "color")
            .unwrap_err();
        assert!(matches!(err, Error::Usage { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.scatter(): color uses Cmap.MAGMA; this axes already maps colors with \
             Cmap.VIRIDIS."
        );
        assert_eq!(err.hint(), Some(SAME_SCALE_HINT));
        let log = spec(Transform::Log(LogBase::TEN), None);
        assert_eq!(
            log.check_same(&spec(Transform::Linear, None), "Axes.scatter()", "color")
                .unwrap_err()
                .to_string(),
            "Axes.scatter(): color uses Scale.log(10); this axes already maps colors with \
             Scale.linear()."
        );
        assert_eq!(
            spec(Transform::Linear, Some(0.0))
                .check_same(&spec(Transform::Linear, None), "Axes.bubbles()", "color")
                .unwrap_err()
                .to_string(),
            "Axes.bubbles(): color uses center 0; this axes already maps colors with no center."
        );
        assert!(
            viridis
                .check_same(&viridis, "Axes.scatter()", "color")
                .is_ok()
        );
    }
}
