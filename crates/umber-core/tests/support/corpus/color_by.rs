//! Output cases for color-mapped scatter and bubble marks, color limits and their errors.

use std::fmt::Write as _;

use umber_core::{
    BubbleOptions, Cmap, CmapChoice, ColorBy, Figure, FontLibrary, LogBase, PointColor,
    ScatterOptions, Theme, Transform,
};

use super::super::{Output, axes, emit_scene, figure, linear, numbers, record, themes, wobble};

fn mapped(by: ColorBy) -> ScatterOptions {
    ScatterOptions {
        color: PointColor::By(by),
        ..ScatterOptions::default()
    }
}

/// Readings over distance and depth, colored by temperature with the theme's colormap.
fn readings(theme: Theme) -> Figure {
    let (x, y) = wobble(120, 41);
    let values: Vec<f64> = x
        .iter()
        .zip(&y)
        .map(|(x, y)| 18.0 + 4.5 * (x / 22.0).sin() + 0.8 * y)
        .collect();
    let mut f = figure(6.0, 4.4, theme, linear(), linear());
    axes(&mut f)
        .title("Readings by depth and distance")
        .subtitle("Colored by temperature")
        .xlabel("Distance, km")
        .ylabel("Depth, m")
        .scatter(numbers(x), &y, mapped(ColorBy::new(values)))
        .unwrap();
    f
}

/// Countries with population as bubble area and urban share as color.
fn countries(theme: Theme) -> Figure {
    let mut f = figure(6.4, 4.4, theme, linear(), linear());
    axes(&mut f)
        .title("Countries")
        .subtitle("Area is population, color is urban share")
        .bubbles(
            &[8.0, 15.0, 22.0, 31.0, 38.0, 46.0, 55.0],
            &[63.0, 66.0, 68.0, 71.0, 73.0, 76.0, 79.0],
            &[120.0, 870.0, 60.0, 300.0, 45.0, 210.0, 90.0],
            BubbleOptions {
                label: "Population".into(),
                color: PointColor::By(ColorBy::new(vec![
                    30.0,
                    42.0,
                    f64::NAN,
                    61.0,
                    70.0,
                    83.0,
                    92.0,
                ])),
                ..BubbleOptions::default()
            },
        )
        .unwrap();
    f
}

/// Anomalies about zero with the theme's diverging colormap, clamped by color limits.
fn anomalies(theme: Theme) -> Figure {
    let x: Vec<f64> = (0..40).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| (v / 6.0).sin() * 3.0).collect();
    let values: Vec<f64> = x.iter().map(|v| (v - 14.0) / 4.0).collect();
    let mut f = figure(6.0, 4.0, theme, linear(), linear());
    axes(&mut f)
        .title("Anomaly")
        .clim(-3.0, 3.0)
        .unwrap()
        .scatter(
            numbers(x),
            &y,
            mapped(ColorBy {
                center: Some(0.0),
                ..ColorBy::new(values)
            }),
        )
        .unwrap();
    f
}

/// Counts on a log color scale with a fixed colormap read backwards.
fn counts() -> Figure {
    let x: Vec<f64> = (1..=30).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| v.sqrt()).collect();
    let values: Vec<f64> = x.iter().map(|v| 10f64.powf(v / 8.0)).collect();
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut f)
        .scatter(
            numbers(x),
            &y,
            mapped(ColorBy {
                cmap: CmapChoice::Fixed(Cmap::magma().reversed()),
                scale: Transform::Log(LogBase::TEN),
                ..ColorBy::new(values)
            }),
        )
        .unwrap();
    f
}

/// Records `result` of a call on `f`, which must leave `f` unchanged when it fails.
fn record_call(
    out: &mut String,
    label: &str,
    f: &mut Figure,
    call: impl FnOnce(&mut Figure) -> umber_core::Result<()>,
) {
    let before = f.clone();
    match call(f) {
        Ok(()) => {
            let _ = writeln!(out, "{label}: ok");
        }
        Err(err) => {
            assert_eq!(*f, before, "{label}");
            let _ = writeln!(
                out,
                "{label}: {}\nparameter={:?} hint={:?}",
                record::error(&err),
                err.parameter(),
                err.hint()
            );
        }
    }
}

