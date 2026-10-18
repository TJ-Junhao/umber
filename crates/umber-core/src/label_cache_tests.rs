//! Category layout with and without reuse of measured labels.

use super::*;
use crate::render::{SvgText, render_png, render_svg};
use crate::{BarOptions, Grid, GridIndex, HeatmapOptions, Placement, ScaleKind, Share};

fn category_figures() -> Vec<Figure> {
    let names: Vec<_> = (0..200).map(|i| format!("c{i}")).collect();
    let mut figures = Vec::new();
    for theme in [
        Theme::pigment(),
        Theme::graphite(),
        Theme::midnight().with_font_scale(1.2).unwrap(),
    ] {
        for horizontal in [false, true] {
            let mut figure = Figure::new(6.0, 4.0, theme.clone()).unwrap();
            let (x, y) = if horizontal {
                (ScaleKind::Linear, ScaleKind::Category)
            } else {
                (ScaleKind::Category, ScaleKind::Linear)
            };
            figure
                .add_axes(x, y)
                .unwrap()
                .bar(names.clone(), &vec![1.0; 200], BarOptions::default())
                .unwrap();
            figures.push(figure);
        }
    }
    let mut figure = Figure::with_grid(
        10.0,
        6.0,
        Theme::pigment(),
        Grid::new(2, 2).unwrap().with_share_x(Share::All),
    )
    .unwrap();
    let labels: Vec<_> = (0..40).map(|i| format!("日 e\u{301} {i}")).collect();
    for row in 0..2 {
        for col in 0..2 {
            figure
                .add_axes_at(
                    Placement {
                        row: GridIndex::At(row),
                        col: GridIndex::At(col),
                        ..Placement::default()
                    },
                    ScaleKind::Category,
                    ScaleKind::Linear,
                )
                .unwrap()
                .bar(labels.clone(), &vec![1.0; 40], BarOptions::default())
                .unwrap();
        }
    }
    figures.push(figure);
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let short: Vec<_> = (0..30).map(|i| format!("x{i}")).collect();
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &short,
            &short,
            &(0..900).map(|i| (i % 23) as f64).collect::<Vec<_>>(),
            HeatmapOptions::default(),
        )
        .unwrap();
    figures.push(figure);
    figures
}

#[test]
fn cached_category_scenes_svg_and_png_are_byte_identical() {
    let lib = FontLibrary::bundled();
    for (i, figure) in category_figures().iter().enumerate() {
        let cached = scene(figure, &lib, "Figure.to_scene()").unwrap();
        let uncached = scene_with_labels(
            figure,
            &lib,
            "Figure.to_scene()",
            &layout::LabelCache::uncached(&lib),
        )
        .unwrap();
        assert_eq!(cached, uncached, "scene {i}");
        for text in [SvgText::Outlines, SvgText::Editable] {
            assert_eq!(
                render_svg(&cached, text),
                render_svg(&uncached, text),
                "SVG {i}"
            );
        }
        assert_eq!(
            render_png(&cached, 72).unwrap(),
            render_png(&uncached, 72).unwrap(),
            "PNG {i}"
        );
        assert_eq!(
            cached,
            scene(figure, &lib, "Figure.to_scene()").unwrap(),
            "fresh cache {i}"
        );
    }
}

#[test]
fn concurrent_category_scenes_do_not_share_a_cache() {
    let lib = FontLibrary::bundled();
    let figures = category_figures();
    let expected: Vec<_> = figures
        .iter()
        .map(|f| scene(f, &lib, "Figure.to_scene()").unwrap())
        .collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = figures
            .iter()
            .map(|figure| {
                let lib = &lib;
                scope.spawn(move || scene(figure, lib, "Figure.to_scene()").unwrap())
            })
            .collect();
        for (handle, expected) in handles.into_iter().zip(expected) {
            assert_eq!(handle.join().unwrap(), expected);
        }
    });
}
