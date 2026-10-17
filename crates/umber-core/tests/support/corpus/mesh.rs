//! Numeric cells and sparse two-dimensional counts in each theme; cells drawn as one image on
//! log axes and cut by explicit limits, and the error of an image of uneven edges.

use std::fmt::Write as _;

use super::super::{Output, axes, emit_scene, figure, linear, record, themes};
use umber_core::{
    AxisData, FontLibrary, HexbinOptions, Hist2dOptions, Instant, LogBase, MeshOptions, Render,
    ScaleKind, Theme, Transform,
};

/// Emits edge-spanning cells, rectangular counts and regular hexagonal counts.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .mesh(
                &[-2.0, 0.0, 1.0, 4.0],
                &[0.0, 1.0, 3.0],
                &[-2.0, f64::NAN, 4.0, 1.0, 0.0, 2.0],
                MeshOptions {
                    center: Some(0.0),
                    ..MeshOptions::default()
                },
            )
            .unwrap()
            .title("Cell values");
        emit_scene(&format!("mesh-{name}"), &f.to_scene(lib).unwrap(), sink);

        let x: Vec<_> = (0..180)
            .map(|i| (i as f64 * 0.37).sin() + (i % 9) as f64 / 7.0)
            .collect();
        let y: Vec<_> = (0..180)
            .map(|i| (i as f64 * 0.17).cos() + (i % 7) as f64 / 5.0)
            .collect();
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .hist2d(
                &x,
                &y,
                Hist2dOptions {
                    bins: (12, 9),
                    scale: Transform::Log(LogBase::TEN),
                    ..Hist2dOptions::default()
                },
            )
            .unwrap()
            .title("Rectangular counts");
        emit_scene(&format!("hist2d-{name}"), &f.to_scene(lib).unwrap(), sink);

        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        axes(&mut f)
            .hexbin(
                &x,
                &y,
                HexbinOptions {
                    gridsize: 12,
                    ..HexbinOptions::default()
                },
            )
            .unwrap()
            .title("Hexagonal counts");
        emit_scene(&format!("hexbin-{name}"), &f.to_scene(lib).unwrap(), sink);
    }
}

/// Emits cells drawn as one image: a mesh on log axes with an empty cell, and a 2-D histogram
/// whose outer bins explicit limits cut; then the errors of images of uneven edges.
pub fn raster_cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let decades: Vec<f64> = (0..=8).map(|i| 10f64.powf(f64::from(i) / 2.0)).collect();
    let values: Vec<f64> = (0..8 * 6)
        .map(|i| {
            if i == 13 {
                f64::NAN
            } else {
                f64::from(i * 7 % 23)
            }
        })
        .collect();
    let mut f = figure(6.0, 4.0, Theme::pigment(), log, log);
    axes(&mut f)
        .mesh(
            &decades,
            &decades[..7],
            &values,
            MeshOptions {
                render: Render::Raster,
                ..MeshOptions::default()
            },
        )
        .unwrap()
        .title("Cells on log axes");
    emit_scene("mesh-log-raster", &f.to_scene(lib).unwrap(), sink);

    let x: Vec<f64> = (0..400)
        .map(|i| (f64::from(i) * 0.37).sin() * 2.0 + f64::from(i % 9) / 7.0)
        .collect();
    let y: Vec<f64> = (0..400)
        .map(|i| (f64::from(i) * 0.17).cos() * 2.0 + f64::from(i % 7) / 5.0)
        .collect();
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f)
        .hist2d(
            &x,
            &y,
            Hist2dOptions {
                bins: (14, 11),
                render: Render::Raster,
                ..Hist2dOptions::default()
            },
        )
        .unwrap()
        .xlim(-1.3, 2.2)
        .unwrap()
        .ylim(-1.1, 2.6)
        .unwrap()
        .title("Counts cut by limits");
    emit_scene("hist2d-raster-limits", &f.to_scene(lib).unwrap(), sink);

    let day = |n: i64| Instant::from_micros(n * 86_400_000_000).ok();
    let numbers = |v: &[f64]| AxisData::Numbers(v.to_vec());
    let cases = [
        (
            "uneven numbers on a linear x axis",
            [linear(), linear()],
            numbers(&[0.0, 1.0, 2.0, 4.0]),
            numbers(&[0.0, 1.0]),
            3,
        ),
        (
            "evenly spaced numbers on a log y axis",
            [linear(), log],
            numbers(&[0.0, 1.0]),
            numbers(&[1.0, 2.0, 3.0]),
            2,
        ),
        (
            "months on a time x axis",
            [ScaleKind::Time, linear()],
            AxisData::Times(vec![day(0), day(31), day(59), day(90)]),
            numbers(&[0.0, 1.0]),
            3,
        ),
    ];
    let mut out = String::new();
    for (label, [x_kind, y_kind], x, y, count) in cases {
        let mut f = figure(6.0, 4.0, Theme::pigment(), x_kind, y_kind);
        let before = f.clone();
        let err = axes(&mut f)
            .mesh_time(
                x,
                y,
                &vec![1.0; count],
                MeshOptions {
                    render: Render::Raster,
                    ..MeshOptions::default()
                },
            )
            .unwrap_err();
        assert_eq!(f, before, "{label}");
        let _ = writeln!(
            out,
            "{label}: {}\nparameter={:?} hint={:?}\nstate unchanged=true",
            record::error(&err),
            err.parameter(),
            err.hint(),
        );
    }
    sink(Output {
        kind: "errors",
        file: "mesh-raster.txt".into(),
        bytes: out.into_bytes(),
    });
}
