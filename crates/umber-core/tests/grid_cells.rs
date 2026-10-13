//! Panels on a grid draw inside their own cells: their texts do not run into a neighbouring
//! cell, and the marks clip of a panel stops before the next panel's cells.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use umber_core::{
    AxisData, Error, Figure, FontLibrary, Grid, GridIndex, Item, LineOptions, Placement, ScaleKind,
    Scene, SizeSpec, Theme,
};

/// An empty 1 × 2 figure, 12 × 8 in.
fn one_by_two() -> Figure {
    let grid = Grid::new(1, 2).expect("a valid grid");
    Figure::with_grid(12.0, 8.0, Theme::pigment(), grid).expect("a valid size")
}

/// The placement of an axes in one cell.
fn cell(row: i64, col: i64) -> Placement {
    Placement {
        row: GridIndex::At(row),
        col: GridIndex::At(col),
        ..Placement::default()
    }
}

fn scene(f: &Figure) -> Scene {
    f.to_scene(FontLibrary::global())
        .expect("the figure lays out")
}

fn render_error(f: &Figure) -> Error {
    f.to_scene(FontLibrary::global())
        .expect_err("the figure does not lay out")
}

/// The texts of the scene, with their left and right ends.
fn texts(scene: &Scene) -> Vec<(String, f32, f32)> {
    fn walk(items: &[Item], out: &mut Vec<(String, f32, f32)>) {
        for item in items {
            match item {
                Item::Group { items, .. } => walk(items, out),
                Item::Text(text) => {
                    out.push((text.text.clone(), text.origin.x, text.origin.x + text.width))
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&scene.items, &mut out);
    out
}

#[test]
fn a_panel_title_wider_than_its_panel_fails() {
    // A left title of 35 × "W" is about 511 pt wide; the left panel's plot is far narrower. It
    // used to run into the right cell and over the right panel's title.
    let mut f = one_by_two();
    f.add_axes_at(cell(0, 0), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title(&"W".repeat(35));
    f.add_axes_at(cell(0, 1), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Right");
    let err = render_error(&f);
    let message = err.to_string();
    assert!(
        message.starts_with(
            "Figure.to_scene(): row 0, col 0: the title is 511 pt wide; the panel has "
        ),
        "{message}"
    );
    assert!(message.ends_with(" pt."), "{message}");
    assert_eq!(err.hint(), Some("shorten the title or widen the figure."));

    // A title that fits stays left of the right panel's title.
    f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
        .unwrap()
        .title(&"W".repeat(20));
    let texts = texts(&scene(&f));
    let end = |name: &str| {
        texts
            .iter()
            .find(|t| t.0 == name)
            .map(|t| (t.1, t.2))
            .unwrap()
    };
    let (_, left_end) = end(&"W".repeat(20));
    let (right_start, _) = end("Right");
    assert!(left_end < right_start, "{left_end} {right_start}");
}

/// The clip rectangle of the marks group of the panel whose chrome is the `n`th group.
fn marks_clip(scene: &Scene, n: usize) -> umber_core::Rect {
    let clips: Vec<_> = scene
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Group {
                clip: Some(clip), ..
            } => Some(*clip),
            _ => None,
        })
        .collect();
    clips[n]
}

/// A 36 pt wide horizontal line across the plot, limits 0 to 1 on both axes.
fn thick_line(f: &mut Figure, col: i64) {
    f.add_axes_at(cell(0, col), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.5, 0.5],
            LineOptions {
                width: SizeSpec::pt(36.0).unwrap(),
                ..LineOptions::default()
            },
        )
        .unwrap();
}

#[test]
fn a_marks_clip_stops_before_the_next_panel() {
    // The round cap of a thick line at the right end of the left plot used to be drawn into
    // the right cell, over the right panel's y tick labels.
    let mut f = one_by_two();
    thick_line(&mut f, 0);
    thick_line(&mut f, 1);
    let scene = scene(&f);
    let left = marks_clip(&scene, 0);
    let right = marks_clip(&scene, 1);
    let left_end = left.x + left.width;
    // The right panel's y tick labels and its marks clip both start right of the left clip.
    let right_labels = texts(&scene)
        .into_iter()
        .filter(|(text, start, _)| {
            ["0.0", "0.5", "1.0"].contains(&text.as_str()) && *start > left.x + left.width / 2.0
        })
        .map(|(_, start, _)| start)
        .fold(f32::INFINITY, f32::min);
    assert!(right_labels.is_finite());
    assert!(left_end <= right_labels, "{left_end} {right_labels}");
    assert!(left_end < right.x, "{left_end} {}", right.x);
}

/// The right end of the first gridline of the first chrome group: the plot's right edge.
fn first_plot_right(scene: &Scene) -> f32 {
    let chrome = match scene.items.first() {
        Some(Item::Group { items, .. }) => items.as_slice(),
        _ => &[],
    };
    chrome
        .iter()
        .find_map(|item| match item {
            Item::Stroke { path, .. } => path.bounds().map(|(_, max)| max.x),
            _ => None,
        })
        .expect("a gridline")
}

#[test]
fn a_marks_clip_next_to_an_empty_cell_stops_halfway_into_the_gap() {
    // Only the left cell holds axes: half the line's 36 pt stroke would reach 18 pt past the
    // plot's right edge, but the clip ends half the 24 pt gap past it, where the right cell
    // starts.
    let mut f = one_by_two();
    thick_line(&mut f, 0);
    let scene = scene(&f);
    let clip = marks_clip(&scene, 0);
    let plot_right = first_plot_right(&scene);
    let past = clip.x + clip.width - plot_right;
    assert!((past - 12.0).abs() < 1e-3, "{past}");
}
