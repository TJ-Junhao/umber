//! Emphasis in every theme: a confusion matrix of twelve sound classes whose machine sounds are
//! highlighted on both axes, with the confusions within that group outlined, zero cells left
//! empty, values written on the cells and edges between them; the same with an explicit outline
//! color; and the same with its cells drawn as one image.

use std::sync::Arc;

use umber_core::{ColorSpec, FontLibrary, HeatmapOptions, Render, Rgba, ScaleKind};

use super::super::{Output, axes, emit_scene, figure, themes};

/// The sound classes, the machine sounds among the others.
const CLASSES: [&str; 12] = [
    "dog", "cat", "fan", "hen", "drill", "cow", "rain", "saw", "sea", "jet", "fire", "wind",
];

/// The machine sounds: the group that is highlighted.
const MACHINES: [&str; 4] = ["fan", "drill", "saw", "jet"];

/// Predictions per true class, row by row: one row per true class, one column per predicted
/// class.
const COUNTS: [[u32; 12]; 12] = [
    [38, 2, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0],
    [3, 35, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 31, 0, 4, 0, 0, 2, 0, 3, 0, 1],
    [1, 2, 0, 36, 0, 1, 0, 0, 0, 0, 0, 0],
    [0, 0, 5, 0, 29, 0, 0, 6, 0, 0, 0, 0],
    [2, 0, 0, 1, 0, 37, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 33, 0, 4, 0, 1, 2],
    [0, 0, 1, 0, 7, 0, 0, 30, 0, 2, 0, 0],
    [0, 0, 0, 0, 0, 0, 5, 0, 34, 0, 0, 1],
    [0, 0, 4, 0, 0, 0, 0, 1, 0, 33, 0, 2],
    [0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 39, 0],
    [0, 0, 2, 0, 0, 0, 3, 0, 1, 0, 0, 34],
];

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

/// The counts row by row, zero counts as NaN so that their cells stay empty.
fn values() -> Vec<f64> {
    COUNTS
        .iter()
        .flatten()
        .map(|&count| {
            if count == 0 {
                f64::NAN
            } else {
                f64::from(count)
            }
        })
        .collect()
}

/// The confusions within the machine sounds: off the diagonal, non-zero, with both the true and
/// the predicted class a machine sound.
fn within_group() -> Arc<[bool]> {
    let machine = |i: usize| MACHINES.contains(&CLASSES[i]);
    (0..CLASSES.len() * CLASSES.len())
        .map(|i| {
            let (row, column) = (i / CLASSES.len(), i % CLASSES.len());
            machine(row) && machine(column) && row != column && COUNTS[row][column] > 0
        })
        .collect()
}

/// Emits the highlighted, outline-colored and raster confusion matrices.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let classes = names(&CLASSES);
    for (name, theme) in themes() {
        for (case, outline_color, render) in [
            ("confusion", ColorSpec::AUTO, Render::Vector),
            (
                "outline-color",
                ColorSpec::Fixed(Rgba::BLUE),
                Render::Vector,
            ),
            ("raster", ColorSpec::AUTO, Render::Raster),
        ] {
            let mut f = figure(
                7.2,
                6.0,
                theme.clone(),
                ScaleKind::Category,
                ScaleKind::Category,
            );
            axes(&mut f)
                .title("Confusions among the machine sounds")
                .xlabel("Predicted class")
                .ylabel("True class")
                .heatmap(
                    &classes,
                    &classes,
                    &values(),
                    HeatmapOptions {
                        annotate: true,
                        edges: true,
                        outline: Some(within_group()),
                        outline_color,
                        render,
                        ..HeatmapOptions::default()
                    },
                )
                .unwrap()
                .xtick_highlight(names(&MACHINES), ColorSpec::AUTO)
                .unwrap()
                .ytick_highlight(names(&MACHINES), ColorSpec::AUTO)
                .unwrap();
            emit_scene(
                &format!("emphasis-{case}-{name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
