//! Output cases for contours: lines, filled bands and both in each theme, level labels in each
//! theme, lines in one color, a field with holes, a diverging field, a log color scale, a
//! contour cut by explicit limits beside a scatter, and the errors of invalid contours.

use std::fmt::Write as _;

use umber_core::{
    AxisData, ColorSpec, ColorbarOptions, ColorbarPosition, ContourLevels, ContourOptions,
    ContourStyle, Figure, FontLibrary, LogBase, PointColor, Rgba, ScaleKind, ScatterOptions, Theme,
    Transform,
};

use super::super::{Output, axes, emit_scene, figure, linear, record, themes};

/// `n` evenly spaced values from `lo` to `hi`.
fn spaced(lo: f64, hi: f64, n: u32) -> Vec<f64> {
    (0..n)
        .map(|i| lo + (hi - lo) * f64::from(i) / f64::from(n - 1))
        .collect()
}

/// A smooth field on a 60 × 45 grid: two bumps and a gentle saddle, the second bump negative
/// when `diverging`; NaN on an island and in a corner when `holes`.
fn field(diverging: bool, holes: bool) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (x, y) = (spaced(-3.0, 3.0, 60), spaced(-2.4, 2.4, 45));
    let sign = if diverging { -1.0 } else { 1.0 };
    let z = y
        .iter()
        .flat_map(|&y| {
            x.iter().map(move |&x| {
                let a = 1.8 * (-((x + 0.85).powi(2) / 1.0 + (y - 0.35).powi(2) / 0.75)).exp();
                let b = 1.4 * (-((x - 1.05).powi(2) / 0.8 + (y + 0.55).powi(2) / 0.65)).exp();
                let island = (x + 0.3).powi(2) + (y - 0.6).powi(2) < 0.26;
                let corner = x < -2.0 && y > 1.55;
                if holes && (island || corner) {
                    f64::NAN
                } else {
                    1.5 * (a + sign * b) + 0.1 * x * y
                }
            })
        })
        .collect();
    (x, y, z)
}

/// A labelled figure with one contour of [`field`] drawn with `options`, and its colorbar
/// labelled when the contour is color-mapped.
fn contour(theme: Theme, title: &str, options: ContourOptions, field: [bool; 2]) -> Figure {
    let (x, y, z) = self::field(field[0], field[1]);
    let mapped = matches!(options.color, ColorSpec::Auto(_));
    let mut f = figure(6.0, 4.4, theme, linear(), linear());
    let mut ax = axes(&mut f);
    ax.title(title)
        .xlabel("x")
        .ylabel("y")
        .contour(&x, &y, &z, options)
        .unwrap();
    if mapped {
        ax.colorbar(ColorbarOptions {
            label: "z".into(),
            ..ColorbarOptions::default()
        })
        .unwrap();
    }
    f
}

