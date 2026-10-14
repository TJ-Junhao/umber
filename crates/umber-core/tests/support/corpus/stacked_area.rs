//! Stacked areas in three themes, shared gaps, limits, log axes and long inputs.

use super::super::{Log, Output, axes, emit_scene, figure, linear, log, themes};
use umber_core::{Figure, FontLibrary, ScaleKind, Series, Theme};

fn series(values: &[f64]) -> Series {
    Series {
        values: values.to_vec(),
        ..Series::default()
    }
}

fn generation(theme: Theme, gaps: bool) -> Figure {
    let x: Vec<_> = (2010..=2025).map(f64::from).collect();
    let mut layers = vec![
        Series {
            label: "Hydro".into(),
            values: x.iter().map(|x| 12.0 + 0.6 * x.sin()).collect(),
            ..Series::default()
        },
        Series {
            label: "Wind".into(),
            values: x
                .iter()
                .map(|x| 6.0 + 0.8 * (x - 2010.0) + 0.5 * (1.7 * (x - 2010.0)).sin())
                .collect(),
            ..Series::default()
        },
        Series {
            label: "Solar".into(),
            values: x
                .iter()
                .map(|x| 2.0 + 0.9 * (x - 2010.0).powf(1.3) / 4.0)
                .collect(),
            ..Series::default()
        },
    ];
    if gaps {
        layers[0].values[4] = f64::NAN;
        layers[2].values[10] = f64::NAN;
    }
    let mut f = figure(6.0, 4.0, theme, linear(), linear());
    axes(&mut f)
        .title("Renewable generation")
        .subtitle("TWh per year, stacked")
        .stacked_area(&x, layers)
        .unwrap();
    f
}

/// Passes the stacked area scenes and error records to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (theme_name, theme) in themes() {
        for gaps in [false, true] {
            let name = if gaps {
                "stacked-area-gaps"
            } else {
                "stacked-area"
            };
            let f = generation(theme.clone(), gaps);
            emit_scene(
                &format!("{name}-{theme_name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
    let mut f = generation(Theme::graphite(), false);
    axes(&mut f)
        .xlim(2013.5, 2022.5)
        .unwrap()
        .ylim(8.0, 30.0)
        .unwrap();
    emit_scene("stacked-area-limits", &f.to_scene(lib).unwrap(), sink);

    let mut f = figure(6.0, 4.0, Theme::midnight(), log(10.0), log(10.0));
    axes(&mut f)
        .stacked_area(
            &[1.0, 10.0, 100.0, 1000.0],
            vec![
                series(&[1.0, 2.0, 4.0, 8.0]),
                series(&[2.0, 4.0, 8.0, 16.0]),
            ],
        )
        .unwrap();
    emit_scene("stacked-area-log", &f.to_scene(lib).unwrap(), sink);

    let x: Vec<_> = (0..200_000).map(f64::from).collect();
    let a: Vec<_> = x.iter().map(|x| 2.0 + (x * 0.47).sin()).collect();
    let b: Vec<_> = x.iter().map(|x| 3.0 + (x * 0.31).cos()).collect();
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut f)
        .stacked_area(&x, vec![series(&a), series(&b)])
        .unwrap()
        .xlim(60_000.25, 140_000.75)
        .unwrap();
    emit_scene("stacked-area-long-limits", &f.to_scene(lib).unwrap(), sink);
    sink(Output {
        kind: "errors",
        file: "stacked_area.txt".into(),
        bytes: errors().into_bytes(),
    });
}

fn errors() -> String {
    let mut rec = Log::default();
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "valid layers", |a| {
        a.stacked_area(&[0.0, 1.0], vec![series(&[1.0, 2.0]), series(&[2.0, 1.0])])
            .map(drop)
    });
    rec.on_axes(&mut f, "empty series", |a| {
        a.stacked_area(&[], vec![]).map(drop)
    });
    rec.on_axes(&mut f, "length", |a| {
        a.stacked_area(&[0.0, 1.0], vec![series(&[1.0, 2.0]), series(&[1.0])])
            .map(drop)
    });
    rec.on_axes(&mut f, "negative", |a| {
        a.stacked_area(&[0.0, 1.0], vec![series(&[1.0, 2.0]), series(&[1.0, -2.0])])
            .map(drop)
    });
    rec.on_axes(&mut f, "unsorted x", |a| {
        a.stacked_area(&[1.0, 0.0], vec![series(&[1.0, 2.0])])
            .map(drop)
    });
    rec.on_axes(&mut f, "unsorted x across gap", |a| {
        a.stacked_area(&[1.0, f64::NAN, 0.0], vec![series(&[1.0; 3])])
            .map(drop)
    });
    rec.on_axes(&mut f, "infinite x", |a| {
        a.stacked_area(&[f64::INFINITY], vec![series(&[1.0])])
            .map(drop)
    });
    rec.on_axes(&mut f, "infinite value", |a| {
        a.stacked_area(&[0.0], vec![series(&[f64::INFINITY])])
            .map(drop)
    });
    rec.on_axes(&mut f, "huge value", |a| {
        a.stacked_area(&[0.0], vec![series(&[1e301])]).map(drop)
    });
    rec.on_axes(&mut f, "huge total", |a| {
        a.stacked_area(&[0.0], vec![series(&[1e300]), series(&[1e300])])
            .map(drop)
    });
    rec.on_axes(&mut f, "empty x", |a| {
        a.stacked_area(&[], vec![series(&[])]).map(drop)
    });
    rec.on_axes(&mut f, "shared gap", |a| {
        a.stacked_area(
            &[0.0, 1.0],
            vec![series(&[1.0, f64::NAN]), series(&[f64::NAN, 1.0])],
        )
        .map(drop)
    });
    let mut f = rec.new_figure("category x", ScaleKind::Category, linear());
    rec.on_axes(&mut f, "category refusal before empty series", |a| {
        a.stacked_area(&[], vec![]).map(drop)
    });
    let mut f = rec.new_figure("log/log", log(10.0), log(10.0));
    for (name, x, value) in [
        ("zero x", 0.0, 1.0),
        ("zero y", 1.0, 0.0),
        ("negative y", 1.0, -1.0),
        ("tiny y", 1.0, 1e-301),
        ("valid", 1.0, 2.0),
    ] {
        rec.on_axes(&mut f, name, |a| {
            a.stacked_area(&[x], vec![series(&[value])]).map(drop)
        });
    }
    rec.out
}
