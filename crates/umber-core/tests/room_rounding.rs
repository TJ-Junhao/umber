//! Whole-point room errors must give enough room when their stated sizes are used.

#![allow(clippy::unwrap_used)]

use umber_core::{
    AxesMut, AxisData, ColorBy, ColorbarOptions, ColorbarPosition, Figure, FontLibrary, Grid,
    GridIndex, HeatmapOptions, LegendMode, LegendPosition, LegendStyle, LineOptions, Placement,
    PointColor, ScaleKind, ScatterOptions, Theme, TickFormat,
};

fn figure(width: f64, height: f64, rows: i64, cols: i64) -> Figure {
    Figure::with_grid(
        width / 72.0,
        height / 72.0,
        Theme::midnight(),
        Grid::new(rows, cols)
            .unwrap()
            .with_legend(LegendMode::PerPanel),
    )
    .unwrap()
}

fn axes(f: &mut Figure, row: i64, col: i64) -> AxesMut<'_> {
    let mut ax = f
        .add_axes_at(
            Placement {
                row: GridIndex::At(row),
                col: GridIndex::At(col),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap();
    ax.xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap()
        .legend(LegendStyle::None, Default::default())
        .unwrap();
    ax
}

fn line(ax: &mut AxesMut<'_>, label: &str) {
    ax.line(
        AxisData::Numbers(vec![0.0, 1.0]),
        &[0.0, 1.0],
        LineOptions {
            label: label.into(),
            ..LineOptions::default()
        },
    )
    .unwrap();
}

fn colorbar(ax: &mut AxesMut<'_>, position: ColorbarPosition) {
    ax.scatter(
        AxisData::Numbers(vec![0.0, 1.0]),
        &[0.0, 1.0],
        ScatterOptions {
            color: PointColor::By(ColorBy::new(vec![0.0, 1.0])),
            ..ScatterOptions::default()
        },
    )
    .unwrap()
    .colorbar(ColorbarOptions {
        label: "WWWWWWWWWWWW".into(),
        position: Some(position),
        ..ColorbarOptions::default()
    })
    .unwrap();
}

fn sizes(message: &str) -> (f64, f64) {
    let mut parts = message.split(" pt");
    let mut next = || {
        parts
            .next()
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    (next(), next())
}

/// Find the rendering boundary without depending on a font's exact metrics. A panel or
/// cell gains one point per `scale` canvas points; a fixed band gains one per canvas point.
/// Each failing figure is rebuilt using only the two whole-point sizes in its error.
fn stated_room_suffices(build: impl Fn(f64) -> Figure, scale: f64, cause: &str) {
    stated_room_suffices_at(build, scale, cause, &[1.1, 1.4, 1.7, 2.1]);
}

fn stated_room_suffices_at(
    build: impl Fn(f64) -> Figure,
    scale: f64,
    cause: &str,
    deficits: &[f64],
) {
    let lib = FontLibrary::global();
    let (mut lo, mut hi) = (1.0, 4000.0);
    assert!(build(hi).to_scene(lib).is_ok());
    for _ in 0..32 {
        let middle = (lo + hi) / 2.0;
        if build(middle).to_scene(lib).is_ok() {
            hi = middle;
        } else {
            lo = middle;
        }
    }
    for missing in deficits {
        let initial = hi - scale * missing;
        let error = build(initial).to_scene(lib).unwrap_err();
        let message = error.to_string();
        assert!(message.contains(cause), "expected {cause}: {message}");
        let (needed, available) = sizes(&message);
        let resized = if message.contains("at least") {
            available
        } else {
            assert!(needed > available, "{message}");
            initial + scale * (needed - available)
        };
        let result = build(resized).to_scene(lib);
        assert!(
            result.is_ok(),
            "{initial:.6} -> {resized:.6} pt, using {message}: {}",
            result.unwrap_err()
        );
    }
}

#[test]
fn single_panel_minimum_width_suffices() {
    stated_room_suffices(
        |width| {
            let mut f = figure(width, 288.0, 1, 1);
            axes(&mut f, 0, 0);
            f
        },
        1.0,
        "needed for the plot area",
    );
}

#[test]
fn single_panel_minimum_height_suffices() {
    stated_room_suffices(
        |height| {
            let mut f = figure(432.0, height, 1, 1);
            axes(&mut f, 0, 0).title("Load").subtitle("Daily mean");
            f
        },
        1.0,
        "needed for the plot area",
    );
}

#[test]
fn grid_minimum_width_suffices() {
    stated_room_suffices(
        |width| {
            let mut f = figure(width, 288.0, 1, 3);
            for col in 0..3 {
                axes(&mut f, 0, col);
            }
            f
        },
        1.0,
        "needed for 3 columns of plots",
    );
}

#[test]
fn grid_minimum_height_suffices() {
    stated_room_suffices(
        |height| {
            let mut f = figure(432.0, height, 2, 1);
            for row in 0..2 {
                axes(&mut f, row, 0).title("Load").subtitle("Daily mean");
            }
            f
        },
        1.0,
        "needed for 2 rows of plots",
    );
}

#[test]
fn y_tick_label_difference_suffices() {
    for cols in [1, 2] {
        stated_room_suffices(
            |width| {
                let mut f = figure(width, 288.0, 1, cols);
                axes(&mut f, 0, 0)
                    .yticks_labeled(&[0.5], vec!["W".repeat(12)])
                    .unwrap();
                f
            },
            1.0,
            "y tick label",
        );
    }
}

#[test]
fn x_tick_label_difference_suffices() {
    for cols in [1, 2] {
        stated_room_suffices(
            |width| {
                let mut f = figure(width, 288.0, 1, cols);
                axes(&mut f, 0, 0)
                    .xticks_labeled(&[0.5], vec!["W".repeat(12)])
                    .unwrap();
                f
            },
            cols as f64,
            "x tick label",
        );
    }
}

fn panel_text_difference_suffices(name: &str) {
    stated_room_suffices(
        |width| {
            let mut f = figure(width, 288.0, 1, 2);
            let mut ax = axes(&mut f, 0, 0);
            let text = "WWWWWWWWWWWW";
            match name {
                "title" => ax.title("WWWWWWW"),
                "subtitle" => ax.subtitle(text),
                "x axis label" => ax.xlabel(text),
                _ => ax.ylabel(text),
            };
            f
        },
        2.0,
        &format!("the {name} is"),
    );
}

#[test]
fn title_difference_suffices() {
    panel_text_difference_suffices("title");
}

#[test]
fn subtitle_difference_suffices() {
    panel_text_difference_suffices("subtitle");
}

#[test]
fn x_axis_label_difference_suffices() {
    panel_text_difference_suffices("x axis label");
}

#[test]
fn y_axis_label_difference_suffices() {
    panel_text_difference_suffices("y axis label");
}

#[test]
fn legend_width_difference_suffices() {
    for cols in [1, 2] {
        for style in [LegendStyle::Header, LegendStyle::Chips] {
            for position in [
                LegendPosition::TopLeft,
                LegendPosition::TopRight,
                LegendPosition::Right,
                LegendPosition::Bottom,
                LegendPosition::Best,
            ] {
                if cols == 1 && position == LegendPosition::TopLeft {
                    continue;
                }
                stated_room_suffices(
                    |width| {
                        let mut f = figure(width, 288.0, 1, cols);
                        let mut ax = axes(&mut f, 0, 0);
                        ax.legend_at(style, position, Default::default()).unwrap();
                        line(&mut ax, "WWWWWWWWWWWW");
                        f
                    },
                    if position == LegendPosition::Right {
                        1.0
                    } else {
                        cols as f64
                    },
                    "the legend label",
                );
            }
        }
    }
}

#[test]
fn figure_legend_difference_suffices() {
    for style in [LegendStyle::Header, LegendStyle::Chips] {
        stated_room_suffices(
            |width| {
                let mut f = Figure::with_grid(
                    width / 72.0,
                    4.0,
                    Theme::midnight(),
                    Grid::new(1, 2).unwrap().with_legend(LegendMode::Merged),
                )
                .unwrap();
                let mut ax = axes(&mut f, 0, 0);
                ax.legend(style, Default::default()).unwrap();
                line(&mut ax, "WWWWWWWWWWWW");
                f
            },
            1.0,
            "the figure has",
        );
    }
}

#[test]
fn right_legend_with_colorbar_difference_suffices() {
    stated_room_suffices_at(
        |width| {
            let mut f = figure(width, 288.0, 1, 2);
            let mut ax = axes(&mut f, 0, 0);
            ax.legend_at(
                LegendStyle::Header,
                LegendPosition::Right,
                Default::default(),
            )
            .unwrap();
            line(&mut ax, &"W".repeat(32));
            colorbar(&mut ax, ColorbarPosition::Right);
            f
        },
        1.0,
        "the legend label",
        // Removing the colorbar alone cannot make these figures fit: name the legend.
        &[80.1, 80.4, 80.7, 81.1],
    );
}

#[test]
fn legend_height_difference_suffices() {
    for rows in [1, 2] {
        for position in [LegendPosition::Right, LegendPosition::Best] {
            stated_room_suffices(
                |height| {
                    let mut f = figure(432.0, height, rows, 1);
                    let mut ax = axes(&mut f, 0, 0);
                    ax.legend_at(LegendStyle::Header, position, Default::default())
                        .unwrap();
                    for label in ["North", "South", "East"] {
                        line(&mut ax, label);
                    }
                    f
                },
                rows as f64,
                "the legend is",
            );
        }
    }
}

#[test]
fn direct_label_difference_suffices() {
    stated_room_suffices(
        |width| {
            let mut f = figure(width, 288.0, 1, 2);
            let mut ax = axes(&mut f, 0, 0);
            ax.legend(LegendStyle::Direct, Default::default()).unwrap();
            line(&mut ax, "WWWWWWWWWWWW");
            f
        },
        // The end-label reserve is capped at a quarter of a cell's inner width.
        8.0,
        "the direct label",
    );
}

#[test]
fn colorbar_width_difference_suffices() {
    for cols in [1, 2] {
        stated_room_suffices(
            |width| {
                let mut f = figure(width, 288.0, 1, cols);
                colorbar(&mut axes(&mut f, 0, 0), ColorbarPosition::Right);
                f
            },
            1.0,
            "the colorbar needs",
        );
    }
}

#[test]
fn colorbar_height_difference_suffices() {
    for rows in [1, 2] {
        stated_room_suffices(
            |height| {
                let mut f = figure(432.0, height, rows, 1);
                colorbar(&mut axes(&mut f, 0, 0), ColorbarPosition::Bottom);
                f
            },
            1.0,
            "the colorbar needs",
        );
    }
}

#[test]
fn colorbar_tick_label_difference_suffices() {
    stated_room_suffices(
        |width| {
            let mut f = figure(width, 288.0, 1, 1);
            let mut ax = axes(&mut f, 0, 0);
            colorbar(&mut ax, ColorbarPosition::Bottom);
            ax.colorbar(ColorbarOptions {
                position: Some(ColorbarPosition::Bottom),
                format: Some(
                    TickFormat::fixed(0)
                        .unwrap()
                        .affix("", &"W".repeat(11))
                        .unwrap(),
                ),
                ..ColorbarOptions::default()
            })
            .unwrap();
            f
        },
        // End ticks can move in by a quarter of the inner width, and labels are centered.
        2.0,
        "the colorbar needs",
    );
}

#[test]
fn heatmap_label_width_difference_suffices() {
    heatmap_label_difference_suffices(false);
}

#[test]
fn heatmap_label_height_difference_suffices() {
    heatmap_label_difference_suffices(true);
}

fn heatmap_label_difference_suffices(vertical: bool) {
    stated_room_suffices(
        |side| {
            let (width, height) = if vertical {
                (432.0, side)
            } else {
                (side, 288.0)
            };
            let mut f = figure(width, height, 1, 1);
            f.add_axes(ScaleKind::Category, ScaleKind::Category)
                .unwrap()
                .heatmap(
                    &["".into()],
                    &["".into()],
                    &[1.0],
                    HeatmapOptions {
                        annotate: true,
                        format: Some(TickFormat::fixed(1).unwrap()),
                        ..HeatmapOptions::default()
                    },
                )
                .unwrap()
                .colorbar(ColorbarOptions {
                    position: Some(ColorbarPosition::None),
                    ..ColorbarOptions::default()
                })
                .unwrap();
            f
        },
        1.0,
        if vertical {
            "the heatmap labels are"
        } else {
            "the heatmap label "
        },
    );
}
