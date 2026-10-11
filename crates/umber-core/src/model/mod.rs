//! The figure model: a [`Figure`] holds one [`Axes`], which holds [`Mark`]s.
//!
//! The model is data only. Every input is validated at the call that receives it, so a model
//! that exists is always drawable. Marks hold copies of their data that never change, so clones
//! of a figure share them. Explicit limits are kept as the scales they define. Colors stay as
//! [`ColorSpec`](crate::ColorSpec)s; resolving them and drawing are later stages that read the
//! model.

mod axes;
mod axis_data;
mod bar;
mod figure;
mod hist;
mod mark;
mod validation;

pub use axes::Axes;
pub use axis_data::AxisData;
pub use bar::{BarGroupMark, BarMark, BarOptions, BarSeries, BarSeriesView};
pub use figure::Figure;
pub use hist::{HistMark, HistOptions};
pub use mark::{AreaMark, AreaOptions, LineMark, LineOptions, Mark, ScatterMark, ScatterOptions};

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
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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
        let a = f.add_axes(log_kind(), log_kind()).unwrap();
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
        let a = f.add_axes(log_kind(), log_kind()).unwrap();
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
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
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
        let b = g.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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

    #[test]
    fn numeric_y_on_a_category_y_axis_is_a_data_error() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
        assert_data(
            a.line(nums(&[0.0]), &[1.0], LineOptions::default()),
            "Axes.line()",
            "y axis is a category axis",
        );
    }

    #[test]
    fn invalid_limits_are_style_errors() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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
        let a = f.add_axes(log_kind(), log_kind()).unwrap();
        assert_style(a.xlim(0.0, 10.0), "Axes.xlim()");
        assert_style(a.xlim(-5.0, 10.0), "Axes.xlim()");
        assert_style(a.ylim(1e-301, 10.0), "Axes.ylim()");
        assert_style(a.ylim(1.0, 1e301), "Axes.ylim()");
        assert!(a.xlim(1e-300, 1e300).is_ok());
    }

    #[test]
    fn limits_on_a_category_axis_are_usage_errors() {
        let mut f = figure();
        let a = f
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap();
        assert_usage(a.xlim(0.0, 1.0), "Axes.xlim()");
        assert_usage(a.ylim(0.0, 1.0), "Axes.ylim()");
        // Usage wins over invalid values.
        assert_usage(a.xlim(1.0, 0.0), "Axes.xlim()");
    }

    #[test]
    fn nan_is_allowed_in_lines_and_scatters() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let mut y = vec![1.0, 2.0];
        let x = nums(&[0.0, 1.0]);
        a.line(x.clone(), &y, LineOptions::default()).unwrap();
        y[0] = 99.0;
        match a.marks() {
            [Mark::Line(line)] => {
                assert_eq!(line.y(), &[1.0, 2.0]);
                assert_eq!(line.x(), &x);
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
        let a = f.axes_mut().unwrap();
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
        let a = f.axes().unwrap();
        assert_eq!(a.x_limits(), Some((-1.0, 2.0)));
        assert_eq!(a.y_limits(), Some((0.5, 50.0)));
        assert_eq!(a.marks().len(), 1);
    }

    #[test]
    fn explicit_limits_keep_the_scale_they_define() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, log_kind()).unwrap();
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
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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

    fn bars(a: &mut Axes, categories: &[&str], heights: &[f64]) -> Result<()> {
        a.bar(names(categories), heights, BarOptions::default())
            .map(|_| ())
    }

    #[test]
    fn bar_checks_run_in_order() {
        const BAR: &str = "Axes.bar()";
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        // Lengths first, whatever else is wrong.
        assert_data(
            bars(a, &["a", "a"], &[f64::INFINITY]),
            BAR,
            "categories has 2 values but heights has 1; categories and heights must have the same length.",
        );
        // Duplicates before heights; the message names both indices.
        assert_data(
            bars(a, &["a", "b", "a"], &[f64::INFINITY, 1.0, 2.0]),
            BAR,
            "categories[2] repeats \"a\" from categories[0]; each category must appear once.",
        );
        // The first bad height, by index.
        assert_data(
            bars(
                a,
                &["a", "b", "c", "d"],
                &[1.0, f64::NAN, 1e301, f64::INFINITY],
            ),
            BAR,
            "heights[2] is 1e301; magnitudes must not exceed 1e300.",
        );
        assert_data(
            bars(a, &["a", "b", "c", "d"], &[1.0, 2.0, 3.0, f64::INFINITY]),
            BAR,
            "heights[3] is inf; values must be finite",
        );
        assert!(a.marks().is_empty());

        // The x axis kind before duplicates.
        let mut g = figure();
        let b = g.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            bars(b, &["a", "a"], &[1.0, 2.0]),
            BAR,
            "categories holds names but the x axis is not a category axis; use a category x axis.",
        );

        // Heights before the y axis kind.
        let mut h = figure();
        let c = h
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap();
        assert_data(
            bars(c, &["a"], &[f64::NEG_INFINITY]),
            BAR,
            "heights[0] is -inf",
        );
        assert_data(
            bars(c, &["a"], &[1.0]),
            BAR,
            "heights holds numbers but the y axis is a category axis; use a linear or log y axis.",
        );
        assert!(c.marks().is_empty());
    }

    #[test]
    fn bars_on_a_log_y_axis_need_positive_heights() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Category, log_kind()).unwrap();
        assert_data(
            bars(a, &["a", "b"], &[2.0, 0.0]),
            "Axes.bar()",
            "heights[1] is 0 on a log axis; values must be positive.",
        );
        assert_data(
            bars(a, &["a"], &[1e-301]),
            "Axes.bar()",
            "heights[0] is 1e-301 on a log axis",
        );
        assert!(a.marks().is_empty());
        bars(a, &["a", "b"], &[2.0, f64::NAN]).unwrap();
        assert_eq!(a.marks().len(), 1);
    }

    #[test]
    fn bars_store_their_input_and_share_it_with_clones() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
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
        let (Some([_, Mark::Bar(bar)]), Some([_, Mark::Bar(cloned)])) =
            (f.axes().map(Axes::marks), copy.axes().map(Axes::marks))
        else {
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
        let a = f.axes_mut().unwrap();
        assert!(bars(a, &["a"], &[]).is_err());
        assert!(bars(a, &["a", "a"], &[1.0, 1.0]).is_err());
        assert!(bars(a, &["b"], &[f64::INFINITY]).is_err());
        assert_eq!(f, before);
    }

    fn series(heights: &[f64]) -> BarSeries {
        BarSeries {
            heights: heights.to_vec(),
            ..BarSeries::default()
        }
    }

    fn grouped(a: &mut Axes, categories: &[&str], heights: &[&[f64]]) -> Result<()> {
        let list = heights.iter().map(|h| series(h)).collect();
        a.grouped_bar(names(categories), list).map(|_| ())
    }

    fn stacked(a: &mut Axes, categories: &[&str], heights: &[&[f64]]) -> Result<()> {
        let list = heights.iter().map(|h| series(h)).collect();
        a.stacked_bar(names(categories), list).map(|_| ())
    }

    const GROUPED: &str = "Axes.grouped_bar()";
    const STACKED: &str = "Axes.stacked_bar()";

    #[test]
    fn an_empty_series_list_is_a_data_error() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        // Before anything else, even with no categories.
        assert_data(
            grouped(a, &["a", "a"], &[]),
            GROUPED,
            "series is empty; pass at least one series.",
        );
        assert_data(stacked(a, &[], &[]), STACKED, "series is empty");
        assert!(a.marks().is_empty());
    }

    #[test]
    fn a_series_length_mismatch_names_the_series() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        for (context, call) in [
            (
                GROUPED,
                grouped as fn(&mut Axes, &[&str], &[&[f64]]) -> Result<()>,
            ),
            (STACKED, stacked),
        ] {
            assert_data(
                call(
                    a,
                    &["a", "b"],
                    &[&[1.0, 2.0], &[1.0, 2.0], &[f64::INFINITY]],
                ),
                context,
                "categories has 2 values but series[2].heights has 1; each series must have one height per category.",
            );
            // The first mismatched series, before duplicates and bad heights.
            assert_data(
                call(a, &["a", "a"], &[&[f64::INFINITY], &[1.0, 2.0, 3.0]]),
                context,
                "series[0].heights has 1",
            );
        }
        assert!(a.marks().is_empty());
    }

    #[test]
    fn grouped_and_stacked_checks_run_in_order() {
        for (context, call) in [
            (
                GROUPED,
                grouped as fn(&mut Axes, &[&str], &[&[f64]]) -> Result<()>,
            ),
            (STACKED, stacked),
        ] {
            // The x axis kind before duplicates.
            let mut f = figure();
            let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            assert_data(
                call(a, &["a", "a"], &[&[1.0, 2.0]]),
                context,
                "categories holds names but the x axis is not a category axis; use a category x axis.",
            );

            let mut f = figure();
            let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
            // Duplicates before heights.
            assert_data(
                call(a, &["a", "b", "a"], &[&[f64::INFINITY; 3]]),
                context,
                "categories[2] repeats \"a\" from categories[0]; each category must appear once.",
            );
            // Heights in series order; the message names the series and the height.
            assert_data(
                call(
                    a,
                    &["a", "b"],
                    &[&[1.0, f64::NAN], &[2.0, 1e301], &[f64::INFINITY, 0.0]],
                ),
                context,
                "series[1].heights[1] is 1e301; magnitudes must not exceed 1e300.",
            );
            assert!(a.marks().is_empty());

            // Heights before the y axis kind.
            let mut f = figure();
            let a = f
                .add_axes(ScaleKind::Category, ScaleKind::Category)
                .unwrap();
            assert_data(
                call(a, &["a"], &[&[f64::NEG_INFINITY]]),
                context,
                "series[0].heights[0] is -inf",
            );
            assert_data(
                call(a, &["a"], &[&[1.0]]),
                context,
                "the series hold numbers but the y axis is a category axis; use a linear or log y axis.",
            );
            assert!(a.marks().is_empty());

            // A log y axis needs positive heights.
            let mut f = figure();
            let a = f.add_axes(ScaleKind::Category, log_kind()).unwrap();
            assert_data(
                call(a, &["a", "b"], &[&[1.0, 2.0], &[3.0, -1.0]]),
                context,
                "series[1].heights[1] is -1 on a log axis; values must be positive.",
            );
            call(a, &["a", "b"], &[&[1.0, f64::NAN], &[1e300, 2.0]]).unwrap();
            assert_eq!(a.marks().len(), 1);
        }
    }

    #[test]
    fn stacked_total_beyond_limit_is_a_data_error() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        // Totals at the bound are fine, on both sides.
        stacked(
            a,
            &["a", "b"],
            &[&[6e299, -6e299], &[f64::NAN, -4e299], &[4e299, 0.0]],
        )
        .unwrap();
        // The positive side of the second category.
        assert_data(
            stacked(a, &["a", "b"], &[&[1.0, 6e299], &[-1e300, 5e299]]),
            STACKED,
            "the heights of categories[1] (\"b\") stack to 1.1e300; each stack must stay within 1e300 on both sides of 0.",
        );
        // The negative side; a positive height does not offset it.
        assert_data(
            stacked(a, &["a"], &[&[-1e300], &[5.0], &[-1e300]]),
            STACKED,
            "the heights of categories[0] (\"a\") stack to -2e300",
        );
        // Totals are checked before the y axis kind.
        let mut g = figure();
        let b = g
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap();
        assert_data(
            stacked(b, &["a"], &[&[1e300], &[1e300]]),
            STACKED,
            "stack to 2e300",
        );
        // Grouped bars do not add up, so they have no such limit.
        let mut h = figure();
        let c = h.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        grouped(c, &["a"], &[&[1e300], &[1e300]]).unwrap();
        assert_eq!(a.marks().len(), 1);
        assert_eq!(c.marks().len(), 1);
    }

    #[test]
    fn bar_groups_store_their_input_and_share_it_with_clones() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        let fixed = crate::ColorSpec::Fixed(crate::Rgba::opaque(1, 2, 3));
        let gap = f64::from_bits(0xfff8_0000_0000_0001);
        let input = vec![
            BarSeries {
                label: "first".into(),
                heights: vec![-0.0, gap],
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
        ) = (f.axes().map(Axes::marks), copy.axes().map(Axes::marks))
        else {
            panic!("expected a grouped and a stacked mark");
        };
        assert_eq!(group.categories(), names(&["b", "a"]));
        assert_eq!(group.series().len(), 2);
        let bits = |v: &[f64]| v.iter().map(|h| h.to_bits()).collect::<Vec<_>>();
        for (view, given) in group.series().zip(&input) {
            assert_eq!(view.label(), given.label);
            assert_eq!(bits(view.heights()), bits(&given.heights));
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
        let a = f.axes_mut().unwrap();
        assert!(grouped(a, &["a"], &[]).is_err());
        assert!(grouped(a, &["a"], &[&[1.0], &[]]).is_err());
        assert!(stacked(a, &["a", "a"], &[&[1.0, 1.0]]).is_err());
        assert!(stacked(a, &["a"], &[&[1e300], &[1e300]]).is_err());
        assert!(grouped(a, &["b"], &[&[f64::INFINITY]]).is_err());
        assert_eq!(f, before);
    }

    fn hist(a: &mut Axes, values: &[f64], bins: i64) -> Result<()> {
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
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        for (bins, shown) in [(0, "0"), (10_001, "10001")] {
            // Before anything else, even an infinite value on a log x axis.
            match hist(a, &[f64::INFINITY], bins) {
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
        hist(a, &[1.0], 1).unwrap();
        hist(a, &[1.0], 10_000).unwrap();
        assert_eq!(a.marks().len(), 2);
        let mut g = figure();
        let b = g.add_axes(log_kind(), ScaleKind::Linear).unwrap();
        assert_style(hist(b, &[f64::INFINITY], 0), HIST);
    }

    #[test]
    fn negative_bins_are_a_style_error() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        for bins in [-1, i64::MIN] {
            let err = hist(a, &[1.0], bins).unwrap_err();
            assert!(matches!(err, Error::Style { .. }));
            assert_eq!(
                err.to_string(),
                format!("{HIST}: bins is {bins}; bins must be between 1 and 10000.")
            );
            assert_eq!(err.parameter(), Some("bins"));
            assert_eq!(err.hint(), Some("use at least one bin."));
        }
        // Above the range there is no hint to give.
        assert_eq!(hist(a, &[1.0], 10_001).unwrap_err().hint(), None);
        assert!(a.marks().is_empty());
    }

    #[test]
    fn hist_on_a_log_x_axis_is_a_usage_error() {
        let mut f = figure();
        let a = f.add_axes(log_kind(), ScaleKind::Linear).unwrap();
        // Before the values are checked.
        match hist(a, &[-1.0, f64::INFINITY], 30) {
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
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        assert_data(
            hist(a, &[f64::INFINITY], 30),
            HIST,
            "values holds numbers but the x axis is a category axis; use a linear x axis.",
        );
        assert!(a.marks().is_empty());

        // The first bad value, by index; NaN is allowed.
        let mut g = figure();
        let b = g.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            hist(b, &[1.0, f64::NAN, -1e301, f64::INFINITY], 30),
            HIST,
            "values[2] is -1e301; magnitudes must not exceed 1e300.",
        );
        assert_data(
            hist(b, &[1.0, f64::NEG_INFINITY], 30),
            HIST,
            "values[1] is -inf; values must be finite",
        );
        assert!(b.marks().is_empty());

        // Values before the y axis kind.
        let mut h = figure();
        let c = h.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
        assert_data(hist(c, &[f64::INFINITY], 30), HIST, "values[0] is inf");
        assert_data(
            hist(c, &[1.0], 30),
            HIST,
            "a histogram counts values but the y axis is a category axis; use a linear or log y axis.",
        );
        assert!(c.marks().is_empty());

        // A log y axis is fine: the counts are positive, whatever the values.
        let mut k = figure();
        let d = k.add_axes(ScaleKind::Linear, log_kind()).unwrap();
        hist(d, &[-3.0, 0.0, 1e300, -1e300], 30).unwrap();
        assert_eq!(d.marks().len(), 1);
    }

    #[test]
    fn hist_stores_its_input_and_shares_it_with_clones() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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
        let (Some([_, _, Mark::Hist(hist)]), Some([_, _, Mark::Hist(cloned)])) =
            (f.axes().map(Axes::marks), copy.axes().map(Axes::marks))
        else {
            panic!("expected three histogram marks");
        };
        let bits = |v: &[f64]| v.iter().map(|h| h.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(hist.values()), bits(&values));
        assert_eq!(hist.bins(), 7);
        assert_eq!((hist.label(), hist.color()), ("latency", options.color));
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
        let a = f.axes_mut().unwrap();
        assert!(hist(a, &[1.0], 0).is_err());
        assert!(hist(a, &[1.0], 10_001).is_err());
        assert!(hist(a, &[f64::INFINITY], 30).is_err());
        assert_eq!(f, before);
    }
}
