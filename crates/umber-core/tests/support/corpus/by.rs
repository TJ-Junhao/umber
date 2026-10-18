//! Named groups in first-appearance order, drawn in all built-in themes.

use umber_core::{
    AxisData, Bins, Dash, FontLibrary, GroupedHistOptions, GroupedKdeOptions, HistStat, HistStyle,
    Marker, ScaleKind, Series, SizeSpec, Step,
};

use super::super::{Output, axes, emit_scene, figure, labelled, linear, themes};

fn xy_groups() -> Vec<(String, AxisData, AxisData)> {
    vec![
        (
            "South".into(),
            AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]),
            AxisData::Numbers(vec![1.0, 3.0, f64::NAN, 4.0]),
        ),
        (
            "North".into(),
            AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]),
            AxisData::Numbers(vec![4.0, 2.0, 3.0, 5.0]),
        ),
    ]
}

fn samples() -> Vec<Series> {
    vec![
        Series {
            label: "South".into(),
            values: vec![1.0, 2.0, 2.0, 3.0],
            ..Series::default()
        },
        Series {
            label: "North".into(),
            values: vec![2.0, 4.0, 4.0, 5.0, 5.0, 6.0],
            ..Series::default()
        },
    ]
}

/// Emits each kind of grouped chart, keeping the first group first in the palette and legend.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (theme_name, theme) in themes() {
        for method in ["line", "area", "scatter", "step", "ecdf", "kde"] {
            let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
            labelled(&mut f, "Regional measurements");
            let mut ax = axes(&mut f);
            match method {
                "line" => ax
                    .grouped_line(xy_groups(), Dash::Solid, SizeSpec::Auto)
                    .unwrap(),
                "area" => ax
                    .grouped_area(xy_groups(), Dash::Dashed, SizeSpec::Auto)
                    .unwrap(),
                "scatter" => ax
                    .grouped_scatter(xy_groups(), Marker::Diamond, SizeSpec::Auto)
                    .unwrap(),
                "step" => ax
                    .grouped_step(xy_groups(), Step::Post, Dash::Solid, SizeSpec::Auto)
                    .unwrap(),
                "ecdf" => ax
                    .grouped_ecdf(
                        samples()
                            .into_iter()
                            .map(|s| (s.label, AxisData::Numbers(s.values)))
                            .collect(),
                        false,
                        Dash::Solid,
                        SizeSpec::Auto,
                    )
                    .unwrap(),
                "kde" => ax
                    .grouped_kde(samples(), GroupedKdeOptions::default())
                    .unwrap(),
                _ => unreachable!(),
            };
            emit_scene(
                &format!("by-{method}-{theme_name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
        for (name, style) in [("step", HistStyle::Step), ("stacked", HistStyle::Stacked)] {
            let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
            labelled(&mut f, "Share by region");
            axes(&mut f)
                .grouped_hist(
                    samples(),
                    GroupedHistOptions {
                        bins: Bins::Edges(vec![0.0, 2.0, 4.0, 7.0]),
                        style,
                        stat: HistStat::Fraction,
                        ..GroupedHistOptions::default()
                    },
                )
                .unwrap();
            emit_scene(
                &format!("by-hist-{name}-{theme_name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
        for (name, x, y) in [
            ("bar", ScaleKind::Category, linear()),
            ("horizontal", linear(), ScaleKind::Category),
        ] {
            let mut f = figure(6.0, 4.0, theme.clone(), x, y);
            labelled(&mut f, "Missing observations");
            axes(&mut f)
                .grouped_bar(
                    vec!["Q2".into(), "Q1".into(), "Q3".into()],
                    vec![
                        Series {
                            label: "South".into(),
                            values: vec![2.0, f64::NAN, 4.0],
                            ..Series::default()
                        },
                        Series {
                            label: "North".into(),
                            values: vec![5.0, 3.0, f64::NAN],
                            ..Series::default()
                        },
                    ],
                )
                .unwrap();
            emit_scene(
                &format!("by-{name}-{theme_name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
