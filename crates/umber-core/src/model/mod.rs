//! The figure model: a [`Figure`] holds a [`Grid`] and the [`Axes`] placed on it, which hold
//! [`Mark`]s. An [`AxesMut`] changes one axes of a figure.
//!
//! The model is data only. Every input is validated at the call that receives it, so a model
//! that exists is always drawable. Marks hold copies of their data that never change, so clones
//! of a figure share them. Explicit limits are kept as the scales they define, and explicit
//! ticks and tick formats as set, once per shared group. Colors stay as
//! [`ColorSpec`](crate::ColorSpec)s; resolving them and drawing are later stages that read the
//! model.

mod axes;
mod axes_mut;
mod axis_data;
mod band;
mod bar;
mod box_plot;
mod bubbles;
mod error_bars;
mod figure;
mod grid;
mod hist;
mod mark;
mod rule;
mod share;
mod stacked_area;
mod stem;
mod step;
mod ticks;
mod validation;
mod violin;
pub use violin::{ViolinGroupMark, ViolinMark, ViolinOptions};

#[cfg(test)]
mod share_tests;

pub use axes::Axes;
pub use axes_mut::AxesMut;
pub use axis_data::AxisData;
pub use band::{BandMark, BandOptions};
pub use bar::{BarGroupMark, BarMark, BarOptions, Series, SeriesView};
pub use box_plot::{BoxGroupMark, BoxMark, BoxOptions, Distribution, DistributionView};
pub use bubbles::{BubbleMark, BubbleOptions};
pub use error_bars::{ErrorBarMark, ErrorBarOptions, ErrorBarOrientation};
pub use figure::Figure;
pub use grid::{Cell, CellRange, Grid, GridIndex, LegendMode, MAX_GRID, Placement, Share};
pub use hist::{HistMark, HistOptions};
pub use mark::{AreaMark, AreaOptions, LineMark, LineOptions, Mark, ScatterMark, ScatterOptions};
pub use rule::{AxisValue, HLineMark, RuleOptions, SpanMark, SpanOptions, VLineMark};
pub(crate) use share::{ShareGroups, label_visibility};
pub use stacked_area::StackedAreaMark;
pub use stem::{StemMark, StemOptions};
pub(crate) use step::{Edge, holds};
pub use step::{Step, StepMark, StepOptions};
pub(crate) use ticks::AxisTicks;
pub use ticks::ExplicitTicks;
pub(crate) use validation::check_text;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, LogBase, Result, ScaleKind, Theme};

    fn figure() -> Figure {
        Figure::new(6.0, 4.0, Theme::pigment()).unwrap()
    }

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn log_kind() -> ScaleKind {
        ScaleKind::Log { base: LogBase::TEN }
    }

    /// Asserts a `Data` error from `context` whose message contains `needle`.
    #[track_caller]
    fn assert_data<T: std::fmt::Debug>(r: Result<T>, context: &str, needle: &str) {
        match r {
            Err(Error::Data {
                context: c,
                message,
                ..
            }) => {
                assert_eq!(c, context);
                assert!(message.contains(needle), "{message}");
            }
            other => panic!("expected Data error, got {other:?}"),
        }
    }

    #[track_caller]
    fn assert_style<T: std::fmt::Debug>(r: Result<T>, context: &str) {
        match r {
            Err(Error::Style { context: c, .. }) => assert_eq!(c, context),
            other => panic!("expected Style error, got {other:?}"),
        }
    }

    #[track_caller]
    fn assert_usage<T: std::fmt::Debug>(r: Result<T>, context: &str) {
        match r {
            Err(Error::Usage { context: c, .. }) => assert_eq!(c, context),
            other => panic!("expected Usage error, got {other:?}"),
        }
    }

    #[test]
    fn figure_size_is_validated() {
        for (w, h) in [
            (f64::NAN, 4.0),
            (6.0, f64::INFINITY),
            (0.0, 4.0),
            (6.0, -1.0),
            (200.5, 4.0),
            (6.0, 201.0),
        ] {
            assert_style(Figure::new(w, h, Theme::pigment()), "Figure()");
        }
        assert!(Figure::new(200.0, 0.01, Theme::pigment()).is_ok());
    }

    #[test]
    fn second_add_axes_is_a_usage_error() {
        let mut f = figure();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_usage(
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear),
            "Figure.add_axes()",
        );
    }

    #[test]
    fn length_mismatch_is_a_data_error() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            a.line(nums(&[0.0, 1.0, 2.0]), &[1.0, 2.0], LineOptions::default()),
            "Axes.line()",
            "x has 3 values but y has 2",
        );
        assert_data(
            a.scatter(nums(&[0.0]), &[1.0, 2.0], ScatterOptions::default()),
            "Axes.scatter()",
            "x has 1 values but y has 2",
        );
        assert!(a.marks().is_empty());
    }

    #[test]
    fn infinite_values_are_data_errors() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            a.line(
                nums(&[0.0, 1.0, 2.0, f64::INFINITY]),
                &[0.0; 4],
                LineOptions::default(),
            ),
            "Axes.line()",
            "x[3] is inf; values must be finite.",
        );
        assert_data(
            a.scatter(
                nums(&[0.0, 1.0]),
                &[0.0, f64::NEG_INFINITY],
                ScatterOptions::default(),
            ),
            "Axes.scatter()",
            "y[1] is -inf",
        );
        assert!(a.marks().is_empty());
    }

    #[test]
    fn huge_magnitudes_are_data_errors() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            a.line(nums(&[0.0, 1e301]), &[0.0, 0.0], LineOptions::default()),
            "Axes.line()",
            "x[1] is 1e301",
        );
        assert_data(
            a.line(nums(&[0.0]), &[-1e301], LineOptions::default()),
            "Axes.line()",
            "y[0] is -1e301",
        );
        assert!(
            a.line(nums(&[0.0, 1e300]), &[-1e300, 0.0], LineOptions::default())
                .is_ok()
        );
    }

    #[test]
    fn non_positive_values_on_a_log_axis_are_data_errors() {
        let mut f = figure();
        let mut a = f.add_axes(log_kind(), log_kind()).unwrap();
        assert_data(
            a.line(nums(&[1.0, 0.0]), &[1.0, 1.0], LineOptions::default()),
            "Axes.line()",
            "x[1] is 0 on a log axis",
        );
        assert_data(
            a.scatter(nums(&[1.0, 2.0]), &[1.0, -3.0], ScatterOptions::default()),
            "Axes.scatter()",
            "y[1] is -3 on a log axis",
        );
        // NaN is ignored.
        assert!(
            a.line(
                nums(&[1.0, f64::NAN]),
                &[f64::NAN, 2.0],
                LineOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn log_axis_values_outside_the_log_range_are_data_errors() {
        let mut f = figure();
        let mut a = f.add_axes(log_kind(), log_kind()).unwrap();
        assert_data(
            a.line(nums(&[1.0, 1e-301]), &[1.0, 1.0], LineOptions::default()),
            "Axes.line()",
            "x[1] is 1e-301 on a log axis",
        );
        assert_data(
            a.line(nums(&[1.0]), &[1e301], LineOptions::default()),
            "Axes.line()",
            "y[0] is 1e301",
        );
        assert!(
            a.line(
                nums(&[1e-300, 1e300]),
                &[1e-300, 1e300],
                LineOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn axis_data_kind_must_match_the_axis() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        assert_data(
            a.line(nums(&[0.0]), &[1.0], LineOptions::default()),
            "Axes.line()",
            "category axis",
        );
        assert!(
            a.line(
                AxisData::Categories(vec!["a".into()]),
                &[1.0],
                LineOptions::default()
            )
            .is_ok()
        );

        let mut g = figure();
        let mut b = g.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            b.scatter(
                AxisData::Categories(vec!["a".into()]),
                &[1.0],
                ScatterOptions::default(),
            ),
            "Axes.scatter()",
            "not a category axis",
        );
        assert!(b.marks().is_empty());
    }

    /// Asserts the error of the mark method `context` called on a category y axis.
    #[track_caller]
    fn assert_category_y<T: std::fmt::Debug>(r: Result<T>, context: &str) {
        match &r {
            Err(err @ Error::Usage { message, .. }) => {
                let expected = format!("y is a category scale; {context} needs a numeric y axis.");
                assert_eq!(message, &expected);
                assert_eq!(err.to_string(), format!("{context}: {expected}"));
                assert_eq!(err.parameter(), Some("y"));
                assert_eq!(err.hint(), Some("use a linear or log y axis."));
            }
            other => panic!("expected Usage error, got {other:?}"),
        }
    }

    #[test]
    fn add_axes_accepts_category_y() {
        for x in [ScaleKind::Linear, log_kind(), ScaleKind::Category] {
            let mut f = figure();
            let mut a = f.add_axes(x, ScaleKind::Category).unwrap();
            assert_eq!((a.x_scale(), a.y_scale()), (x, ScaleKind::Category));
            let names = AxisData::Categories(vec!["a".into()]);
            let x_data = if x == ScaleKind::Category {
                names.clone()
            } else {
                nums(&[1.0])
            };
            assert_category_y(
                a.line(x_data.clone(), &[1.0], LineOptions::default()),
                "Axes.line()",
            );
            // The y check comes first, whatever the values.
            assert_category_y(
                a.line_shared(x_data, names, LineOptions::default()),
                "Axes.line()",
            );
            assert!(a.marks().is_empty());
            // The axes render, without marks.
            assert!(f.to_scene(crate::FontLibrary::global()).is_ok());
        }
        // A category y axis takes a cell like any other axes.
        let mut f = figure();
        f.add_axes(ScaleKind::Linear, log_kind()).unwrap();
        match f.add_axes(ScaleKind::Linear, ScaleKind::Category) {
            Err(Error::Usage { message, .. }) => assert_eq!(
                message,
                "row 0, col 0 is taken by the axes added at row 0, col 0."
            ),
            other => panic!("expected Usage error, got {other:?}"),
        }
        let scales: Vec<ScaleKind> = f.panels().map(Axes::y_scale).collect();
        assert_eq!(scales, [log_kind()]);
    }

    #[test]
    fn every_mark_method_refuses_a_category_y() {
        use crate::model::{
            AreaOptions, AxisValue, BandOptions, BarOptions, BoxOptions, BubbleOptions,
            Distribution, ErrorBarOptions, HistOptions, RuleOptions, Series, SpanOptions,
            StemOptions, StepOptions, ViolinOptions,
        };
        let mut f = figure();
        let mut a = f
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap();
        let names = || vec!["a".to_owned()];
        let cats = || AxisData::Categories(names());
        let series = || {
            vec![Series {
                values: vec![1.0],
                ..Series::default()
            }]
        };
        let distribution = || {
            vec![Distribution {
                groups: vec![vec![1.0]],
                ..Distribution::default()
            }]
        };
        let one = || vec![vec![1.0]];
        assert_category_y(
            a.line(cats(), &[1.0], LineOptions::default()),
            "Axes.line()",
        );
        assert_category_y(
            a.scatter(cats(), &[1.0], ScatterOptions::default()),
            "Axes.scatter()",
        );
        assert_category_y(
            a.bubbles(&[1.0], &[1.0], &[1.0], BubbleOptions::default()),
            "Axes.bubbles()",
        );
        assert_category_y(
            a.stem(cats(), &[1.0], StemOptions::default()),
            "Axes.stem()",
        );
        assert_category_y(
            a.area(cats(), &[1.0], AreaOptions::default()),
            "Axes.area()",
        );
        assert_category_y(a.stacked_area(&[1.0], series()), "Axes.stacked_area()");
        assert_category_y(a.bar(names(), &[1.0], BarOptions::default()), "Axes.bar()");
        assert_category_y(a.grouped_bar(names(), series()), "Axes.grouped_bar()");
        assert_category_y(a.stacked_bar(names(), series()), "Axes.stacked_bar()");
        assert_category_y(
            a.box_plot(names(), one(), BoxOptions::default()),
            "Axes.box()",
        );
        assert_category_y(
            a.grouped_box_plot(names(), distribution()),
            "Axes.grouped_box()",
        );
        assert_category_y(
            a.violin(names(), one(), ViolinOptions::default()),
            "Axes.violin()",
        );
        assert_category_y(
            a.grouped_violin(names(), distribution()),
            "Axes.grouped_violin()",
        );
        assert_category_y(a.hist(&[1.0], HistOptions::default()), "Axes.hist()");
        assert_category_y(
            a.step(cats(), &[1.0], StepOptions::default()),
            "Axes.step()",
        );
        assert_category_y(a.hline(1.0, RuleOptions::default()), "Axes.hline()");
        assert_category_y(
            a.vline(AxisValue::Category("a".into()), RuleOptions::default()),
            "Axes.vline()",
        );
        assert_category_y(a.hspan(0.0, 1.0, SpanOptions::default()), "Axes.hspan()");
        assert_category_y(a.vspan(0.0, 1.0, SpanOptions::default()), "Axes.vspan()");
        assert_category_y(
            a.band(&[1.0], &[0.0], &[1.0], BandOptions::default()),
            "Axes.band()",
        );
        assert_category_y(
            a.y_error_bars(cats(), &[0.0], &[1.0], ErrorBarOptions::default()),
            "Axes.y_error_bars()",
        );
        assert_category_y(
            a.x_error_bars(&[1.0], &[0.0], &[1.0], ErrorBarOptions::default()),
            "Axes.x_error_bars()",
        );
        assert!(a.marks().is_empty());
    }

    #[test]
    fn y_limits_and_ticks_refuse_a_category_y() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
        let hint = Some("pass y=Scale.linear() or y=Scale.log() to Figure.add_axes().");
        for (result, message) in [
            (
                a.ylim(0.0, 1.0).map(drop),
                "Axes.ylim(): this is a category axis; limits apply to linear and log axes only.",
            ),
            (
                a.yticks(&[0.0]).map(drop),
                "Axes.yticks(): this is a category axis; ticks apply to linear and log axes only.",
            ),
            (
                a.yticks_labeled(&[0.0], vec!["a".into()]).map(drop),
                "Axes.yticks_labeled(): this is a category axis; ticks apply to linear and log axes only.",
            ),
            (
                a.ytick_format(crate::TickFormat::AUTO).map(drop),
                "Axes.ytick_format(): this is a category axis; tick formats apply to linear and log axes only.",
            ),
        ] {
            let err = result.unwrap_err();
            assert!(matches!(err, Error::Usage { .. }), "{err:?}");
            assert_eq!(err.to_string(), message);
            assert_eq!(err.hint(), hint, "{message}");
        }
        assert_eq!((a.y_limits(), a.y_ticks()), (None, None));
        // The x axis of the same axes is linear, so it takes limits.
        assert!(a.xlim(0.0, 1.0).is_ok());
    }

    #[test]
    fn invalid_limits_are_style_errors() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        for (lo, hi) in [
            (1.0, 1.0),
            (2.0, 1.0),
            (f64::NAN, 1.0),
            (0.0, f64::INFINITY),
            (f64::NEG_INFINITY, 0.0),
            (0.0, 1e301),
            (-1e301, 0.0),
        ] {
            assert_style(a.xlim(lo, hi), "Axes.xlim()");
            assert_style(a.ylim(lo, hi), "Axes.ylim()");
        }
        assert_eq!(a.x_limits(), None);
        assert_eq!(a.y_limits(), None);
        a.xlim(-1e300, 1e300).unwrap().ylim(0.0, 1.0).unwrap();
        assert_eq!(a.x_limits(), Some((-1e300, 1e300)));
        assert_eq!(a.y_limits(), Some((0.0, 1.0)));
    }

    #[test]
    fn non_positive_or_out_of_range_limits_on_a_log_axis_are_style_errors() {
        let mut f = figure();
        let mut a = f.add_axes(log_kind(), log_kind()).unwrap();
        assert_style(a.xlim(0.0, 10.0), "Axes.xlim()");
        assert_style(a.xlim(-5.0, 10.0), "Axes.xlim()");
        assert_style(a.ylim(1e-301, 10.0), "Axes.ylim()");
        assert_style(a.ylim(1.0, 1e301), "Axes.ylim()");
        assert!(a.xlim(1e-300, 1e300).is_ok());
    }

    #[test]
    fn extreme_limits_print_in_scientific_form() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, log_kind()).unwrap();
        assert_eq!(
            a.xlim(0.0, 1e301).unwrap_err().to_string(),
            "Axes.xlim(): the limits are 0 and 1e301; their magnitude must not exceed 1e300."
        );
        assert_eq!(
            a.ylim(1e-301, 1.0).unwrap_err().to_string(),
            "Axes.ylim(): the limits are 1e-301 and 1; a log scale supports values from 1e-300 to 1e300."
        );
    }

    #[test]
    fn limits_on_a_category_axis_are_usage_errors() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        assert_usage(a.xlim(0.0, 1.0), "Axes.xlim()");
        // Usage wins over invalid values.
        assert_usage(a.xlim(1.0, 0.0), "Axes.xlim()");
        assert_eq!(a.x_limits(), None);
        // The y axis of the same axes is linear, so it takes limits.
        a.ylim(0.0, 1.0).unwrap();
    }

    #[test]
    fn nan_is_allowed_in_lines_and_scatters() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.line(
            nums(&[0.0, f64::NAN, 2.0]),
            &[f64::NAN, 1.0, 2.0],
            LineOptions::default(),
        )
        .unwrap()
        .scatter(nums(&[f64::NAN]), &[f64::NAN], ScatterOptions::default())
        .unwrap();
        assert_eq!(a.marks().len(), 2);
    }

    #[test]
    fn marks_store_copies_of_the_input() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let mut y = vec![1.0, 2.0];
        let x = nums(&[0.0, 1.0]);
        a.line(x.clone(), &y, LineOptions::default()).unwrap();
        y[0] = 99.0;
        match a.marks() {
            [Mark::Line(line)] => {
                assert_eq!(line.y(), &nums(&[1.0, 2.0]));
                assert_eq!(line.x(), &x);
            }
            other => panic!("unexpected marks {other:?}"),
        }
    }

    #[test]
    fn marks_keep_their_width_and_size() {
        use crate::SizeSpec;
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let line = LineOptions {
            width: SizeSpec::times(2.0).unwrap(),
            ..LineOptions::default()
        };
        let area = AreaOptions {
            width: SizeSpec::pt(0.5).unwrap(),
            ..AreaOptions::default()
        };
        let scatter = ScatterOptions {
            size: SizeSpec::pt(9.0).unwrap(),
            ..ScatterOptions::default()
        };
        a.line(nums(&[0.0]), &[1.0], line.clone())
            .unwrap()
            .area(nums(&[0.0]), &[1.0], area.clone())
            .unwrap()
            .scatter(nums(&[0.0]), &[1.0], scatter.clone())
            .unwrap();
        match a.marks() {
            [Mark::Line(l), Mark::Area(r), Mark::Scatter(s)] => {
                assert_eq!(l.options(), &line);
                assert_eq!(r.options(), &area);
                assert_eq!(s.options(), &scatter);
            }
            other => panic!("unexpected marks {other:?}"),
        }
    }

    #[test]
    fn failed_calls_change_nothing() {
        let mut f = figure();
        f.add_axes(ScaleKind::Linear, log_kind())
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap()
            .xlim(-1.0, 2.0)
            .unwrap()
            .ylim(0.5, 50.0)
            .unwrap()
            .title("kept");
        let before = f.clone();
        let mut a = f.single_axes_mut();
        assert!(
            a.line(nums(&[0.0]), &[1.0, 2.0], LineOptions::default())
                .is_err()
        );
        assert!(
            a.line(nums(&[0.0]), &[0.0], LineOptions::default())
                .is_err()
        );
        assert!(
            a.scatter(
                AxisData::Categories(vec!["a".into()]),
                &[1.0],
                ScatterOptions::default()
            )
            .is_err()
        );
        assert!(
            a.scatter(nums(&[f64::INFINITY]), &[1.0], ScatterOptions::default())
                .is_err()
        );
        assert!(a.xlim(2.0, -1.0).is_err());
        assert!(a.xlim(f64::NAN, 1.0).is_err());
        assert!(a.ylim(0.0, 10.0).is_err());
        assert!(a.ylim(1.0, 1e301).is_err());
        assert!(f.add_axes(ScaleKind::Linear, ScaleKind::Linear).is_err());
        assert_eq!(f, before);
        let a = f.single_axes();
        assert_eq!(a.x_limits(), Some((-1.0, 2.0)));
        assert_eq!(a.y_limits(), Some((0.5, 50.0)));
        assert_eq!(a.marks().len(), 1);
    }

    #[test]
    fn explicit_limits_keep_the_scale_they_define() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, log_kind()).unwrap();
        a.xlim(-0.0, 3.5).unwrap().ylim(2.0, 500.0).unwrap();
        let x = a.x_limit_scale().unwrap();
        let y = a.y_limit_scale().unwrap();
        assert_eq!(x.transform(), crate::Transform::Linear);
        assert_eq!(y.transform(), crate::Transform::Log(LogBase::TEN));
        // The getters give back the limits exactly as passed, the sign of zero included.
        let (lo, hi) = a.x_limits().unwrap();
        assert_eq!(
            (lo.to_bits(), hi.to_bits()),
            ((-0.0_f64).to_bits(), 3.5_f64.to_bits())
        );
        assert_eq!(a.y_limits(), Some((2.0, 500.0)));
        assert_eq!(x.domain(), (-0.0, 3.5));
        assert_eq!(y.domain(), (2.0, 500.0));
    }

    #[test]
    fn text_setters_store_text() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.title("t").subtitle("s").xlabel("x").ylabel("y");
        assert_eq!(
            (
                a.title_text(),
                a.subtitle_text(),
                a.xlabel_text(),
                a.ylabel_text()
            ),
            ("t", "s", "x", "y")
        );
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn bars(a: &mut AxesMut<'_>, categories: &[&str], heights: &[f64]) -> Result<()> {
        a.bar(names(categories), heights, BarOptions::default())
            .map(|_| ())
    }

    #[test]
    fn bar_checks_run_in_order() {
        const BAR: &str = "Axes.bar()";
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        // Lengths first, whatever else is wrong.
        assert_data(
            bars(&mut a, &["a", "a"], &[f64::INFINITY]),
            BAR,
            "categories has 2 values but heights has 1; categories and heights must have the same length.",
        );
        // Duplicates before heights; the message names both indices.
        assert_data(
            bars(&mut a, &["a", "b", "a"], &[f64::INFINITY, 1.0, 2.0]),
            BAR,
            "categories[2] repeats \"a\" from categories[0]; each category must appear once.",
        );
        // The first bad height, by index.
        assert_data(
            bars(
                &mut a,
                &["a", "b", "c", "d"],
                &[1.0, f64::NAN, 1e301, f64::INFINITY],
            ),
            BAR,
            "heights[2] is 1e301; magnitudes must not exceed 1e300.",
        );
        assert_data(
            bars(
                &mut a,
                &["a", "b", "c", "d"],
                &[1.0, 2.0, 3.0, f64::INFINITY],
            ),
            BAR,
            "heights[3] is inf; values must be finite",
        );
        assert!(a.marks().is_empty());

        // The x axis kind before duplicates.
        let mut g = figure();
        let mut b = g.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            bars(&mut b, &["a", "a"], &[1.0, 2.0]),
            BAR,
            "categories holds names but the x axis is not a category axis; use a category x axis.",
        );

        // Bars refuse a category y axis.
        let mut h = figure();
        let mut c = h
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap();
        assert_category_y(bars(&mut c, &["a"], &[1.0]), BAR);
    }

    #[test]
    fn bars_on_a_log_y_axis_need_positive_heights() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, log_kind()).unwrap();
        assert_data(
            bars(&mut a, &["a", "b"], &[2.0, 0.0]),
            "Axes.bar()",
            "heights[1] is 0 on a log axis; values must be positive.",
        );
        assert_data(
            bars(&mut a, &["a"], &[1e-301]),
            "Axes.bar()",
            "heights[0] is 1e-301 on a log axis",
        );
        assert!(a.marks().is_empty());
        bars(&mut a, &["a", "b"], &[2.0, f64::NAN]).unwrap();
        assert_eq!(a.marks().len(), 1);
    }

    #[test]
    fn bars_store_their_input_and_share_it_with_clones() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        let heights = [-0.0, f64::from_bits(0xfff8_0000_0000_0001), -2.5, 0.0];
        let options = BarOptions {
            label: "sales".to_owned(),
            color: crate::ColorSpec::Palette {
                index: 2,
                alpha: crate::Alpha::OPAQUE,
            },
        };
        // Empty bars are allowed, and so is an empty category name.
        a.bar(Vec::new(), &[], BarOptions::default())
            .unwrap()
            .bar(names(&["b", "", "a", "c"]), &heights, options.clone())
            .unwrap();
        let copy = f.clone();
        let (Some([_, Mark::Bar(bar)]), Some([_, Mark::Bar(cloned)])) = (
            f.panels().next().map(Axes::marks),
            copy.panels().next().map(Axes::marks),
        ) else {
            panic!("expected two bar marks");
        };
        assert_eq!(bar.categories(), names(&["b", "", "a", "c"]));
        let bits = |v: &[f64]| v.iter().map(|h| h.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(bar.heights()), bits(&heights));
        assert_eq!(bar.options(), &options);
        assert!(bar.shares_data_with(cloned));
    }

    #[test]
    fn failed_bar_calls_change_nothing() {
        let mut f = figure();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .bar(names(&["a"]), &[1.0], BarOptions::default())
            .unwrap();
        let before = f.clone();
        let mut a = f.single_axes_mut();
        assert!(bars(&mut a, &["a"], &[]).is_err());
        assert!(bars(&mut a, &["a", "a"], &[1.0, 1.0]).is_err());
        assert!(bars(&mut a, &["b"], &[f64::INFINITY]).is_err());
        assert_eq!(f, before);
    }

    fn series(values: &[f64]) -> Series {
        Series {
            values: values.to_vec(),
            ..Series::default()
        }
    }

    fn grouped(a: &mut AxesMut<'_>, categories: &[&str], heights: &[&[f64]]) -> Result<()> {
        let list = heights.iter().map(|h| series(h)).collect();
        a.grouped_bar(names(categories), list).map(|_| ())
    }

    fn stacked(a: &mut AxesMut<'_>, categories: &[&str], heights: &[&[f64]]) -> Result<()> {
        let list = heights.iter().map(|h| series(h)).collect();
        a.stacked_bar(names(categories), list).map(|_| ())
    }

    const GROUPED: &str = "Axes.grouped_bar()";
    const STACKED: &str = "Axes.stacked_bar()";

    #[test]
    fn an_empty_series_list_is_a_data_error() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        // Before anything else, even with no categories.
        assert_data(
            grouped(&mut a, &["a", "a"], &[]),
            GROUPED,
            "series is empty; pass at least one series.",
        );
        assert_data(stacked(&mut a, &[], &[]), STACKED, "series is empty");
        assert!(a.marks().is_empty());
    }

    #[test]
    fn a_series_length_mismatch_names_the_series() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        for (context, call) in [
            (
                GROUPED,
                grouped as fn(&mut AxesMut<'_>, &[&str], &[&[f64]]) -> Result<()>,
            ),
            (STACKED, stacked),
        ] {
            assert_data(
                call(
                    &mut a,
                    &["a", "b"],
                    &[&[1.0, 2.0], &[1.0, 2.0], &[f64::INFINITY]],
                ),
                context,
                "categories has 2 values but series[2].values has 1; each series must have one value per category.",
            );
            // The first mismatched series, before duplicates and bad heights.
            assert_data(
                call(&mut a, &["a", "a"], &[&[f64::INFINITY], &[1.0, 2.0, 3.0]]),
                context,
                "series[0].values has 1",
            );
        }
        assert!(a.marks().is_empty());
    }

    #[test]
    fn grouped_and_stacked_checks_run_in_order() {
        for (context, call) in [
            (
                GROUPED,
                grouped as fn(&mut AxesMut<'_>, &[&str], &[&[f64]]) -> Result<()>,
            ),
            (STACKED, stacked),
        ] {
            // The x axis kind before duplicates.
            let mut f = figure();
            let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            assert_data(
                call(&mut a, &["a", "a"], &[&[1.0, 2.0]]),
                context,
                "categories holds names but the x axis is not a category axis; use a category x axis.",
            );

            let mut f = figure();
            let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
            // Duplicates before heights.
            assert_data(
                call(&mut a, &["a", "b", "a"], &[&[f64::INFINITY; 3]]),
                context,
                "categories[2] repeats \"a\" from categories[0]; each category must appear once.",
            );
            // Heights in series order; the message names the series and the height.
            assert_data(
                call(
                    &mut a,
                    &["a", "b"],
                    &[&[1.0, f64::NAN], &[2.0, 1e301], &[f64::INFINITY, 0.0]],
                ),
                context,
                "series[1].values[1] is 1e301; magnitudes must not exceed 1e300.",
            );
            assert!(a.marks().is_empty());

            // A log y axis needs positive heights.
            let mut f = figure();
            let mut a = f.add_axes(ScaleKind::Category, log_kind()).unwrap();
            assert_data(
                call(&mut a, &["a", "b"], &[&[1.0, 2.0], &[3.0, -1.0]]),
                context,
                "series[1].values[1] is -1 on a log axis; values must be positive.",
            );
            call(&mut a, &["a", "b"], &[&[1.0, f64::NAN], &[1e300, 2.0]]).unwrap();
            assert_eq!(a.marks().len(), 1);
        }
    }

    #[test]
    fn stacked_total_beyond_limit_is_a_data_error() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        // Totals at the bound are fine, on both sides.
        stacked(
            &mut a,
            &["a", "b"],
            &[&[6e299, -6e299], &[f64::NAN, -4e299], &[4e299, 0.0]],
        )
        .unwrap();
        // The positive side of the second category.
        assert_data(
            stacked(&mut a, &["a", "b"], &[&[1.0, 6e299], &[-1e300, 5e299]]),
            STACKED,
            "the values of categories[1] (\"b\") stack to 1.1e300; each stack must stay within 1e300 on both sides of 0.",
        );
        // The negative side; a positive height does not offset it.
        assert_data(
            stacked(&mut a, &["a"], &[&[-1e300], &[5.0], &[-1e300]]),
            STACKED,
            "the values of categories[0] (\"a\") stack to -2e300",
        );
        // Grouped bars do not add up, so they have no such limit.
        let mut h = figure();
        let mut c = h.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        grouped(&mut c, &["a"], &[&[1e300], &[1e300]]).unwrap();
        assert_eq!(a.marks().len(), 1);
        assert_eq!(c.marks().len(), 1);
    }

    #[test]
    fn bar_groups_store_their_input_and_share_it_with_clones() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        let fixed = crate::ColorSpec::Fixed(crate::Rgba::opaque(1, 2, 3));
        let gap = f64::from_bits(0xfff8_0000_0000_0001);
        let input = vec![
            Series {
                label: "first".into(),
                values: vec![-0.0, gap],
                color: fixed,
            },
            series(&[2.0, -1.0]),
        ];
        a.grouped_bar(names(&["b", "a"]), input.clone())
            .unwrap()
            .stacked_bar(names(&["c"]), vec![series(&[1.0])])
            .unwrap();
        let copy = f.clone();
        let (
            Some([Mark::GroupedBar(group), Mark::StackedBar(stack)]),
            Some([Mark::GroupedBar(cloned), _]),
        ) = (
            f.panels().next().map(Axes::marks),
            copy.panels().next().map(Axes::marks),
        )
        else {
            panic!("expected a grouped and a stacked mark");
        };
        assert_eq!(group.categories(), names(&["b", "a"]));
        assert_eq!(group.series().len(), 2);
        let bits = |v: &[f64]| v.iter().map(|h| h.to_bits()).collect::<Vec<_>>();
        for (view, given) in group.series().zip(&input) {
            assert_eq!(view.label(), given.label);
            assert_eq!(bits(view.values()), bits(&given.values));
            assert_eq!(view.color(), given.color);
        }
        assert_eq!(stack.categories(), names(&["c"]));
        assert!(group.shares_data_with(cloned));
    }

    #[test]
    fn failed_bar_group_calls_change_nothing() {
        let mut f = figure();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .grouped_bar(names(&["a"]), vec![series(&[1.0])])
            .unwrap();
        let before = f.clone();
        let mut a = f.single_axes_mut();
        assert!(grouped(&mut a, &["a"], &[]).is_err());
        assert!(grouped(&mut a, &["a"], &[&[1.0], &[]]).is_err());
        assert!(stacked(&mut a, &["a", "a"], &[&[1.0, 1.0]]).is_err());
        assert!(stacked(&mut a, &["a"], &[&[1e300], &[1e300]]).is_err());
        assert!(grouped(&mut a, &["b"], &[&[f64::INFINITY]]).is_err());
        assert_eq!(f, before);
    }

    fn hist(a: &mut AxesMut<'_>, values: &[f64], bins: i64) -> Result<()> {
        let options = HistOptions {
            bins,
            ..HistOptions::default()
        };
        a.hist(values, options).map(|_| ())
    }

    const HIST: &str = "Axes.hist()";

    #[test]
    fn hist_bin_count_outside_its_range_is_a_style_error_naming_bins() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        for (bins, shown) in [(0, "0"), (10_001, "10001")] {
            // Before anything else, even an infinite value on a log x axis.
            match hist(&mut a, &[f64::INFINITY], bins) {
                Err(Error::Style {
                    context, message, ..
                }) => {
                    assert_eq!(context, HIST);
                    assert_eq!(
                        message,
                        format!("bins is {shown}; bins must be between 1 and 10000.")
                    );
                }
                other => panic!("expected a Style error, got {other:?}"),
            }
        }
        assert!(a.marks().is_empty());
        hist(&mut a, &[1.0], 1).unwrap();
        hist(&mut a, &[1.0], 10_000).unwrap();
        assert_eq!(a.marks().len(), 2);
        let mut g = figure();
        let mut b = g.add_axes(log_kind(), ScaleKind::Linear).unwrap();
        assert_style(hist(&mut b, &[f64::INFINITY], 0), HIST);
    }

    #[test]
    fn negative_bins_are_a_style_error() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        for bins in [-1, i64::MIN] {
            let err = hist(&mut a, &[1.0], bins).unwrap_err();
            assert!(matches!(err, Error::Style { .. }));
            assert_eq!(
                err.to_string(),
                format!("{HIST}: bins is {bins}; bins must be between 1 and 10000.")
            );
            assert_eq!(err.parameter(), Some("bins"));
            assert_eq!(err.hint(), Some("use at least one bin."));
        }
        // Above the range there is no hint to give.
        assert_eq!(hist(&mut a, &[1.0], 10_001).unwrap_err().hint(), None);
        assert!(a.marks().is_empty());
    }

    #[test]
    fn hist_on_a_log_x_axis_is_a_usage_error() {
        let mut f = figure();
        let mut a = f.add_axes(log_kind(), ScaleKind::Linear).unwrap();
        // Before the values are checked.
        match hist(&mut a, &[-1.0, f64::INFINITY], 30) {
            Err(Error::Usage {
                context, message, ..
            }) => {
                assert_eq!(context, HIST);
                assert_eq!(
                    message,
                    "the x axis is a log axis, which histograms do not support; use a linear x axis."
                );
            }
            other => panic!("expected a Usage error, got {other:?}"),
        }
        assert!(a.marks().is_empty());
    }

    #[test]
    fn hist_checks_run_in_order() {
        // A category x axis, before the values.
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        assert_data(
            hist(&mut a, &[f64::INFINITY], 30),
            HIST,
            "values holds numbers but the x axis is a category axis; use a linear x axis.",
        );
        assert!(a.marks().is_empty());

        // The first bad value, by index; NaN is allowed.
        let mut g = figure();
        let mut b = g.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            hist(&mut b, &[1.0, f64::NAN, -1e301, f64::INFINITY], 30),
            HIST,
            "values[2] is -1e301; magnitudes must not exceed 1e300.",
        );
        assert_data(
            hist(&mut b, &[1.0, f64::NEG_INFINITY], 30),
            HIST,
            "values[1] is -inf; values must be finite",
        );
        assert!(b.marks().is_empty());

        // A log y axis is fine: the counts are positive, whatever the values.
        let mut k = figure();
        let mut d = k.add_axes(ScaleKind::Linear, log_kind()).unwrap();
        hist(&mut d, &[-3.0, 0.0, 1e300, -1e300], 30).unwrap();
        assert_eq!(d.marks().len(), 1);
    }

    #[test]
    fn hist_stores_its_input_and_shares_it_with_clones() {
        let mut f = figure();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let values = [-0.0, f64::from_bits(0x7ff8_0000_0000_0001), 2.5, 1e300];
        let options = HistOptions {
            bins: 7,
            label: "latency".to_owned(),
            color: crate::ColorSpec::Fixed(crate::Rgba::opaque(1, 2, 3)),
        };
        // Empty and all-NaN values are allowed.
        a.hist(&[], HistOptions::default())
            .unwrap()
            .hist(&[f64::NAN], HistOptions::default())
            .unwrap()
            .hist(&values, options.clone())
            .unwrap();
        let copy = f.clone();
        let (Some([_, _, Mark::Hist(hist)]), Some([_, _, Mark::Hist(cloned)])) = (
            f.panels().next().map(Axes::marks),
            copy.panels().next().map(Axes::marks),
        ) else {
            panic!("expected three histogram marks");
        };
        let bits = |v: &[f64]| v.iter().map(|h| h.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(hist.values()), bits(&values));
        assert_eq!(hist.options(), options);
        assert!(hist.shares_data_with(cloned));
    }

    #[test]
    fn failed_hist_calls_change_nothing() {
        let mut f = figure();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .hist(&[1.0, 2.0], HistOptions::default())
            .unwrap();
        let before = f.clone();
        let mut a = f.single_axes_mut();
        assert!(hist(&mut a, &[1.0], 0).is_err());
        assert!(hist(&mut a, &[1.0], 10_001).is_err());
        assert!(hist(&mut a, &[f64::INFINITY], 30).is_err());
        assert_eq!(f, before);
    }
}
