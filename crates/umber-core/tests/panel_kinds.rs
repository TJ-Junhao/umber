//! Mixed panel kinds remain distinguishable through copies and row-major traversal.

use umber_core::{Figure, Grid, GridIndex, Panel, Placement, ScaleKind, Theme};

#[test]
fn panels_expose_their_kind_without_cartesian_state_on_pies() {
    let mut figure =
        Figure::with_grid(9.0, 4.0, Theme::pigment(), Grid::new(1, 3).unwrap()).unwrap();
    figure
        .add_pie_axes_at(Placement {
            col: GridIndex::At(2),
            ..Default::default()
        })
        .unwrap()
        .title("Pie");
    figure
        .add_axes_at(
            Placement {
                col: GridIndex::At(1),
                ..Default::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap()
        .title("Cartesian");
    figure
        .add_pie_axes_at(Placement {
            col: GridIndex::At(0),
            ..Default::default()
        })
        .unwrap()
        .title("Empty pie");
    let copy = figure.clone();
    let mut kinds = Vec::new();
    for panel in copy.panels() {
        kinds.push(match panel {
            Panel::Cartesian { axes, .. } => ("cartesian", axes.title_text()),
            Panel::Pie(pie) => ("pie", pie.title_text()),
        });
    }
    assert_eq!(
        kinds,
        [
            ("pie", "Empty pie"),
            ("cartesian", "Cartesian"),
            ("pie", "Pie")
        ]
    );
}
