//! Panel legends above, beside, below and inside the plot.

use umber_core::{
    AxesMut, AxisData, Figure, FontLibrary, Grid, GridIndex, LegendMode, LegendPosition,
    LegendStyle, LineOptions, Placement, ScaleKind, ScatterOptions, Theme,
};

use super::super::{Output, emit_scene, record, themes};

const POSITIONS: [(&str, LegendPosition); 5] = [
    ("top-left", LegendPosition::TopLeft),
    ("top-right", LegendPosition::TopRight),
    ("right", LegendPosition::Right),
    ("bottom", LegendPosition::Bottom),
    ("best", LegendPosition::Best),
];

fn lines(axes: &mut AxesMut<'_>) {
    axes.title("Regional output")
        .subtitle("Weekly means")
        .xlabel("Week")
        .ylabel("MW");
    for (label, y) in [
        ("North", [1.0, 2.0, 1.5, 2.7]),
        ("South", [2.5, 1.5, 2.7, 2.0]),
    ] {
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
}

/// Emits all positions, corner occupancy, mixed grid positions and the new API accessors.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut api = String::new();
    for (theme_name, theme) in themes() {
        api.push_str(&format!(
            "{theme_name} default={:?} alpha={}\n",
            theme.legend_position(),
            theme.legend_background_alpha()
        ));
        // Graphite's direct labels have no position: use its row style explicitly.
        let style = if theme.legend_style() == LegendStyle::Direct {
            LegendStyle::Header
        } else {
            theme.legend_style()
        };
        for (name, position) in POSITIONS {
            let theme = theme
                .with_legend_position(position)
                .unwrap()
                .with_legend_style(style)
                .unwrap();
            let mut figure = Figure::new(6.0, 4.0, theme.clone()).unwrap();
            lines(
                &mut figure
                    .add_axes(ScaleKind::Linear, ScaleKind::Linear)
                    .unwrap(),
            );
            api.push_str(&format!(
                "{theme_name} {name}={:?}\n",
                theme.legend_position()
            ));
            emit_scene(
                &format!("legend-position-{name}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
        for (corner, x, y) in [
            ("top-left", 0.08, 0.9),
            ("top-right", 0.92, 0.9),
            ("bottom-left", 0.08, 0.1),
            ("bottom-right", 0.92, 0.1),
        ] {
            let mut figure = Figure::new(6.0, 4.0, theme.clone()).unwrap();
            figure
                .add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .title("Observations")
                .legend_at(style, LegendPosition::Best, Default::default())
                .unwrap()
                .xlim(0.0, 1.0)
                .unwrap()
                .ylim(0.0, 1.0)
                .unwrap()
                .scatter(
                    AxisData::Numbers(vec![x - 0.025, x, x + 0.025]),
                    &[y - 0.025, y, y + 0.025],
                    ScatterOptions {
                        label: "Samples".into(),
                        ..ScatterOptions::default()
                    },
                )
                .unwrap();
            emit_scene(
                &format!("legend-position-best-data-{corner}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
        let mut figure = Figure::with_grid(
            12.0,
            8.0,
            theme,
            Grid::new(2, 2).unwrap().with_legend(LegendMode::PerPanel),
        )
        .unwrap();
        for (i, position) in [
            LegendPosition::TopRight,
            LegendPosition::Right,
            LegendPosition::Bottom,
            LegendPosition::Best,
        ]
        .into_iter()
        .enumerate()
        {
            let mut axes = figure
                .add_axes_at(
                    Placement {
                        row: GridIndex::At((i / 2) as i64),
                        col: GridIndex::At((i % 2) as i64),
                        ..Placement::default()
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap();
            axes.legend_at(style, position, Default::default()).unwrap();
            lines(&mut axes);
        }
        emit_scene(
            &format!("legend-position-grid-{theme_name}"),
            &figure.to_scene(lib).unwrap(),
            sink,
        );
    }
    for (_, position) in POSITIONS {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        let error = axes
            .legend_at(LegendStyle::Direct, position, Default::default())
            .unwrap_err();
        api.push_str(&format!("{}\n", record::error(&error)));
        api.push_str(&format!(
            "unchanged style={:?} position={:?}\n",
            axes.legend_style(),
            axes.legend_position()
        ));
    }
    let mut errors = String::new();
    for style in [LegendStyle::Header, LegendStyle::Chips] {
        for grid in [false, true] {
            for (position, tall) in [
                (LegendPosition::TopRight, false),
                (LegendPosition::Right, false),
                (LegendPosition::Bottom, false),
                (LegendPosition::Best, false),
                (LegendPosition::Right, true),
                (LegendPosition::Best, true),
            ] {
                let mut figure = if grid {
                    Figure::with_grid(
                        6.0,
                        4.0,
                        Theme::pigment(),
                        Grid::new(1, 2).unwrap().with_legend(LegendMode::PerPanel),
                    )
                    .unwrap()
                } else {
                    Figure::new(6.0, 4.0, Theme::pigment()).unwrap()
                };
                let mut axes = figure
                    .add_axes_at(
                        Placement {
                            row: GridIndex::At(0),
                            col: GridIndex::At(i64::from(grid)),
                            ..Placement::default()
                        },
                        ScaleKind::Linear,
                        ScaleKind::Linear,
                    )
                    .unwrap();
                axes.legend_at(style, position, Default::default())
                    .unwrap()
                    .xlim(0.0, 1.0)
                    .unwrap()
                    .ylim(0.0, 1.0)
                    .unwrap();
                for i in 0..if tall { 30 } else { 1 } {
                    axes.line(
                        AxisData::Numbers(vec![0.0, 1.0]),
                        &[0.0, 1.0],
                        LineOptions {
                            label: if tall {
                                format!("Series {i}")
                            } else {
                                "W".repeat(80)
                            },
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
                }
                let before = figure.clone();
                let error = figure.to_scene(lib).unwrap_err();
                errors.push_str(&format!(
                    "{style:?} grid={grid} {position:?} tall={tall}\n{}\nunchanged={}\n",
                    record::error(&error),
                    figure == before
                ));
            }
        }
    }
    sink(Output {
        kind: "errors",
        file: "legend_position.txt".into(),
        bytes: errors.into_bytes(),
    });
    sink(Output {
        kind: "api",
        file: "legend-positions.txt".into(),
        bytes: api.into_bytes(),
    });
}
