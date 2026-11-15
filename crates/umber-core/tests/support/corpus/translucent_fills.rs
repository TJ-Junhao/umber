//! Output-gate cases for translucent fill colors and the width of a scatter's joining line.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Scenes, in the three built-in themes: a translucent band beside an opaque one; a translucent
//! area beside an opaque one; a translucent histogram; translucent boxes and violins on a
//! category axis; translucent scatter, bubble and stem markers beside opaque ones; and scatters
//! with a joining line of the automatic, a fixed and a relative width, one with error bars.
//!
//! In `errors/translucent-fills.txt`: a joining-line width without a joining line, on a plain
//! and on a grouped scatter.

use std::sync::Arc;

use umber_core::figure::{
    AreaOptions, AxisData, BandOptions, BoxOptions, BubbleOptions, ErrorRange, HistOptions,
    LineOptions, ScatterOptions, StemOptions, ViolinOptions,
};
use umber_core::{
    Alpha, ColorSpec, Figure, FontLibrary, MarkRender, PointColor, PointLabels, ScaleKind,
    ScatterGroup, SizeSpec,
};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, linear, themes};

fn palette(index: usize, alpha: f64) -> ColorSpec {
    ColorSpec::Palette {
        index,
        alpha: Alpha::new(alpha).unwrap(),
    }
}

fn point(index: usize, alpha: f64) -> PointColor {
    PointColor::Uniform(palette(index, alpha))
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "translucent-fills.txt".into(),
        bytes: errors().into_bytes(),
    });
}