fn styled(style: ContourStyle) -> ContourOptions {
    ContourOptions {
        style,
        ..ContourOptions::default()
    }
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

/// Emits the scenes and the errors of invalid contours.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (style, label) in [
            (ContourStyle::Lines, "lines"),
            (ContourStyle::Filled, "filled"),
            (ContourStyle::Both, "both"),
        ] {
            let f = contour(theme.clone(), "Smooth field", styled(style), [false, false]);
            emit_scene(
                &format!("contour-{label}-{name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
        let annotated = ContourOptions {
            annotate: true,
            ..ContourOptions::default()
        };
        let f = contour(theme, "Level labels", annotated, [false, false]);
        emit_scene(
            &format!("contour-annotated-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
    let one_color = ContourOptions {
        color: ColorSpec::Fixed(Rgba::hex("#2E6B70").unwrap()),
        annotate: true,
        levels: ContourLevels::Count(5),
        ..ContourOptions::default()
    };
    let f = contour(Theme::graphite(), "One color", one_color, [false, false]);
    emit_scene("contour-color", &f.to_scene(lib).unwrap(), sink);
    let holes = ContourOptions {
        annotate: true,
        ..styled(ContourStyle::Both)
    };
    let f = contour(Theme::pigment(), "Holes", holes, [false, true]);
    emit_scene("contour-holes", &f.to_scene(lib).unwrap(), sink);
    let diverging = ContourOptions {
        center: Some(0.0),
        ..styled(ContourStyle::Filled)
    };
    let mut f = contour(
        Theme::midnight(),
        "Centred at zero",
        diverging,
        [true, false],
    );
    axes(&mut f)
        .colorbar(ColorbarOptions {
            label: "z".into(),
            position: Some(ColorbarPosition::Bottom),
            ..ColorbarOptions::default()
        })
        .unwrap();
    emit_scene("contour-diverging", &f.to_scene(lib).unwrap(), sink);

    // A log color scale over five decades: log ticks as levels, explicit ones in the data.
    let (x, y) = (spaced(0.0, 4.0, 21), spaced(0.0, 3.0, 16));
    let z: Vec<f64> = y
        .iter()
        .flat_map(|&y| x.iter().map(move |&x| 10f64.powf(x + y / 3.0)))
        .collect();
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut f)
        .title("Log color scale")
        .contour(
            &x,
            &y,
            &z,
            ContourOptions {
                scale: Transform::Log(LogBase::TEN),
                annotate: true,
                ..styled(ContourStyle::Both)
            },
        )
        .unwrap();
    emit_scene("contour-log", &f.to_scene(lib).unwrap(), sink);

    // Explicit limits cut the grid; a scatter over it keeps its own colors.
    let (x, y, z) = field(false, false);
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    axes(&mut f)
        .contour(
            &x,
            &y,
            &z,
            ContourOptions {
                levels: ContourLevels::Values(vec![-0.2, 0.3, 0.9, 1.6, 2.2]),
                annotate: true,
                ..styled(ContourStyle::Both)
            },
        )
        .unwrap()
        .scatter(
            AxisData::Numbers(vec![-1.0, 0.0, 1.2]),
            &[0.2, -0.4, -0.6],
            ScatterOptions {
                color: PointColor::Uniform(ColorSpec::Fixed(Rgba::RED)),
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .xlim(-1.5, 2.0)
        .unwrap()
        .ylim(-1.2, 1.4)
        .unwrap()
        .title("Cut by limits");
    emit_scene("contour-limits", &f.to_scene(lib).unwrap(), sink);

    let mut out = String::new();
    let add = |x: &'static [f64], y: &'static [f64], z: &'static [f64], options: ContourOptions| {
        move |f: &mut Figure| axes(f).contour(x, y, z, options).map(drop)
    };
    const X: [f64; 3] = [0.0, 1.0, 2.0];
    const Y: [f64; 2] = [0.0, 1.0];
    const Z: [f64; 6] = [0.0, 1.0, 2.0, 1.0, 2.0, 3.0];
    let mut categories = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    record_call(
        &mut out,
        "category x axis",
        &mut categories,
        add(&X, &Y, &Z, ContourOptions::default()),
    );
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    for (label, call) in [
        ("one x", add(&[0.0], &Y, &Z, ContourOptions::default())),
        (
            "x not increasing",
            add(&[0.0, 2.0, 1.0], &Y, &Z, ContourOptions::default()),
        ),
        (
            "z too short",
            add(&X, &Y, &Z[..5], ContourOptions::default()),
        ),
        (
            "z infinite",
            add(
                &X,
                &Y,
                &[0.0, f64::INFINITY, 2.0, 1.0, 2.0, 3.0],
                ContourOptions::default(),
            ),
        ),
        (
            "no levels",
            add(
                &X,
                &Y,
                &Z,
                ContourOptions {
                    levels: ContourLevels::Values(Vec::new()),
                    ..ContourOptions::default()
                },
            ),
        ),
        (
            "zero levels",
            add(
                &X,
                &Y,
                &Z,
                ContourOptions {
                    levels: ContourLevels::Count(0),
                    ..ContourOptions::default()
                },
            ),
        ),
        (
            "filled with labels",
            add(
                &X,
                &Y,
                &Z,
                ContourOptions {
                    annotate: true,
                    ..styled(ContourStyle::Filled)
                },
            ),
        ),
        (
            "filled in one color",
            add(
                &X,
                &Y,
                &Z,
                ContourOptions {
                    color: ColorSpec::Fixed(Rgba::RED),
                    ..styled(ContourStyle::Both)
                },
            ),
        ),
        (
            "format without labels",
            add(
                &X,
                &Y,
                &Z,
                ContourOptions {
                    format: Some(umber_core::TickFormat::fixed(1).unwrap()),
                    ..ContourOptions::default()
                },
            ),
        ),
    ] {
        record_call(&mut out, label, &mut f, call);
    }
    sink(Output {
        kind: "errors",
        file: "contour.txt".into(),
        bytes: out.into_bytes(),
    });
}
