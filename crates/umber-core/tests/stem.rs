//! Stem data, options, shared storage and atomic validation through the public API.

use std::sync::Arc;

use umber_core::{
    AxisData, ColorSpec, Error, Figure, Grid, GridIndex, LogBase, Mark, Marker, Placement,
    ScaleKind, SizeSpec, StemOptions, Theme,
};

const LIN: ScaleKind = ScaleKind::Linear;
const LOG: ScaleKind = ScaleKind::Log { base: LogBase::TEN };

fn nums(values: &[f64]) -> AxisData {
    AxisData::Numbers(values.to_vec())
}

fn names() -> AxisData {
    AxisData::Categories(vec!["a".into(), "b".into()])
}

#[test]
fn stem_defaults_and_theme_widths() {
    let options = StemOptions::default();
    assert!(options.label.is_empty());
    assert_eq!(options.color, ColorSpec::AUTO);
    assert_eq!(options.marker, Marker::Circle);
    assert_eq!(options.size, SizeSpec::Auto);
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        assert_eq!(theme.stem_width(), 1.5);
        assert_eq!(theme.with_line_width(8.0).unwrap().stem_width(), 1.5);
    }
}

#[test]
fn stem_keeps_unsorted_values_options_and_shared_storage() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let gap = f64::from_bits(0xfff8_0000_0000_0001);
    let x = vec![3.0, -0.0, gap, 1.0];
    let y: Arc<[f64]> = Arc::from([gap, 1.0, -0.0, 4.0]);
    let options = StemOptions {
        label: "Response".into(),
        marker: Marker::Diamond,
        size: SizeSpec::pt(8.0).unwrap(),
        ..StemOptions::default()
    };
    figure
        .add_axes(LIN, LIN)
        .unwrap()
        .stem_shared(nums(&x), Arc::clone(&y), options.clone())
        .unwrap();
    let axes = figure.axes_at(GridIndex::At(0), GridIndex::At(0)).unwrap();
    let [Mark::Stem(mark)] = axes.marks() else {
        panic!("one stem expected")
    };
    let AxisData::Numbers(stored) = mark.x() else {
        panic!("numeric x expected")
    };
    let bits = |v: &[f64]| v.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(stored), bits(&x));
    assert_eq!(bits(mark.y()), bits(&y));
    assert!(std::ptr::eq(mark.y(), &*y));
    assert_eq!(mark.options(), &options);
    assert!(format!("{mark:?}").starts_with("StemMark"));
    let cloned = figure.clone();
    let [Mark::Stem(copy)] = cloned
        .axes_at(GridIndex::At(0), GridIndex::At(0))
        .unwrap()
        .marks()
    else {
        panic!("one stem expected")
    };
    assert!(std::ptr::eq(mark.y(), copy.y()));
}

#[test]
fn stem_validation_matches_scatter_and_leaves_the_grid_unchanged() {
    let point_hint = Some("use NaN to leave a point out.");
    let positive = Some("use a linear axis, or remove values that are not positive.");
    let rescale = Some("use a linear axis, or rescale the values.");
    let cases = [
        (
            LIN,
            LIN,
            nums(&[0.0, 1.0]),
            vec![1.0],
            "x has 2 values but y has 1; x and y must have the same length.",
            None,
            None,
        ),
        (
            LIN,
            LIN,
            nums(&[0.0, f64::NEG_INFINITY]),
            vec![1.0, 2.0],
            "x[1] is -inf; values must be finite.",
            Some("x"),
            point_hint,
        ),
        (
            LIN,
            LIN,
            nums(&[0.0, 1.0]),
            vec![1.0, f64::INFINITY],
            "y[1] is inf; values must be finite.",
            Some("y"),
            point_hint,
        ),
        (
            LIN,
            LIN,
            nums(&[0.0, 1e301]),
            vec![1.0, 2.0],
            "x[1] is 1e301; magnitudes must not exceed 1e300.",
            Some("x"),
            None,
        ),
        (
            LIN,
            LIN,
            nums(&[0.0, 1.0]),
            vec![1.0, -1e301],
            "y[1] is -1e301; magnitudes must not exceed 1e300.",
            Some("y"),
            None,
        ),
        (
            LOG,
            LIN,
            nums(&[1.0, 0.0]),
            vec![1.0, 2.0],
            "x[1] is 0 on a log axis; values must be positive.",
            Some("x"),
            positive,
        ),
        (
            LIN,
            LOG,
            nums(&[0.0, 1.0]),
            vec![1.0, -1.0],
            "y[1] is -1 on a log axis; values must be positive.",
            Some("y"),
            positive,
        ),
        (
            LOG,
            LIN,
            nums(&[1.0, 1e-310]),
            vec![1.0, 2.0],
            "x[1] is 1e-310 on a log axis; values must be at least 1e-300.",
            Some("x"),
            rescale,
        ),
        (
            LIN,
            LOG,
            nums(&[0.0, 1.0]),
            vec![1.0, 1e-310],
            "y[1] is 1e-310 on a log axis; values must be at least 1e-300.",
            Some("y"),
            rescale,
        ),
        (
            LIN,
            LIN,
            names(),
            vec![1.0, 2.0],
            "x holds category names but the x axis is not a category axis; pass numbers.",
            None,
            Some("pass x=Scale.category() to Figure.add_axes()."),
        ),
        (
            ScaleKind::Category,
            LIN,
            nums(&[0.0, 1.0]),
            vec![1.0, 2.0],
            "x holds numbers but the x axis is a category axis; pass category names.",
            None,
            Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."),
        ),
    ];
    for (x_kind, y_kind, x, y, message, parameter, hint) in cases {
        let mut f =
            Figure::with_grid(8.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
        for col in 0..2 {
            let data = if x_kind == ScaleKind::Category {
                names()
            } else {
                nums(&[1.0, 2.0])
            };
            f.add_axes_at(
                Placement {
                    row: GridIndex::At(0),
                    col: GridIndex::At(col),
                    row_span: 1,
                    col_span: 1,
                },
                x_kind,
                y_kind,
            )
            .unwrap()
            .stem(data, &[2.0, 3.0], StemOptions::default())
            .unwrap();
        }
        let before = f.clone();
        let err = f
            .axes_at_mut(GridIndex::At(0), GridIndex::At(1))
            .unwrap()
            .stem(x, &y, StemOptions::default())
            .unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(err.to_string(), format!("Axes.stem(): {message}"));
        assert_eq!((err.parameter(), err.hint()), (parameter, hint));
        assert_eq!(f, before);
    }
}

#[test]
fn stem_checks_lengths_before_values_and_x_before_y() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut ax = f.add_axes(LIN, LIN).unwrap();
    let err = ax
        .stem(nums(&[f64::INFINITY]), &[], StemOptions::default())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.stem(): x has 1 values but y has 0; x and y must have the same length."
    );
    let err = ax
        .stem(
            nums(&[f64::INFINITY]),
            &[f64::INFINITY],
            StemOptions::default(),
        )
        .unwrap_err();
    assert_eq!(err.parameter(), Some("x"));
    assert!(ax.marks().is_empty());
}