/// Every figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();
    let x: Vec<f64> = (0..9).map(f64::from).collect();
    let mean: Vec<f64> = x.iter().map(|x| 6.0 + 2.0 * (x / 2.0).sin()).collect();
    for (theme_name, theme) in themes() {
        let new = |title: &str, x_scale: ScaleKind| {
            let mut f = figure(6.0, 4.0, theme.clone(), x_scale, linear());
            labelled(&mut f, title);
            f
        };

        let mut f = new("Translucent band", linear());
        axes(&mut f)
            .band(
                &x,
                &mean.iter().map(|m| m - 1.5).collect::<Vec<_>>(),
                &mean.iter().map(|m| m + 1.5).collect::<Vec<_>>(),
                BandOptions {
                    label: "Alpha 0.15".into(),
                    color: palette(0, 0.15),
                },
            )
            .unwrap()
            .band(
                &x,
                &mean.iter().map(|m| m - 4.0).collect::<Vec<_>>(),
                &mean.iter().map(|m| m - 3.0).collect::<Vec<_>>(),
                BandOptions {
                    label: "Opaque".into(),
                    color: palette(1, 1.0),
                },
            )
            .unwrap()
            .line(
                AxisData::Numbers(x.clone()),
                &mean,
                LineOptions {
                    label: "Mean".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        out.push((format!("translucent-band-{theme_name}"), f));

        let mut f = new("Translucent area", linear());
        axes(&mut f)
            .area(
                AxisData::Numbers(x.clone()),
                &mean,
                AreaOptions {
                    label: "Alpha 0.5".into(),
                    color: palette(0, 0.5),
                    ..AreaOptions::default()
                },
            )
            .unwrap()
            .area(
                AxisData::Numbers(x.clone()),
                &mean.iter().map(|m| m / 2.0).collect::<Vec<_>>(),
                AreaOptions {
                    label: "Opaque".into(),
                    color: palette(1, 1.0),
                    ..AreaOptions::default()
                },
            )
            .unwrap();
        out.push((format!("translucent-area-{theme_name}"), f));

        let mut f = new("Translucent histogram", linear());
        let values: Vec<f64> = (0..60)
            .map(|i| f64::from(i % 7) + f64::from(i % 3))
            .collect();
        axes(&mut f)
            .hist(
                &values,
                HistOptions {
                    label: "Alpha 0.5".into(),
                    color: palette(0, 0.5),
                    ..HistOptions::default()
                },
            )
            .unwrap();
        out.push((format!("translucent-hist-{theme_name}"), f));

        let groups = || {
            vec![
                vec![1.0, 2.0, 2.5, 3.0, 4.0, 9.0],
                vec![2.0, 3.0, 3.5, 5.0, 5.5, 6.0],
            ]
        };
        let names = || vec!["A".to_owned(), "B".to_owned()];
        let mut f = new("Translucent boxes", ScaleKind::Category);
        axes(&mut f)
            .box_plot(
                names(),
                groups(),
                BoxOptions {
                    label: "Alpha 0.5".into(),
                    color: palette(0, 0.5),
                },
            )
            .unwrap();
        out.push((format!("translucent-box-{theme_name}"), f));
        let mut f = new("Translucent violins", ScaleKind::Category);
        axes(&mut f)
            .violin(
                names(),
                groups(),
                ViolinOptions {
                    label: "Alpha 0.5".into(),
                    color: palette(0, 0.5),
                },
            )
            .unwrap();
        out.push((format!("translucent-violin-{theme_name}"), f));

        let mut f = new("Translucent markers", linear());
        axes(&mut f)
            .scatter(
                AxisData::Numbers(x.clone()),
                &mean,
                ScatterOptions {
                    label: "Alpha 0.4".into(),
                    color: point(0, 0.4),
                    size: SizeSpec::pt(14.0).unwrap(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .scatter(
                AxisData::Numbers(x.clone()),
                &mean.iter().map(|m| m - 2.0).collect::<Vec<_>>(),
                ScatterOptions {
                    label: "Opaque".into(),
                    color: point(1, 1.0),
                    size: SizeSpec::pt(14.0).unwrap(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .bubbles(
                &x,
                &mean.iter().map(|m| m - 4.0).collect::<Vec<_>>(),
                &x.iter().map(|x| x + 1.0).collect::<Vec<_>>(),
                BubbleOptions {
                    label: "Bubbles".into(),
                    color: point(2, 0.4),
                    ..BubbleOptions::default()
                },
            )
            .unwrap()
            .stem(
                AxisData::Numbers(x.clone()),
                &mean.iter().map(|m| m - 5.0).collect::<Vec<_>>(),
                StemOptions {
                    label: "Stems".into(),
                    color: palette(3, 0.4),
                    ..StemOptions::default()
                },
            )
            .unwrap();
        out.push((format!("translucent-markers-{theme_name}"), f));

        let mut f = new("Connect width", linear());
        let mut axes = axes(&mut f);
        for (i, (label, width)) in [
            ("Automatic", SizeSpec::Auto),
            ("3 pt", SizeSpec::pt(3.0).unwrap()),
            ("2 x line width", SizeSpec::times(2.0).unwrap()),
        ]
        .into_iter()
        .enumerate()
        {
            axes.scatter(
                AxisData::Numbers(x.clone()),
                &mean.iter().map(|m| m - 2.5 * i as f64).collect::<Vec<_>>(),
                ScatterOptions {
                    label: label.into(),
                    connect: true,
                    connect_width: width,
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        }
        out.push((format!("connect-width-{theme_name}"), f));
    }

    // A fixed width beside error bars, which keep their own width, and as one image.
    let mut f = figure(6.0, 4.0, themes()[0].1.clone(), linear(), linear());
    labelled(&mut f, "Connect width with error bars");
    axes(&mut f)
        .scatter(
            AxisData::Numbers(vec![1.0, 2.0, 3.0, 4.0]),
            &[2.5, 4.0, 3.5, 5.0],
            ScatterOptions {
                label: "Folds".into(),
                labels: PointLabels::default(),
                y_error: Some(ErrorRange::Numbers {
                    low: Arc::from([2.0, 3.0, 3.0, 4.5]),
                    high: Arc::from([3.0, 5.0, 4.0, 5.5]),
                }),
                connect: true,
                connect_width: SizeSpec::pt(4.0).unwrap(),
                render: MarkRender::Raster,
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    out.push(("connect-width-errors-raster".to_owned(), f));
    out
}

/// A joining-line width without a joining line.
fn errors() -> String {
    let mut rec = Log::default();
    rec.section("Axes.scatter, connect_width");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    let width = SizeSpec::pt(2.0).unwrap();
    rec.on_axes(&mut f, "scatter connect_width without connect", |a| {
        a.scatter(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            ScatterOptions {
                connect_width: width,
                ..ScatterOptions::default()
            },
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "scatter connect_width with connect", |a| {
        a.scatter(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            ScatterOptions {
                connect: true,
                connect_width: width,
                ..ScatterOptions::default()
            },
        )
        .map(drop)
    });
    rec.on_axes(
        &mut f,
        "grouped scatter connect_width without connect",
        |a| {
            a.grouped_scatter(
                vec![ScatterGroup::new(
                    "North",
                    AxisData::Numbers(vec![0.0, 1.0]),
                    AxisData::Numbers(vec![1.0, 2.0]),
                )],
                umber_core::Marker::Circle,
                SizeSpec::Auto,
                PointLabels::default(),
                (false, width),
                MarkRender::Vector,
            )
            .map(drop)
        },
    );
    rec.out
}
