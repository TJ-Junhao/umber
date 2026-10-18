//! Output cases for bubble scales, size legends and invalid data.

use std::fmt::Write as _;

use umber_core::{
    BubbleOptions, Figure, FontLibrary, Grid, GridIndex, LegendMode, LegendPosition, LegendStyle,
    Marker, Placement, ScaleKind, Theme, TickFormat,
};

use super::super::{Output, axes, emit_scene, figure, linear, themes};

fn options(label: &str) -> BubbleOptions {
    BubbleOptions {
        label: label.into(),
        ..BubbleOptions::default()
    }
}

fn cities(theme: Theme) -> Figure {
    let mut f = figure(6.0, 4.0, theme, linear(), linear());
    axes(&mut f)
        .title("Cities")
        .subtitle("Marker area shows population, in thousands")
        .xlabel("Income")
        .ylabel("Output")
        .bubbles(
            &[12.0, 18.0, 24.0, 35.0, 42.0, 55.0, 67.0, 78.0],
            &[3.2, 4.8, 2.9, 6.4, 5.7, 7.1, 4.2, 8.3],
            &[80.0, 140.0, 220.0, 500.0, 330.0, 870.0, 190.0, 650.0],
            options("Cities"),
        )
        .unwrap();
    f
}

/// Emits scenes in all themes and the errors from invalid bubble calls.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut getters = String::new();
    for (name, theme) in themes().into_iter().chain([("default", Theme::default())]) {
        let _ = writeln!(
            getters,
            "{name}: bubble_max_radius {:08x}",
            theme.bubble_max_radius().to_bits()
        );
    }
    sink(Output {
        kind: "api",
        file: "themes-bubbles.txt".into(),
        bytes: getters.into_bytes(),
    });
    for (name, theme) in themes() {
        emit_scene(
            &format!("bubbles-{name}"),
            &cities(theme.clone()).to_scene(lib).unwrap(),
            sink,
        );
        for (position_name, position) in [
            ("top-left", LegendPosition::TopLeft),
            ("top-right", LegendPosition::TopRight),
            ("right", LegendPosition::Right),
            ("bottom", LegendPosition::Bottom),
            ("best", LegendPosition::Best),
        ] {
            let mut f = cities(theme.clone());
            let style = if name == "midnight" {
                LegendStyle::Chips
            } else {
                LegendStyle::Header
            };
            axes(&mut f)
                .legend_at(style, position, umber_core::SizeSpec::Auto)
                .unwrap();
            emit_scene(
                &format!("bubbles-{position_name}-{name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
        let mut f = cities(theme.clone());
        axes(&mut f)
            .bubbles(
                &[20.0, 50.0, 70.0],
                &[6.0, 3.0, 5.0],
                &[1200.0, 400.0, 100.0],
                BubbleOptions {
                    marker: Marker::Diamond,
                    ..options("Towns")
                },
            )
            .unwrap();
        emit_scene(
            &format!("bubbles-shared-scale-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .title("Shares")
            .bubbles(
                &[0.0, 1.0, 2.0],
                &[1.0, 0.5, 0.2],
                &[1.0, 0.5, 0.2],
                options("Share"),
            )
            .unwrap()
            .ytick_format(TickFormat::percent(0).unwrap())
            .unwrap();
        emit_scene(
            &format!("bubbles-percent-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = Figure::with_grid(
            10.0,
            4.0,
            theme,
            Grid::new(1, 2).unwrap().with_legend(LegendMode::Merged),
        )
        .unwrap();
        for col in 0..2 {
            f.add_axes_at(
                Placement {
                    row: GridIndex::At(0),
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                linear(),
                linear(),
            )
            .unwrap()
            .bubbles(&[1.0, 2.0], &[2.0, 3.0], &[100.0, 870.0], options("Cities"))
            .unwrap();
        }
        emit_scene(
            &format!("bubbles-merged-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut f)
        .xlim(0.0, 4.0)
        .unwrap()
        .ylim(0.0, 4.0)
        .unwrap()
        .bubbles(
            &[0.0, 1.0, 2.0, 3.0, 4.0, f64::NAN],
            &[2.0, f64::NAN, 2.0, 3.0, 4.0, 1.0],
            &[100.0, 200.0, 0.0, 400.0, 870.0, 30.0],
            options("Clipped"),
        )
        .unwrap();
    emit_scene("bubbles-gaps-limits", &f.to_scene(lib).unwrap(), sink);
    let mut f = cities(Theme::pigment());
    let mut errors = String::new();
    for (label, x, y, sizes) in [
        ("y length", vec![1.0], vec![], vec![]),
        ("sizes length", vec![1.0], vec![1.0], vec![]),
        ("negative", vec![1.0, 2.0], vec![1.0, 2.0], vec![0.0, -1.0]),
        ("nan", vec![1.0], vec![1.0], vec![f64::NAN]),
        (
            "positive infinity",
            vec![1.0],
            vec![1.0],
            vec![f64::INFINITY],
        ),
        (
            "negative infinity",
            vec![1.0],
            vec![1.0],
            vec![f64::NEG_INFINITY],
        ),
        ("x infinity", vec![f64::INFINITY], vec![1.0], vec![1.0]),
        ("y infinity", vec![1.0], vec![f64::INFINITY], vec![1.0]),
    ] {
        let before = f.clone();
        let err = axes(&mut f)
            .bubbles(&x, &y, &sizes, options(""))
            .unwrap_err();
        assert_eq!(f, before);
        let _ = writeln!(
            errors,
            "{label}: {err}\nparameter={:?} hint={:?}",
            err.parameter(),
            err.hint()
        );
    }
    let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    let err = axes(&mut f)
        .bubbles(&[], &[], &[], options(""))
        .unwrap_err();
    let _ = writeln!(
        errors,
        "category x: {err}\nparameter={:?} hint={:?}",
        err.parameter(),
        err.hint()
    );
    sink(Output {
        kind: "errors",
        file: "bubbles.txt".into(),
        bytes: errors.into_bytes(),
    });
}
