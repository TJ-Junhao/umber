//! The tick settings of one linear or log axis: explicit tick positions, with or without their
//! own labels, and the tick label format, with the rule that keeps given labels and a format
//! apart.

use super::validation::{check_tick_labels, check_tick_positions};
use crate::{Error, NumberStyle, Result, Tick, TickFormat, Transform};

/// Tick positions set explicitly on one axis, in place of the automatic ticks. Read them with
/// [`Axes::x_ticks`](crate::Axes::x_ticks) and [`Axes::y_ticks`](crate::Axes::y_ticks).
///
/// Positions are finite, strictly increasing, within the supported magnitudes and, on a log
/// axis, positive. An empty list means an axis without ticks and gridlines.
///
/// # Examples
///
/// ```
/// use umber_core::{ExplicitTicks, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.xticks(&[0.0, 5.0, 10.0])?;
/// assert_eq!(axes.x_ticks(), Some(&ExplicitTicks::Positions(vec![0.0, 5.0, 10.0])));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ExplicitTicks {
    /// Positions whose labels the axis's [`TickFormat`] writes.
    Positions(Vec<f64>),
    /// Positions with their own labels, drawn as given.
    Labeled(Vec<Tick>),
}

/// The tick settings of one linear or log axis: the explicit ticks, if set, and the label
/// format. Labels given with their positions and a format other than plain
/// [`TickFormat::AUTO`] cannot both be set: whichever setter comes second fails.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct AxisTicks {
    /// The explicit ticks; `None` for the automatic ones.
    explicit: Option<ExplicitTicks>,
    /// The label format.
    format: TickFormat,
}

impl AxisTicks {
    /// The explicit ticks; `None` when the axis uses the automatic ones.
    pub fn explicit(&self) -> Option<&ExplicitTicks> {
        self.explicit.as_ref()
    }

    /// The label format.
    pub fn format(&self) -> &TickFormat {
        &self.format
    }

    /// Replaces the ticks with `positions`, labelled by the format, after checking them for an
    /// axis of `transform` under `context`. Errors change nothing.
    pub(super) fn set_positions(
        &mut self,
        context: &'static str,
        transform: Transform,
        positions: &[f64],
    ) -> Result<()> {
        check_tick_positions(context, transform, positions)?;
        self.explicit = Some(ExplicitTicks::Positions(positions.to_vec()));
        Ok(())
    }

    /// Replaces the ticks with `positions` and their `labels`, after checking both for an axis
    /// of `transform` under `context`; `Error::Usage` when the format is not plain
    /// [`TickFormat::AUTO`]. Errors change nothing.
    pub(super) fn set_labeled(
        &mut self,
        context: &'static str,
        transform: Transform,
        positions: &[f64],
        labels: Vec<String>,
    ) -> Result<()> {
        check_tick_positions(context, transform, positions)?;
        check_tick_labels(context, positions.len(), &labels)?;
        if !self.format.is_auto() {
            return Err(Error::usage(
                context,
                "the axis has a tick format other than TickFormat.AUTO; labels given with their positions are drawn as given and take no format.",
            )
            .with_parameter("labels")
            .with_hint("set the tick format back to TickFormat.AUTO first, or pass only the positions."));
        }
        let ticks = positions
            .iter()
            .zip(labels)
            .map(|(&value, label)| Tick { value, label })
            .collect();
        self.explicit = Some(ExplicitTicks::Labeled(ticks));
        Ok(())
    }