/// Emits the scenes in all themes and the errors of invalid color mappings and limits.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (case, f) in [
            ("scatter", readings(theme.clone())),
            ("bubbles", countries(theme.clone())),
            ("centered", anomalies(theme.clone())),
        ] {
            emit_scene(
                &format!("color-by-{case}-{name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
    emit_scene("color-by-log", &counts().to_scene(lib).unwrap(), sink);
    let mut all_nan = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    axes(&mut all_nan)
        .scatter(
            numbers(vec![1.0, 2.0]),
            &[1.0, 2.0],
            mapped(ColorBy::new(vec![f64::NAN, f64::NAN])),
        )
        .unwrap();
    emit_scene("color-by-all-nan", &all_nan.to_scene(lib).unwrap(), sink);
    let mut equal = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut equal)
        .scatter(
            numbers(vec![1.0, 2.0, 3.0]),
            &[3.0, 1.0, 2.0],
            mapped(ColorBy::new(vec![7.0, 7.0, 7.0])),
        )
        .unwrap();
    emit_scene("color-by-equal", &equal.to_scene(lib).unwrap(), sink);

    let mut out = String::new();
    let base = || {
        let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
        axes(&mut f)
            .scatter(
                numbers(vec![1.0, 2.0]),
                &[1.0, 2.0],
                mapped(ColorBy {
                    cmap: CmapChoice::Fixed(Cmap::viridis()),
                    ..ColorBy::new(vec![0.0, 10.0])
                }),
            )
            .unwrap();
        f
    };
    let point = |by: ColorBy| {
        move |f: &mut Figure| {
            axes(f)
                .scatter(numbers(vec![3.0]), &[3.0], mapped(by))
                .map(drop)
        }
    };
    let fixed = |cmap: Cmap, values: Vec<f64>| ColorBy {
        cmap: CmapChoice::Fixed(cmap),
        ..ColorBy::new(values)
    };
    let mut f = base();
    record_call(
        &mut out,
        "same spec",
        &mut f,
        point(fixed(Cmap::viridis(), vec![5.0])),
    );
    let mut f = base();
    record_call(
        &mut out,
        "other cmap",
        &mut f,
        point(fixed(Cmap::magma(), vec![5.0])),
    );
    record_call(
        &mut out,
        "theme cmap",
        &mut f,
        point(ColorBy::new(vec![5.0])),
    );
    record_call(
        &mut out,
        "custom cmap",
        &mut f,
        point(fixed(
            Cmap::from_colors(&[umber_core::Rgba::BLACK, umber_core::Rgba::WHITE]).unwrap(),
            vec![5.0],
        )),
    );
    record_call(
        &mut out,
        "other scale",
        &mut f,
        point(ColorBy {
            scale: Transform::Log(LogBase::TEN),
            ..fixed(Cmap::viridis(), vec![5.0])
        }),
    );
    record_call(
        &mut out,
        "other center",
        &mut f,
        point(ColorBy {
            center: Some(5.0),
            ..fixed(Cmap::viridis(), vec![5.0])
        }),
    );
    record_call(
        &mut out,
        "values length",
        &mut f,
        point(fixed(Cmap::viridis(), vec![5.0, 6.0])),
    );
    for (label, values) in [
        ("infinite value", vec![f64::INFINITY]),
        ("huge value", vec![1e301]),
    ] {
        record_call(
            &mut out,
            label,
            &mut f,
            point(fixed(Cmap::viridis(), values)),
        );
    }
    let log = |values: Vec<f64>| ColorBy {
        scale: Transform::Log(LogBase::TEN),
        ..ColorBy::new(values)
    };
    let mut empty = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    for (label, by) in [
        ("log zero", log(vec![1.0, 2.0, 0.0])),
        ("log negative", log(vec![-1.0])),
        ("log tiny", log(vec![1e-310])),
        (
            "center nan",
            ColorBy {
                center: Some(f64::NAN),
                ..ColorBy::new(vec![1.0])
            },
        ),
        (
            "center with log",
            ColorBy {
                center: Some(1.0),
                ..log(vec![1.0])
            },
        ),
    ] {
        record_call(&mut out, label, &mut empty, point(by));
    }
    record_call(&mut out, "scatter label", &mut empty, |f| {
        axes(f)
            .scatter(
                numbers(vec![1.0]),
                &[1.0],
                ScatterOptions {
                    label: "a".into(),
                    ..mapped(ColorBy::new(vec![1.0]))
                },
            )
            .map(drop)
    });
    record_call(&mut out, "bubbles label", &mut empty.clone(), |f| {
        axes(f)
            .bubbles(
                &[1.0],
                &[1.0],
                &[1.0],
                BubbleOptions {
                    label: "a".into(),
                    color: PointColor::By(ColorBy::new(vec![1.0])),
                    ..BubbleOptions::default()
                },
            )
            .map(drop)
    });
    for (label, low, high) in [
        ("clim reversed", 5.0, 2.0),
        ("clim equal", 2.0, 2.0),
        ("clim nan", f64::NAN, 2.0),
        ("clim infinite", 0.0, f64::INFINITY),
        ("clim huge", -1e301, 0.0),
        ("clim narrow", 1.0, 1.0 + 1e-12),
        ("clim ok", 0.0, 10.0),
    ] {
        record_call(&mut out, label, &mut empty.clone(), |f| {
            axes(f).clim(low, high).map(drop)
        });
    }
    let mut logged = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut logged)
        .scatter(numbers(vec![1.0]), &[1.0], mapped(log(vec![1.0])))
        .unwrap();
    record_call(&mut out, "clim log zero", &mut logged, |f| {
        axes(f).clim(0.0, 10.0).map(drop)
    });
    record_call(&mut out, "clim log tiny", &mut logged, |f| {
        axes(f).clim(1e-310, 10.0).map(drop)
    });
    let mut centered = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut centered)
        .scatter(
            numbers(vec![1.0]),
            &[1.0],
            mapped(ColorBy {
                center: Some(12.0),
                ..ColorBy::new(vec![1.0])
            }),
        )
        .unwrap();
    record_call(&mut out, "clim below center", &mut centered, |f| {
        axes(f).clim(0.0, 10.0).map(drop)
    });
    record_call(&mut out, "clim above center", &mut centered, |f| {
        axes(f).clim(20.0, 30.0).map(drop)
    });
    let mut limited = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut limited).clim(0.0, 10.0).unwrap();
    record_call(
        &mut out,
        "log after clim",
        &mut limited,
        point(log(vec![1.0])),
    );
    record_call(
        &mut out,
        "center after clim",
        &mut limited,
        point(ColorBy {
            center: Some(12.0),
            ..ColorBy::new(vec![1.0])
        }),
    );
    sink(Output {
        kind: "errors",
        file: "color-by.txt".into(),
        bytes: out.into_bytes(),
    });
}
