//! Equal aspect in every theme: a circle on a wide figure, a circle of points with a colorbar on
//! a tall figure, and a grid of two panels of which one has the setting.

use umber_core::{
    AxesMut, AxisData, ColorBy, ColorbarOptions, Figure, FontLibrary, Grid, GridIndex, LegendMode,
    LineOptions, Placement, PointColor, ScaleKind, ScatterOptions, Theme,
};

use super::super::{Output, emit_scene, themes};

/// A unit circle, as `count` steps around it.
fn circle(count: u32) -> (Vec<f64>, Vec<f64>) {
    (0..=count)
        .map(|i| {
            let angle = std::f64::consts::TAU * f64::from(i) / f64::from(count);
            (angle.cos(), angle.sin())
        })
        .unzip()
}

fn line(axes: &mut AxesMut<'_>) {
    let (x, y) = circle(180);
    axes.line(AxisData::Numbers(x), &y, LineOptions::default())
        .unwrap();
}

/// (a) A circle drawn with `line()` on a wide figure.
fn wide(theme: Theme) -> Figure {
    let mut figure = Figure::new(8.0, 3.5, theme).unwrap();
    figure.caption("Unit circle, drawn to scale.");
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Circular domain")
        .subtitle("One data unit is as long on both axes")
        .xlabel("x (m)")
        .ylabel("y (m)");
    line(&mut axes);
    axes.equal_aspect().unwrap();
    figure
}

/// (b) The same on a tall figure, with a colorbar from a scatter `ColorBy`.
fn tall(theme: Theme) -> Figure {
    let mut figure = Figure::new(4.0, 7.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    let (x, y) = circle(48);
    let phase: Vec<f64> = (0..x.len()).map(|i| i as f64 / 48.0).collect();
    axes.title("Phase around the circle").xlabel("x (m)");
    axes.scatter(
        AxisData::Numbers(x),
        &y,
        ScatterOptions {
            color: PointColor::By(ColorBy::new(phase)),
            ..ScatterOptions::default()
        },
    )
    .unwrap();
    axes.colorbar(ColorbarOptions {
        label: "Phase".into(),
        ..ColorbarOptions::default()
    })
    .unwrap();
    axes.equal_aspect().unwrap();
    figure
}

/// (c) A 1 × 2 grid with one equal-aspect panel.
fn grid(theme: Theme) -> Figure {
    let cells = Grid::new(1, 2).unwrap().with_legend(LegendMode::PerPanel);
    let mut figure = Figure::with_grid(9.0, 3.5, theme, cells).unwrap();
    for (col, title) in [(0, "Free shape"), (1, "Equal aspect")] {
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
        axes.title(title).xlabel("x (m)");
        line(&mut axes);
        if col == 1 {
            axes.equal_aspect().unwrap();
        }
    }
    figure
}

/// Emits the three figures in every theme.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (case, figure) in [
            ("wide", wide(theme.clone())),
            ("tall", tall(theme.clone())),
            ("grid", grid(theme.clone())),
        ] {
            emit_scene(
                &format!("equal-aspect-{case}-{name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
