//! Crowded category axes on a million-cell heatmap and two hundred bars.

use umber_core::{BarOptions, Figure, FontLibrary, HeatmapOptions, ScaleKind, Theme};

use super::super::{Output, emit_scene};

fn names(prefix: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}{i}")).collect()
}

/// Emits dense category axes in both orientations, without changing the marks themselves.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut heatmap = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let values: Vec<_> = (0..1_000_000)
        .map(|i| ((i / 1000 + i % 1000) % 100) as f64)
        .collect();
    heatmap
        .add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &names("x", 1000),
            &names("y", 1000),
            &values,
            HeatmapOptions::default(),
        )
        .unwrap();
    emit_scene(
        "fig-category-labels-heatmap-1000",
        &heatmap.to_scene(lib).unwrap(),
        sink,
    );

    for (name, x, y) in [
        (
            "fig-category-labels-bars-200-vertical",
            ScaleKind::Category,
            ScaleKind::Linear,
        ),
        (
            "fig-category-labels-bars-200-horizontal",
            ScaleKind::Linear,
            ScaleKind::Category,
        ),
    ] {
        let mut bars = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let values: Vec<_> = (0..200).map(|i| 1.0 + f64::from(i % 20)).collect();
        bars.add_axes(x, y)
            .unwrap()
            .bar(names("c", 200), &values, BarOptions::default())
            .unwrap();
        emit_scene(name, &bars.to_scene(lib).unwrap(), sink);
    }
}
