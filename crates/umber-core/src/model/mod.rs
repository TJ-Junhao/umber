//! The figure model: a [`Figure`] holds one [`Axes`], which holds [`Mark`]s.
//!
//! The model is data only. Every input is validated at the call that receives it, so a model
//! that exists is always drawable. Marks hold copies of their data that never change, so clones
//! of a figure share them. Explicit limits are kept as the scales they define. Colors stay as
//! [`ColorSpec`](crate::ColorSpec)s; resolving them and drawing are later stages that read the
//! model.

mod axes;
mod axis_data;
mod figure;
mod mark;
mod validation;

pub use axes::Axes;
pub use axis_data::{AxisData, XValues, YValues};
pub use figure::Figure;
pub use mark::{LineOptions, Mark, ScatterOptions};

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
            "x[3] is inf; values must be finite (NaN marks a gap).",
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
            [Mark::Line { x: mx, y: my, .. }] => {
                assert_eq!(&my[..], &[1.0, 2.0]);
                assert_eq!(&**mx, &x);
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
}
