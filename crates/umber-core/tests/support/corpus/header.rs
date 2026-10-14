//! Figure titles, title alignment, shared colors and merged or per-panel legends.

use umber_core::{
    Align, AxisData, BarOptions, Figure, FontLibrary, Grid, GridIndex, LegendMode, LineOptions,
    Placement, ScaleKind, Theme,
};

use super::super::{Output, emit_scene, themes};

fn grid(theme: Theme, mode: LegendMode, panel_titles: bool) -> Figure {
    let grid = Grid::new(1, 2).unwrap().with_legend(mode);
    let mut figure = Figure::with_grid(9.0, 4.0, theme, grid).unwrap();
    figure
        .title("Regional growth")
        .subtitle("Weekly totals, 2026");
    // Add the right panel first: colors and legend entries still follow cell order.
    for col in [1, 0] {
        let mut axes = figure
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Category,
                ScaleKind::Linear,
            )
            .unwrap();
        if panel_titles {
            axes.title(if col == 0 { "This year" } else { "Last year" })
                .subtitle("By week");
        }
        let x = vec!["One".into(), "Two".into(), "Three".into()];
        for (label, y) in [("North", [2.0, 4.0, 3.0]), ("South", [1.0, 2.0, 5.0])] {
            axes.line(
                AxisData::Categories(x.clone()),
                &y,
                LineOptions {
                    label: label.into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        axes.bar(
            x,
            &[1.0, 1.5, 2.0],
            BarOptions {
                label: "North".into(),
                ..BarOptions::default()
            },
        )
        .unwrap();
    }
    figure
}

/// Emits header scenes and the new title-alignment accessors without changing older API records.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut api = String::new();
    for (theme_name, theme) in themes() {
        for (name, align) in [
            ("left", Align::Left),
            ("center", Align::Center),
            ("right", Align::Right),
        ] {
            let aligned = theme.with_title_align(align).unwrap();
            api.push_str(&format!(
                "{theme_name} {name}: {:?}\n",
                aligned.title_align()
            ));
            let figure = grid(aligned, LegendMode::Merged, true);
            emit_scene(
                &format!("header-align-{name}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
        for (name, mode, panels) in [
            ("merged", LegendMode::Merged, true),
            ("per-panel", LegendMode::PerPanel, true),
            ("no-panel-titles", LegendMode::Merged, false),
        ] {
            let figure = grid(theme.clone(), mode, panels);
            emit_scene(
                &format!("header-{name}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
        let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
        figure.title("Regional growth").subtitle("One panel");
        figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .title("This year")
            .subtitle("Weekly totals")
            .line(
                AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                &[1.0, 3.0, 2.0],
                LineOptions {
                    label: "North".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        api.push_str(&format!(
            "{theme_name} figure: {:?} / {:?}\n",
            figure.title_text(),
            figure.subtitle_text()
        ));
        emit_scene(
            &format!("header-single-{theme_name}"),
            &figure.to_scene(lib).unwrap(),
            sink,
        );
    }
    sink(Output {
        kind: "api",
        file: "headers.txt".into(),
        bytes: api.into_bytes(),
    });
}
