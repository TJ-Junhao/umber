//! Output cases for box plots, grouped distributions, degenerate groups and validation.

use super::super::{Log, Output, axes, emit_scene, figure, linear, log, themes};
use umber_core::{
    BoxOptions, Distribution, Figure, FontLibrary, Grid, GridIndex, Placement, ScaleKind, Theme,
};

fn categories() -> Vec<String> {
    ["North", "East", "South", "West"]
        .map(String::from)
        .to_vec()
}

fn groups() -> Vec<Vec<f64>> {
    vec![
        vec![1.0, 2.0, 3.0, 4.0, 5.0, 12.0],
        vec![2.0, 4.0, 5.0, 6.0, 8.0],
        vec![-4.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        vec![3.0, 4.0, 6.0, 8.0, 9.0],
    ]
}

fn distributions() -> Vec<Distribution> {
    vec![
        Distribution {
            label: "2025".into(),
            groups: groups(),
            ..Default::default()
        },
        Distribution {
            label: "2026".into(),
            groups: groups()
                .into_iter()
                .map(|g| g.into_iter().map(|v| v + 2.0).collect())
                .collect(),
            ..Default::default()
        },
    ]
}

/// Emits plain, grouped, clipped, log and degenerate boxes in all themes, then their errors.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        let mut f = Figure::with_grid(12.0, 4.0, theme.clone(), Grid::new(1, 2).unwrap()).unwrap();
        for col in 0..2 {
            let mut a = f
                .add_axes_at(
                    Placement {
                        row: GridIndex::At(0),
                        col: GridIndex::At(col),
                        row_span: 1,
                        col_span: 1,
                    },
                    ScaleKind::Category,
                    linear(),
                )
                .unwrap();
            if col == 0 {
                a.title("Samples")
                    .box_plot(
                        categories(),
                        groups(),
                        BoxOptions {
                            label: "Values".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            } else {
                a.title("Grouped samples")
                    .grouped_box_plot(categories(), distributions())
                    .unwrap();
            }
        }
        emit_scene(&format!("box-{name}"), &f.to_scene(lib).unwrap(), sink);

        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        axes(&mut f)
            .title("Empty and constant groups")
            .box_plot(
                categories(),
                vec![vec![], vec![f64::NAN], vec![2.0], vec![4.0, 4.0]],
                BoxOptions::default(),
            )
            .unwrap();
        emit_scene(
            &format!("box-degenerate-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        axes(&mut f)
            .title("Clipped boxes")
            .ylim(2.5, 5.5)
            .unwrap()
            .grouped_box_plot(categories(), distributions())
            .unwrap();
        emit_scene(
            &format!("box-limits-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, log(10.0));
        axes(&mut f)
            .title("Log y")
            .box_plot(
                vec!["A".into(), "B".into()],
                vec![vec![1.0, 2.0, 4.0, 8.0, 100.0], vec![2.0, 5.0, 10.0, 30.0]],
                BoxOptions::default(),
            )
            .unwrap();
        emit_scene(
            &format!("box-log-y-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
    let mut f = figure(3.0, 3.0, Theme::pigment(), ScaleKind::Category, linear());
    axes(&mut f)
        .title("Narrow groups")
        .grouped_box_plot(
            vec!["A".into()],
            (0..30)
                .map(|i| Distribution {
                    groups: vec![vec![f64::from(i), f64::from(i + 3)]],
                    ..Default::default()
                })
                .collect(),
        )
        .unwrap();
    emit_scene("box-narrow-gaps", &f.to_scene(lib).unwrap(), sink);
    sink(Output {
        kind: "errors",
        file: "box.txt".into(),
        bytes: errors().into_bytes(),
    });
}

fn errors() -> String {
    let mut rec = Log::default();
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    for (name, groups) in [
        ("length", vec![]),
        ("infinite", vec![vec![1.0, f64::INFINITY]]),
        ("huge", vec![vec![1e301]]),
        ("NaN", vec![vec![f64::NAN]]),
        ("empty group", vec![vec![]]),
        ("one value", vec![vec![2.0]]),
    ] {
        rec.on_axes(&mut f, name, |a| {
            a.box_plot(vec!["A".into()], groups, BoxOptions::default())
                .map(drop)
        });
    }
    rec.on_axes(&mut f, "repeat", |a| {
        a.box_plot(
            vec!["A".into(), "A".into()],
            vec![vec![], vec![]],
            BoxOptions::default(),
        )
        .map(drop)
    });
    for (name, d) in [
        ("empty distributions", vec![]),
        ("group count", vec![Distribution::default()]),
        (
            "infinite distribution",
            vec![Distribution {
                groups: vec![vec![f64::NEG_INFINITY]],
                ..Default::default()
            }],
        ),
    ] {
        rec.on_axes(&mut f, name, |a| {
            a.grouped_box_plot(vec!["A".into()], d).map(drop)
        });
    }
    for x in [linear(), log(10.0)] {
        let mut f = rec.new_figure("numeric x", x, linear());
        rec.on_axes(&mut f, "box", |a| {
            a.box_plot(vec![], vec![], BoxOptions::default()).map(drop)
        });
        rec.on_axes(&mut f, "grouped box", |a| {
            a.grouped_box_plot(vec![], vec![]).map(drop)
        });
    }
    let mut f = rec.new_figure("category/log", ScaleKind::Category, log(10.0));
    for value in [0.0, -1.0, 1e-310] {
        rec.on_axes(&mut f, "invalid log sample", |a| {
            a.box_plot(vec!["A".into()], vec![vec![value]], BoxOptions::default())
                .map(drop)
        });
    }
    rec.out
}
