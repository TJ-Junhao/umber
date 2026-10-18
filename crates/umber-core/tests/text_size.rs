//! Text size overrides reach scene text and the layout that reserves its room.

#![allow(clippy::unwrap_used)]

use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    Figure, FontLibrary, Grid, GridIndex, LegendStyle, Placement, ScaleKind, Share, SizeSpec, Theme,
};

fn texts(scene: &Scene) -> Vec<&TextItem> {
    fn collect<'a>(items: &'a [Item], out: &mut Vec<&'a TextItem>) {
        for item in items {
            match item {
                Item::Text(text) => out.push(text),
                Item::Group { items, .. } => collect(items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    collect(&scene.items, &mut out);
    out
}

#[test]
fn pt_size_ignores_font_scale() {
    for scale in [0.5, 1.0, 2.0] {
        let mut figure =
            Figure::new(12.0, 10.0, Theme::pigment().with_font_scale(scale).unwrap()).unwrap();
        figure.title_sized("Figure", SizeSpec::pt(12.0).unwrap());
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.title_sized("Panel", SizeSpec::pt(12.0).unwrap())
            .subtitle_sized("Subtitle", SizeSpec::pt(12.0).unwrap())
            .xlabel_sized("Horizontal", SizeSpec::pt(12.0).unwrap())
            .ylabel_sized("Vertical", SizeSpec::pt(12.0).unwrap());
        axes.xtick_labels(SizeSpec::pt(12.0).unwrap()).unwrap();
        axes.ytick_labels(SizeSpec::pt(12.0).unwrap()).unwrap();
        let scene = figure.to_scene(FontLibrary::global()).unwrap();
        assert!(texts(&scene).iter().all(|text| text.style.size == 12.0));
    }
}

#[test]
fn automatic_tick_spacing_uses_each_axis_size() {
    for axis in [0, 1] {
        let mut figure = Figure::new(8.0, 6.0, Theme::pigment()).unwrap();
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.xlim(0.0, 1.0).unwrap().ylim(0.0, 1.0).unwrap();
        if axis == 0 {
            axes.xtick_labels(SizeSpec::pt(36.0).unwrap()).unwrap();
        } else {
            axes.ytick_labels(SizeSpec::pt(36.0).unwrap()).unwrap();
        }
        let scene = figure.to_scene(FontLibrary::global()).unwrap();
        let mut labels: Vec<_> = texts(&scene)
            .into_iter()
            .filter(|text| text.style.size == 36.0)
            .collect();
        assert!(labels.len() >= 2);
        labels.sort_by(|a, b| {
            if axis == 0 {
                a.origin.x.total_cmp(&b.origin.x)
            } else {
                a.origin.y.total_cmp(&b.origin.y)
            }
        });
        for pair in labels.windows(2) {
            if axis == 0 {
                assert!(pair[1].origin.x >= pair[0].origin.x + pair[0].width + 18.0 - 1e-3);
            } else {
                let metrics = FontLibrary::global().layout("", &pair[0].style);
                let height = metrics.ascent() + metrics.descent();
                assert!(pair[1].origin.y >= pair[0].origin.y + height + 18.0 - 1e-3);
            }
        }
    }
}

#[test]
fn shared_tick_size_conflict_rejected_without_mutating_figure() {
    let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
    let mut figure = Figure::with_grid(12.0, 4.0, Theme::pigment(), grid).unwrap();
    let left = Placement {
        col: GridIndex::At(0),
        ..Placement::default()
    };
    let right = Placement {
        col: GridIndex::At(1),
        ..Placement::default()
    };
    figure
        .add_axes_at(left, ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xtick_labels(SizeSpec::pt(8.0).unwrap())
        .unwrap();
    figure
        .add_axes_at(right, ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    let before = figure.clone();
    let error = figure
        .axes_at_mut(GridIndex::Auto, GridIndex::At(1))
        .unwrap()
        .xtick_labels(SizeSpec::pt(9.0).unwrap())
        .unwrap_err();
    assert_eq!(error.parameter(), Some("size"));
    assert_eq!(error.hint(), Some("set the same size on every panel."));
    assert_eq!(figure, before);
}

#[test]
fn merged_legend_size_conflict_rejected_without_mutating_figure() {
    let mut figure =
        Figure::with_grid(12.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in [0, 1] {
        figure
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
    }
    figure
        .axes_at_mut(GridIndex::Auto, GridIndex::At(0))
        .unwrap()
        .legend(LegendStyle::Header, SizeSpec::pt(8.0).unwrap())
        .unwrap();
    let before = figure.clone();
    let error = figure
        .axes_at_mut(GridIndex::Auto, GridIndex::At(1))
        .unwrap()
        .legend(LegendStyle::None, SizeSpec::pt(9.0).unwrap())
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Axes.legend(): size is Size.pt(9); another panel's legend uses Size.pt(8), and the figure shows one legend."
    );
    assert_eq!(figure, before);
}
