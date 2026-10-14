//! Public box marks, immutable shared samples and atomic validation.

use std::sync::Arc;
use umber_core::{
    BoxOptions, ColorSpec, Distribution, Error, Figure, Grid, GridIndex, LogBase, Mark, Placement,
    ScaleKind, Theme,
};

const LIN: ScaleKind = ScaleKind::Linear;
const CAT: ScaleKind = ScaleKind::Category;
const LOG: ScaleKind = ScaleKind::Log { base: LogBase::TEN };

#[test]
fn box_defaults_and_shared_read_only_views() {
    assert_eq!(BoxOptions::default().color, ColorSpec::AUTO);
    assert_eq!(Distribution::default().color, ColorSpec::AUTO);
    assert!(Distribution::default().groups.is_empty());
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        assert_eq!(theme.box_width(), 0.5);
        assert_eq!(theme.with_line_width(8.0).unwrap().box_width(), 0.5);
    }
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut ax = f.add_axes(CAT, LIN).unwrap();
    let input = vec![3.0, -0.0, f64::NAN, 1.0];
    ax.box_plot(vec!["A".into()], vec![input.clone()], BoxOptions::default())
        .unwrap();
    ax.grouped_box_plot(
        vec!["A".into()],
        vec![Distribution {
            label: "North".into(),
            groups: vec![input.clone()],
            ..Default::default()
        }],
    )
    .unwrap();
    let cloned = f.clone();
    let marks = |f: &Figure| {
        f.axes_at(GridIndex::At(0), GridIndex::At(0))
            .unwrap()
            .marks()
            .to_vec()
    };
    let original = marks(&f);
    let copied = marks(&cloned);
    let [Mark::Box(a), Mark::GroupedBox(b)] = original.as_slice() else {
        panic!("boxes expected")
    };
    let [Mark::Box(c), Mark::GroupedBox(d)] = copied.as_slice() else {
        panic!("boxes expected")
    };
    assert_eq!(a.categories(), ["A"]);
    assert_eq!(b.categories(), ["A"]);
    assert_eq!(a.options(), &BoxOptions::default());
    let bits = |values: &[f64]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a.groups()[0]), bits(&input));
    assert!(Arc::ptr_eq(&a.groups()[0], &c.groups()[0]));
    assert!(std::ptr::eq(a.categories(), c.categories()));
    assert_eq!(b.distributions().len(), 1);
    let view = b.distributions().next().unwrap();
    let copy = d.distributions().next().unwrap();
    assert_eq!(view.label(), "North");
    assert_eq!(view.color(), ColorSpec::AUTO);
    assert_eq!(bits(&view.groups()[0]), bits(&input));
    assert!(Arc::ptr_eq(&view.groups()[0], &copy.groups()[0]));
}

#[test]
fn box_validation_is_exact_and_atomic_across_grid_panels() {
    let cases = [
        (
            LIN,
            vec![],
            "groups has 0 groups; categories has 1.",
            Some("pass one group per category, using an empty group when there are no samples."),
        ),
        (
            LIN,
            vec![vec![f64::INFINITY]],
            "groups[0][0] is inf; values must be finite.",
            Some("use NaN to leave a sample out."),
        ),
        (
            LIN,
            vec![vec![1e301]],
            "groups[0][0] is 1e301; magnitudes must not exceed 1e300.",
            None,
        ),
        (
            LOG,
            vec![vec![0.0]],
            "groups[0][0] is 0 on a log axis; values must be positive.",
            Some("use a linear axis, or remove values that are not positive."),
        ),
        (
            LOG,
            vec![vec![1e-310]],
            "groups[0][0] is 1e-310 on a log axis; values must be at least 1e-300.",
            Some("use a linear axis, or rescale the values."),
        ),
    ];
    for (y, groups, message, hint) in cases {
        let mut f =
            Figure::with_grid(8.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
        for col in 0..2 {
            f.add_axes_at(
                Placement {
                    row: GridIndex::At(0),
                    col: GridIndex::At(col),
                    row_span: 1,
                    col_span: 1,
                },
                CAT,
                y,
            )
            .unwrap()
            .box_plot(
                vec!["A".into()],
                vec![vec![1.0, 2.0]],
                BoxOptions::default(),
            )
            .unwrap();
        }
        let before = f.clone();
        let err = f
            .axes_at_mut(GridIndex::At(0), GridIndex::At(1))
            .unwrap()
            .box_plot(vec!["A".into()], groups, BoxOptions::default())
            .unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(err.to_string(), format!("Axes.box(): {message}"));
        assert_eq!((err.parameter(), err.hint()), (Some("groups"), hint));
        assert_eq!(f, before);
    }
}

#[test]
fn boxes_require_category_x_before_checking_samples() {
    for x in [LIN, LOG] {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(x, LIN).unwrap();
        let before = f.clone();
        for grouped in [false, true] {
            let mut ax = f.axes_at_mut(GridIndex::At(0), GridIndex::At(0)).unwrap();
            let err = if grouped {
                ax.grouped_box_plot(vec![], vec![]).unwrap_err()
            } else {
                ax.box_plot(vec!["A".into()], vec![], BoxOptions::default())
                    .unwrap_err()
            };
            assert!(matches!(err, Error::Usage { .. }));
            let method = if grouped { "grouped_box" } else { "box" };
            assert_eq!(
                err.to_string(),
                format!(
                    "Axes.{method}(): the x axis is not a category axis; box plots need a category x axis."
                )
            );
            assert_eq!(err.parameter(), Some("categories"));
            assert_eq!(
                err.hint(),
                Some("pass x=Scale.category() to Figure.add_axes().")
            );
            assert_eq!(f, before);
        }
    }
}

#[test]
fn grouped_boxes_validate_every_distribution_before_mutation() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(CAT, LIN).unwrap();
    let before = f.clone();
    for (distributions, message) in [
        (
            vec![],
            "distributions is empty; pass at least one Distribution.",
        ),
        (
            vec![Distribution::default()],
            "distributions[0].groups has 0 groups; categories has 1.",
        ),
        (
            vec![
                Distribution {
                    groups: vec![vec![1.0]],
                    ..Default::default()
                },
                Distribution {
                    groups: vec![vec![f64::NEG_INFINITY]],
                    ..Default::default()
                },
            ],
            "distributions[1].groups[0][0] is -inf; values must be finite.",
        ),
    ] {
        let err = f
            .axes_at_mut(GridIndex::At(0), GridIndex::At(0))
            .unwrap()
            .grouped_box_plot(vec!["A".into()], distributions)
            .unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(err.to_string(), format!("Axes.grouped_box(): {message}"));
        assert_eq!(err.parameter(), Some("distributions"));
        assert_eq!(f, before);
    }
    let err = f
        .axes_at_mut(GridIndex::At(0), GridIndex::At(0))
        .unwrap()
        .box_plot(
            vec!["A".into(), "A".into()],
            vec![vec![], vec![]],
            BoxOptions::default(),
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.box(): categories[1] repeats \"A\" from categories[0]; each category must appear once."
    );
    assert_eq!(err.parameter(), Some("categories"));
    assert_eq!(
        err.hint(),
        Some("combine the samples of repeated categories.")
    );
    assert_eq!(f, before);
}
