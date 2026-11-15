//! Titles that shrink to fit their room, headers aligned with the left edge of the panel's
//! content, and the errors of titles too wide at the smallest size they shrink to.

use std::fmt::Write as _;

use umber_core::{
    AxisData, Figure, FontLibrary, Grid, GridIndex, LegendMode, LegendPosition, LegendStyle,
    LineOptions, Placement, ScaleKind, SizeSpec, Theme,
};

use super::super::{Output, emit_scene, record, themes};

const LONG: &str = "Share of days by weather type and season";

fn line(axes: &mut umber_core::AxesMut<'_>, label: &str, y: [f64; 3]) {
    axes.line(
        AxisData::Numbers(vec![0.0, 1.0, 2.0]),
        &y,
        LineOptions {
            label: label.into(),
            ..LineOptions::default()
        },
    )
    .unwrap();
}

/// A narrow single panel whose title shrinks, with a subtitle, a y axis label and a header
/// legend.
fn single(theme: Theme) -> Figure {
    let mut figure = Figure::new(3.5, 3.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title(LONG)
        .subtitle("Daily")
        .ylabel("Share of days (%)")
        .legend(LegendStyle::Header, SizeSpec::Auto)
        .unwrap();
    line(&mut axes, "Sunny", [20.0, 45.0, 30.0]);
    figure
}

/// Three panels: a long title that shrinks, a short one that takes its size, and one with an
/// explicit size that keeps it; header legends per panel and a figure title that fits.
fn grid(theme: Theme) -> Figure {
    let grid = Grid::new(1, 3).unwrap().with_legend(LegendMode::PerPanel);
    let mut figure = Figure::with_grid(9.0, 3.5, theme, grid).unwrap();
    figure.title("Weather").subtitle("Two stations");
    for (col, title, size) in [
        (0, LONG, SizeSpec::Auto),
        (1, "Rain", SizeSpec::Auto),
        (2, "Wind", SizeSpec::pt(12.0).unwrap()),
    ] {
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
        axes.title_sized(title, size)
            .legend(LegendStyle::Header, SizeSpec::Auto)
            .unwrap();
        if col == 0 {
            axes.ylabel("Share of days (%)");
        }
        line(&mut axes, "North", [1.0, 3.0, 2.0]);
    }
    figure
}

/// A figure title that shrinks on its own over a panel whose title fits.
fn figure_title(theme: Theme) -> Figure {
    let mut figure = Figure::new(5.0, 3.0, theme).unwrap();
    figure.title(LONG).subtitle("Two stations");
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Rain");
    line(&mut axes, "", [1.0, 3.0, 2.0]);
    figure
}

/// A title of two lines, the second too wide, which shrinks as one.
fn lines(theme: Theme) -> Figure {
    let mut figure = Figure::new(3.5, 3.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title(&format!("Weather\n{LONG}"));
    line(&mut axes, "", [1.0, 3.0, 2.0]);
    figure
}

/// A 2 × 2 grid with shared axes, y axis labels in the first column and a caption: every
/// header starts at its panel's y axis label or y tick labels.
fn shared_grid(theme: Theme) -> Figure {
    let grid = Grid::new(2, 2)
        .unwrap()
        .with_share_x(umber_core::Share::All)
        .with_share_y(umber_core::Share::All);
    let mut figure = Figure::with_grid(8.0, 5.0, theme, grid).unwrap();
    figure.title("Regional growth").caption("Source: survey");
    for (row, col, title) in [
        (0, 0, "North"),
        (0, 1, "South"),
        (1, 0, "East"),
        (1, 1, "West"),
    ] {
        let mut axes = figure
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
        axes.title(title).subtitle("Weekly");
        if col == 0 {
            axes.ylabel("Orders (k)");
        }
        line(&mut axes, "Orders", [100.0, 1500.0, 900.0]);
    }
    figure
}

/// A single panel whose wide first x tick label insets the plot: the header, its legend row
/// and the y axis label move in together.
fn inset(theme: Theme) -> Figure {
    let theme = theme
        .with_legend_style(LegendStyle::Header)
        .unwrap()
        .with_legend_position(LegendPosition::TopLeft)
        .unwrap();
    let mut figure = Figure::new(6.0, 3.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Days")
        .ylabel("Share")
        .xlim(0.0, 2.0)
        .unwrap()
        .xticks_labeled(
            &[0.0, 2.0],
            vec!["Start of the record".into(), "End".into()],
        )
        .unwrap()
        .yticks_labeled(&[0.0, 50.0], vec!["0".into(), "50".into()])
        .unwrap();
    line(&mut axes, "Sunny", [20.0, 45.0, 30.0]);
    line(&mut axes, "Cloudy", [30.0, 15.0, 40.0]);
    figure
}

/// The errors of titles too wide at their floor and of a title with an explicit size.
fn error_cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut out = String::new();
    let wide = format!("{LONG}, {LONG}");
    let mut figure = Figure::new(2.0, 3.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title(&wide);
    let _ = writeln!(
        out,
        "panel title at its floor: {}",
        record::error(&figure.to_scene(lib).unwrap_err())
    );
    let mut figure = Figure::new(2.0, 3.0, Theme::pigment()).unwrap();
    figure.title(&wide);
    let _ = writeln!(
        out,
        "figure title at its floor: {}",
        record::error(&figure.to_scene(lib).unwrap_err())
    );
    let mut figure = Figure::new(2.0, 3.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .subtitle(&wide);
    let _ = writeln!(
        out,
        "subtitle: {}",
        record::error(&figure.to_scene(lib).unwrap_err())
    );
    let mut figure = Figure::new(3.5, 3.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title_sized(LONG, SizeSpec::times(1.0).unwrap());
    let _ = writeln!(
        out,
        "explicit size: {}",
        record::error(&figure.to_scene(lib).unwrap_err())
    );
    let grid = Grid::new(1, 2).unwrap();
    let mut figure = Figure::with_grid(4.0, 3.0, Theme::pigment(), grid).unwrap();
    for (col, title) in [(0, "Rain"), (1, wide.as_str())] {
        figure
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .title(title);
    }
    let _ = writeln!(
        out,
        "grid title at its floor: {}",
        record::error(&figure.to_scene(lib).unwrap_err())
    );
    sink(Output {
        kind: "errors",
        file: "title-fit.txt".into(),
        bytes: out.into_bytes(),
    });
}

/// Emits every case of this module.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (theme_name, theme) in themes() {
        let figures = [
            ("single", single(theme.clone())),
            ("grid", grid(theme.clone())),
            ("figure-title", figure_title(theme.clone())),
            ("lines", lines(theme.clone())),
            ("shared-grid", shared_grid(theme.clone())),
            ("inset", inset(theme.clone())),
        ];
        for (name, figure) in figures {
            emit_scene(
                &format!("title-fit-{name}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
    error_cases(lib, sink);
}
