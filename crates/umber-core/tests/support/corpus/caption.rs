//! Figure captions: under a single axes, a grid with bottom legends, a colorbar and a pie, in
//! each title alignment and with several lines, and the errors of a caption too wide or holding
//! a control character.

use std::fmt::Write as _;

use umber_core::{
    Align, AxisData, ColorBy, ColorbarOptions, ColorbarPosition, Figure, FontLibrary, Grid,
    GridIndex, LegendPosition, LegendStyle, LineOptions, PieOptions, Placement, PointColor,
    ScaleKind, ScatterOptions, SizeSpec, Theme,
};

use super::super::{Output, emit_scene, record, themes};

const CAPTION: &str = "Long: up to 1,000 epochs; patience 100.";

fn line(axes: &mut umber_core::AxesMut<'_>, label: &str, y: [f64; 4]) {
    axes.line(
        AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]),
        &y,
        LineOptions {
            label: label.into(),
            ..LineOptions::default()
        },
    )
    .unwrap();
}

/// A single axes with a title, axis labels and a caption.
fn single(theme: Theme) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    figure.title("Training loss");
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.xlabel("Epoch").ylabel("Loss");
    line(&mut axes, "", [1.0, 3.0, 2.0, 4.0]);
    figure.caption(CAPTION);
    figure
}

/// Two panels, each with a bottom legend, and a caption.
fn grid_with_bottom_legend(theme: Theme) -> Figure {
    let theme = theme
        .with_legend_style(LegendStyle::Header)
        .unwrap()
        .with_legend_position(LegendPosition::Bottom)
        .unwrap();
    let mut figure = Figure::with_grid(9.0, 4.5, theme, Grid::new(1, 2).unwrap()).unwrap();
    figure.title("Learning rate schedules");
    for (col, title) in [(0, "Cosine"), (1, "Step")] {
        let mut axes = figure
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        axes.title(title).xlabel("Epoch");
        line(&mut axes, "Short", [1.0, 2.0, 1.5, 2.5]);
        line(&mut axes, "Long", [2.5, 1.5, 2.7, 2.0]);
    }
    figure.caption(
        "Long: up to 1,000 epochs; patience 100.\nCosine horizon and stopping also change.",
    );
    figure
}

/// A scatter with a bottom colorbar and a caption.
fn colorbar(theme: Theme) -> Figure {
    let mut figure = Figure::new(6.4, 4.4, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlabel("Distance, km")
        .scatter(
            AxisData::Numbers(vec![1.0, 2.0, 3.0, 4.0]),
            &[4.0, 2.0, 3.0, 1.0],
            ScatterOptions {
                color: PointColor::By(ColorBy::new(vec![-2.0, 0.0, 3.0, 8.0])),
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "Temperature".into(),
            position: Some(ColorbarPosition::Bottom),
            ..ColorbarOptions::default()
        })
        .unwrap();
    figure.caption("Source: field survey, 2026");
    figure
}

/// A pie with a caption.
fn pie(theme: Theme) -> Figure {
    let mut figure = Figure::new(5.0, 4.0, theme).unwrap();
    figure
        .add_pie_axes()
        .unwrap()
        .title("Orders by region")
        .pie(
            vec!["North".into(), "South".into(), "West".into()],
            &[5.0, 3.0, 2.0],
            PieOptions::default(),
        )
        .unwrap();
    figure.caption("Share of 2026 orders");
    figure
}

/// The errors of a caption too wide for its figure and of a caption with a control character.
fn error_cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut out = String::new();
    for (name, text, size, width) in [
        ("too wide", CAPTION, SizeSpec::Auto, 2.0),
        (
            "too wide, scaled",
            CAPTION,
            SizeSpec::times(1.5).unwrap(),
            3.0,
        ),
        (
            "too wide, points",
            CAPTION,
            SizeSpec::pt(12.0).unwrap(),
            2.0,
        ),
        ("tab", "ab\tc", SizeSpec::Auto, 6.0),
        ("null", "ab\0c", SizeSpec::Auto, 6.0),
    ] {
        let mut figure = Figure::new(width, 3.0, Theme::pigment()).unwrap();
        figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        figure.caption_sized(text, size);
        let error = figure.to_scene(lib).unwrap_err();
        let _ = writeln!(out, "{name}: {}", record::error(&error));
    }
    let mut figure = Figure::new(6.0, 0.6, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    figure.caption("A note\nof two lines");
    let _ = writeln!(
        out,
        "too short: {}",
        record::error(&figure.to_scene(lib).unwrap_err())
    );
    let mut figure = Figure::new(6.0, 0.4, Theme::pigment()).unwrap();
    figure.title("Only a header").caption("A note");
    let _ = writeln!(
        out,
        "too short, no axes: {}",
        record::error(&figure.to_scene(lib).unwrap_err())
    );
    sink(Output {
        kind: "errors",
        file: "caption.txt".into(),
        bytes: out.into_bytes(),
    });
}

/// Emits every case of this module.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut api = String::new();
    for (theme_name, theme) in themes() {
        let figures = [
            ("single", single(theme.clone())),
            ("grid-bottom-legend", grid_with_bottom_legend(theme.clone())),
            ("colorbar", colorbar(theme.clone())),
            ("pie", pie(theme.clone())),
        ];
        for (name, figure) in figures {
            let _ = writeln!(api, "{theme_name} {name}: {:?}", figure.caption_text());
            emit_scene(
                &format!("caption-{name}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
        for (name, align) in [
            ("left", Align::Left),
            ("center", Align::Center),
            ("right", Align::Right),
        ] {
            let figure = single(theme.with_title_align(align).unwrap());
            emit_scene(
                &format!("caption-align-{name}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
        let mut sized = single(theme.clone());
        sized.caption_sized("A larger note", SizeSpec::times(1.5).unwrap());
        emit_scene(
            &format!("caption-size-{theme_name}"),
            &sized.to_scene(lib).unwrap(),
            sink,
        );
    }
    sink(Output {
        kind: "api",
        file: "captions.txt".into(),
        bytes: api.into_bytes(),
    });
    error_cases(lib, sink);
}
