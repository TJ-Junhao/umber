//! Grouped marks validate every group before adding any marks.

use umber_core::{
    AxisData, Dash, EcdfOptions, Figure, LineOptions, Marker, ScaleKind, SizeSpec, Step, Theme,
};

fn groups(bad: bool) -> Vec<(String, AxisData, AxisData)> {
    vec![
        (
            "South".into(),
            AxisData::Numbers(vec![0.0, 1.0]),
            AxisData::Numbers(vec![1.0, 2.0]),
        ),
        (
            "North".into(),
            AxisData::Numbers(vec![0.0, 1.0]),
            AxisData::Numbers(vec![2.0, if bad { f64::INFINITY } else { 3.0 }]),
        ),
    ]
}

#[test]
fn grouped_xy_calls_are_atomic() {
    for method in ["line", "scatter", "step", "area"] {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.line(AxisData::Numbers(vec![0.0]), &[1.0], LineOptions::default())
            .unwrap();
        for bad in [true, false] {
            let result = match method {
                "line" => axes.grouped_line(groups(bad), Dash::Solid, SizeSpec::Auto),
                "scatter" => axes.grouped_scatter(groups(bad), Marker::Circle, SizeSpec::Auto),
                "step" => axes.grouped_step(groups(bad), Step::Post, Dash::Solid, SizeSpec::Auto),
                "area" => axes.grouped_area(groups(bad), Dash::Solid, SizeSpec::Auto),
                _ => unreachable!(),
            };
            assert_eq!(result.is_err(), bad, "{method}");
            assert_eq!(axes.marks().len(), if bad { 1 } else { 3 }, "{method}");
        }
    }
}

#[test]
fn grouped_ecdf_is_atomic_and_matches_separate_calls() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut expected = figure.clone();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    let bad = vec![
        ("South".into(), AxisData::Numbers(vec![1.0, 2.0])),
        ("North".into(), AxisData::Numbers(vec![f64::NAN])),
    ];
    assert!(
        axes.grouped_ecdf(bad, false, Dash::Solid, SizeSpec::Auto)
            .is_err()
    );
    assert!(axes.marks().is_empty());
    let groups = vec![
        ("South".into(), AxisData::Numbers(vec![1.0, 2.0])),
        ("North".into(), AxisData::Numbers(vec![2.0, 4.0])),
    ];
    axes.grouped_ecdf(groups.clone(), true, Dash::Dashed, SizeSpec::Auto)
        .unwrap();
    let mut axes = expected
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    for (label, values) in groups {
        axes.ecdf(
            values,
            EcdfOptions {
                label,
                complementary: true,
                dash: Dash::Dashed,
                ..EcdfOptions::default()
            },
        )
        .unwrap();
    }
    assert_eq!(
        figure.to_svg(umber_core::SvgText::Editable).unwrap(),
        expected.to_svg(umber_core::SvgText::Editable).unwrap()
    );
}

#[test]
fn grouped_step_checks_order_inside_each_group() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    let mut input = groups(false);
    input[1].1 = AxisData::Numbers(vec![2.0, 1.0]);
    assert!(
        axes.grouped_step(input, Step::Mid, Dash::Solid, SizeSpec::Auto)
            .is_err()
    );
    assert!(axes.marks().is_empty());
}
