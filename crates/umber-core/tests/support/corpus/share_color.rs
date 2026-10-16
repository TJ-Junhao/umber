//! Shared color ranges and colorbars across rows, columns and a whole figure.

use super::super::{Output, emit_scene, linear, numbers, themes};
use umber_core::{
    ColorBy, ColorbarOptions, ColorbarPosition, Figure, FontLibrary, Grid, GridIndex, LineOptions,
    Placement, PointColor, ScatterOptions, Share, TickFormat,
};

/// Emits shared ranges, both figure positions and groups with missing panels.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (theme_name, theme) in themes() {
        for (name, share, position) in [
            ("row", Share::Row, None),
            ("col", Share::Col, None),
            ("all-right", Share::All, Some(ColorbarPosition::Right)),
            ("all-bottom", Share::All, Some(ColorbarPosition::Bottom)),
            ("all-hidden", Share::All, Some(ColorbarPosition::None)),
            ("sparse", Share::Col, None),
        ] {
            let grid = Grid::new(2, 2).unwrap().with_share_color(share);
            let mut figure = Figure::with_grid(10.0, 7.0, theme.clone(), grid).unwrap();
            figure.title("Measurements by location");
            figure
                .colorbar(ColorbarOptions {
                    label: "Temperature".into(),
                    position,
                    format: Some(TickFormat::fixed(0).unwrap().affix("", " °C").unwrap()),
                })
                .unwrap();
            for row in 0..2 {
                for col in 0..2 {
                    if name == "sparse" && row == 1 && col == 1 {
                        continue;
                    }
                    let mut ax = figure
                        .add_axes_at(
                            Placement {
                                row: GridIndex::At(row),
                                col: GridIndex::At(col),
                                ..Placement::default()
                            },
                            linear(),
                            linear(),
                        )
                        .unwrap();
                    ax.title(if row == 0 { "Morning" } else { "Afternoon" })
                        .xlabel("Distance")
                        .ylabel("Depth");
                    ax.scatter(
                        numbers(vec![1.0, 2.0, 3.0]),
                        &[1.0, 3.0, 2.0],
                        ScatterOptions {
                            color: PointColor::By(ColorBy {
                                center: Some(0.0),
                                ..ColorBy::new(vec![-2.0, 0.0, 4.0 + (row * 2 + col) as f64 * 2.0])
                            }),
                            ..ScatterOptions::default()
                        },
                    )
                    .unwrap();
                    ax.line(
                        numbers(vec![1.0, 3.0]),
                        &[2.0, 2.0],
                        LineOptions {
                            label: "Reference".into(),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
                }
            }
            emit_scene(
                &format!("share-color-{name}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