    /// Sets the label format under `context`; `Error::Usage` when `format` is
    /// [`TickFormat::iso`], which labels times, and when the ticks have labels given with their
    /// positions and `format` is not plain [`TickFormat::AUTO`]. Errors change nothing.
    pub(super) fn set_format(&mut self, context: &'static str, format: TickFormat) -> Result<()> {
        if format.style() == NumberStyle::Iso {
            return Err(Error::usage(
                context,
                "this is a linear or log axis; TickFormat.iso() labels time axes only.",
            )
            .with_parameter("format")
            .with_hint("use TickFormat.AUTO or a number format on a numeric axis."));
        }
        let labeled = matches!(self.explicit, Some(ExplicitTicks::Labeled(_)));
        if labeled && !format.is_auto() {
            return Err(Error::usage(
                context,
                "the axis has tick labels given with their positions; they are drawn as given and take no format other than TickFormat.AUTO.",
            )
            .with_parameter("format")
            .with_hint("pass only the positions to have them formatted, or write the format into the labels."));
        }
        self.format = format;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AxesMut, Error, ExplicitTicks, Figure, LogBase, Result, ScaleKind, Theme, Tick, TickFormat,
    };

    fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        figure.add_axes(x, y).unwrap();
        figure
    }

    fn log() -> ScaleKind {
        ScaleKind::Log { base: LogBase::TEN }
    }

    fn strings(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|s| (*s).to_owned()).collect()
    }

    /// The category, context, message and parameter of an error.
    #[track_caller]
    fn fault<T: std::fmt::Debug>(r: Result<T>) -> (&'static str, String, Option<&'static str>) {
        let e = r.unwrap_err();
        let kind = match &e {
            Error::Style { .. } => "style",
            Error::Usage { .. } => "usage",
            Error::Data { .. } => "data",
            _ => "other",
        };
        (kind, e.to_string(), e.parameter())
    }

    /// Runs `op` on the axes of `f`, which must fail, and checks that the figure is unchanged.
    #[track_caller]
    fn fails_unchanged(
        f: &mut Figure,
        op: impl for<'b, 'f> FnOnce(&'b mut AxesMut<'f>) -> Result<&'b mut AxesMut<'f>>,
    ) -> (&'static str, String, Option<&'static str>) {
        let before = f.clone();
        let result = op(&mut f.single_axes_mut()).map(|_| ());
        assert_eq!(*f, before, "a failed call changed the axes");
        fault(result)
    }

    #[test]
    fn positions_must_be_finite_increasing_and_in_range() {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let cases: &[(&[f64], &str)] = &[
            (
                &[0.0, f64::NAN],
                "Axes.xticks(): positions[1] is NaN; tick positions must be finite.",
            ),
            (
                &[f64::NEG_INFINITY],
                "Axes.xticks(): positions[0] is -inf; tick positions must be finite.",
            ),
            (
                &[0.0, 1.0, 1.0],
                "Axes.xticks(): positions[2] is 1 but positions[1] is 1; tick positions must be strictly increasing.",
            ),
            (
                &[3.0, 2.0, f64::NAN],
                "Axes.xticks(): positions[1] is 2 but positions[0] is 3; tick positions must be strictly increasing.",
            ),
            (
                &[0.0, 2e300],
                "Axes.xticks(): positions[1] is 2e300; tick positions must not exceed 1e300 in magnitude.",
            ),
        ];
        for &(positions, message) in cases {
            let got = fails_unchanged(&mut f, |a| a.xticks(positions));
            assert_eq!(got, ("style", message.to_owned(), Some("positions")));
        }
        // The same checks under the y setter's name.
        let got = fails_unchanged(&mut f, |a| a.yticks(&[1.0, 0.5]));
        assert!(
            got.1.starts_with("Axes.yticks(): positions[1] is 0.5"),
            "{got:?}"
        );
        // Negative positions and the magnitude bounds themselves are fine on a linear axis.
        f.single_axes_mut()
            .xticks(&[-1e300, -1.0, 0.0, 1e300])
            .unwrap();
    }

    #[test]
    fn log_positions_must_be_positive_and_in_the_log_range() {
        let mut f = figure(log(), log());
        let got = fails_unchanged(&mut f, |a| a.yticks(&[1.0, 10.0, 0.0]));
        assert_eq!(
            got,
            (
                "style",
                "Axes.yticks(): positions[2] is 0 on a log axis; tick positions must be positive."
                    .to_owned(),
                Some("positions")
            )
        );
        let got = fails_unchanged(&mut f, |a| a.xticks(&[-1.0]));
        assert!(
            got.1.contains("positions[0] is -1 on a log axis"),
            "{got:?}"
        );
        let got = fails_unchanged(&mut f, |a| a.yticks(&[1e-301, 1.0]));
        assert_eq!(
            got.1,
            "Axes.yticks(): positions[0] is 1e-301 on a log axis; tick positions must lie from 1e-300 to 1e300."
        );
        let got = fails_unchanged(&mut f, |a| {
            a.xticks_labeled(&[1.0, 0.0], strings(&["a", "b"]))
        });
        assert!(
            got.1
                .starts_with("Axes.xticks_labeled(): positions[1] is 0"),
            "{got:?}"
        );
        f.single_axes_mut().yticks(&[1e-300, 1.0, 1e300]).unwrap();
    }

    #[test]
    fn labels_must_match_the_positions_and_fit() {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let got = fails_unchanged(&mut f, |a| a.xticks_labeled(&[0.0, 1.0], strings(&["a"])));
        assert_eq!(
            got,
            (
                "style",
                "Axes.xticks_labeled(): positions has 2 values but labels has 1; pass one label per position."
                    .to_owned(),
                Some("labels")
            )
        );
        let long = "x".repeat(1001);
        let got = fails_unchanged(&mut f, |a| {
            a.yticks_labeled(&[0.0, 1.0], vec!["ok".to_owned(), long])
        });
        assert_eq!(
            got,
            (
                "style",
                "Axes.yticks_labeled(): labels[1] has 1,001 characters; text must be at most 1,000 characters."
                    .to_owned(),
                Some("labels")
            )
        );
        // Positions are checked before labels.
        let got = fails_unchanged(&mut f, |a| a.xticks_labeled(&[f64::NAN], strings(&[])));
        assert!(got.1.contains("positions[0] is NaN"), "{got:?}");
    }

    #[test]
    fn category_x_axes_refuse_ticks_and_formats() {
        let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
        for (got, context, what) in [
            (
                fails_unchanged(&mut f, |a| a.xticks(&[0.0])),
                "Axes.xticks()",
                "ticks",
            ),
            (
                fails_unchanged(&mut f, |a| a.xticks_labeled(&[0.0], strings(&["a"]))),
                "Axes.xticks_labeled()",
                "ticks",
            ),
            (
                fails_unchanged(&mut f, |a| a.xtick_format(TickFormat::AUTO)),
                "Axes.xtick_format()",
                "tick formats",
            ),
        ] {
            assert_eq!(
                got,
                (
                    "usage",
                    format!(
                        "{context}: this is a category axis; {what} apply to linear and log axes only."
                    ),
                    None
                )
            );
        }
        // The y axis of the same axes takes ticks.
        f.single_axes_mut().yticks(&[0.0, 1.0]).unwrap();
    }

    #[test]
    fn labels_and_a_format_conflict_in_either_order() {
        let percent = TickFormat::percent(0).unwrap();
        let affixed_auto = TickFormat::AUTO.affix("", " kg").unwrap();

        // Format first, then labels.
        for format in [
            percent.clone(),
            affixed_auto.clone(),
            TickFormat::scientific(),
        ] {
            let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
            f.single_axes_mut().ytick_format(format).unwrap();
            let got = fails_unchanged(&mut f, |a| a.yticks_labeled(&[0.0], strings(&["zero"])));
            assert_eq!(got.0, "usage");
            assert!(
                got.1.starts_with(
                    "Axes.yticks_labeled(): the axis has a tick format other than TickFormat.AUTO"
                ),
                "{got:?}"
            );
            assert_eq!(got.2, Some("labels"));
        }

        // Labels first, then a format.
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        f.single_axes_mut()
            .xticks_labeled(&[0.0, 1.0], strings(&["off", "on"]))
            .unwrap();
        for format in [percent.clone(), affixed_auto] {
            let got = fails_unchanged(&mut f, |a| a.xtick_format(format));
            assert_eq!(got.0, "usage");
            assert!(
                got.1.starts_with(
                    "Axes.xtick_format(): the axis has tick labels given with their positions"
                ),
                "{got:?}"
            );
            assert_eq!(got.2, Some("format"));
        }
        // Plain AUTO never conflicts.
        f.single_axes_mut().xtick_format(TickFormat::AUTO).unwrap();
        // The axes are independent: the y axis takes any format.
        f.single_axes_mut().ytick_format(percent.clone()).unwrap();

        // Positions without labels replace the labelled ticks, and the conflict is gone.
        let mut axes = f.single_axes_mut();
        axes.xticks(&[0.0, 1.0]).unwrap();
        axes.xtick_format(percent.clone()).unwrap();
        assert_eq!(
            axes.x_ticks(),
            Some(&ExplicitTicks::Positions(vec![0.0, 1.0]))
        );
        assert_eq!(axes.x_tick_format(), &percent);
    }

    #[test]
    fn tick_setters_replace_the_ticks_and_keep_the_format() {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let mut axes = f.single_axes_mut();
        assert_eq!((axes.x_ticks(), axes.y_ticks()), (None, None));
        axes.yticks_labeled(&[0.0, 5.0], strings(&["none", "five"]))
            .unwrap();
        assert_eq!(
            axes.y_ticks(),
            Some(&ExplicitTicks::Labeled(vec![
                Tick {
                    value: 0.0,
                    label: "none".into()
                },
                Tick {
                    value: 5.0,
                    label: "five".into()
                },
            ]))
        );
        axes.yticks(&[]).unwrap();
        assert_eq!(axes.y_ticks(), Some(&ExplicitTicks::Positions(Vec::new())));
        let fixed = TickFormat::fixed(3).unwrap();
        axes.ytick_format(fixed.clone()).unwrap();
        axes.yticks(&[1.0, 2.0]).unwrap();
        assert_eq!(axes.y_tick_format(), &fixed);
        assert_eq!(
            axes.y_ticks(),
            Some(&ExplicitTicks::Positions(vec![1.0, 2.0]))
        );
        // The x axis is untouched by all of it.
        assert_eq!(
            (axes.x_ticks(), axes.x_tick_format()),
            (None, &TickFormat::AUTO)
        );
        // Clones are independent.
        let copy = f.clone();
        f.single_axes_mut().yticks(&[3.0]).unwrap();
        assert_eq!(
            copy.single_axes().y_ticks(),
            Some(&ExplicitTicks::Positions(vec![1.0, 2.0]))
        );
    }

    #[test]
    fn numeric_axes_refuse_iso_labels() {
        for kind in [ScaleKind::Linear, log()] {
            let mut f = figure(kind, kind);
            for (got, context) in [
                (
                    fails_unchanged(&mut f, |a| a.xtick_format(TickFormat::iso())),
                    "Axes.xtick_format()",
                ),
                (
                    fails_unchanged(&mut f, |a| {
                        a.ytick_format(TickFormat::iso().affix("", " UTC").unwrap())
                    }),
                    "Axes.ytick_format()",
                ),
            ] {
                assert_eq!(
                    got,
                    (
                        "usage",
                        format!(
                            "{context}: this is a linear or log axis; TickFormat.iso() labels time axes only."
                        ),
                        Some("format")
                    )
                );
            }
        }
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let err = f
            .single_axes_mut()
            .xtick_format(TickFormat::iso())
            .map(|_| ())
            .unwrap_err();
        assert_eq!(
            err.hint(),
            Some("use TickFormat.AUTO or a number format on a numeric axis.")
        );
    }
}
