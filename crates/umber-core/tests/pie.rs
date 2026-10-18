//! Circular pie geometry and slice ordering.

use umber_core::{
    Figure, FontLibrary, Item, LegendStyle, Path, PathVerb, PieOptions, Point, SizeSpec, Theme,
};

fn fills(items: &[Item]) -> Vec<&Path> {
    items
        .iter()
        .flat_map(|item| match item {
            Item::Group { items, .. } => fills(items),
            Item::Fill { path, .. } => vec![path],
            _ => vec![],
        })
        .collect()
}

fn endpoints(path: &Path) -> Vec<Point> {
    path.verbs()
        .iter()
        .filter_map(|verb| match verb {
            PathVerb::MoveTo(p)
            | PathVerb::LineTo(p)
            | PathVerb::QuadTo(_, p)
            | PathVerb::CubicTo(_, _, p) => Some(*p),
            PathVerb::Close => None,
        })
        .collect()
}

#[test]
fn slices_start_at_twelve_clockwise() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = figure.add_pie_axes().unwrap();
    axes.pie(
        vec!["A".into(), "B".into()],
        &[1.0, 3.0],
        PieOptions::default(),
    )
    .unwrap();
    axes.legend(LegendStyle::None, None, SizeSpec::Auto)
        .unwrap();
    let scene = figure.to_scene(&FontLibrary::bundled()).unwrap();
    let paths = fills(&scene.items);
    assert_eq!(paths.len(), 2);
    let a = endpoints(paths[0]);
    // The first outer point is at twelve; the first arc ends at three.
    let top = a[0];
    let right = a[1];
    let center = *a.last().unwrap();
    assert!((top.x - center.x).abs() < 0.001);
    assert!(top.y < center.y);
    assert!(right.x > center.x);
    assert!((right.y - center.y).abs() < 0.001);
}

#[test]
fn pie_is_circular_in_wide_panel() {
    let mut figure = Figure::new(10.0, 3.0, Theme::graphite()).unwrap();
    let mut axes = figure.add_pie_axes().unwrap();
    axes.pie(vec!["A".into()], &[1.0], PieOptions::default())
        .unwrap();
    axes.legend(LegendStyle::None, None, SizeSpec::Auto)
        .unwrap();
    let scene = figure.to_scene(&FontLibrary::bundled()).unwrap();
    let paths = fills(&scene.items);
    let points = endpoints(paths[0]);
    let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let max_x = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let min_y = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_y = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    assert!((max_x - min_x - (max_y - min_y)).abs() < 0.001);
    assert!((max_y - min_y - 192.0).abs() < 0.001);
}

fn leaves(items: &[Item]) -> Vec<&Item> {
    items
        .iter()
        .flat_map(|item| match item {
            Item::Group { items, .. } => leaves(items),
            _ => vec![item],
        })
        .collect()
}

#[test]
fn outside_labels_stay_outside_the_circle_after_spreading() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for hole in [0.0, 0.5] {
            let mut figure = Figure::new(6.0, 4.0, theme.clone()).unwrap();
            figure
                .add_pie_axes()
                .unwrap()
                .pie(
                    ["Web", "App", "Shop", "Other"].map(String::from).to_vec(),
                    &[80.0, 15.0, 3.0, 2.0],
                    PieOptions {
                        hole,
                        annotate: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            let library = FontLibrary::bundled();
            let scene = figure.to_scene(&library).unwrap();
            let fonts = library.with_figure_fonts(&scene.fonts).unwrap();
            let bounds: Vec<_> = fills(&scene.items)
                .iter()
                .map(|path| path.bounds().unwrap())
                .collect();
            let left = bounds
                .iter()
                .map(|(lo, _)| lo.x)
                .fold(f32::INFINITY, f32::min);
            let right = bounds
                .iter()
                .map(|(_, hi)| hi.x)
                .fold(f32::NEG_INFINITY, f32::max);
            let top = bounds
                .iter()
                .map(|(lo, _)| lo.y)
                .fold(f32::INFINITY, f32::min);
            let bottom = bounds
                .iter()
                .map(|(_, hi)| hi.y)
                .fold(f32::NEG_INFINITY, f32::max);
            let center = Point {
                x: (left + right) / 2.0,
                y: (top + bottom) / 2.0,
            };
            let radius = (right - left) / 2.0;
            let mut outside = 0;
            for item in leaves(&scene.items) {
                let Item::Text(text) = item else { continue };
                if !matches!(text.text.as_str(), "Shop 3%" | "Other 2%") {
                    continue;
                }
                outside += 1;
                let metrics = fonts.layout(&text.text, &text.style).unwrap();
                let nearest_x = center.x.clamp(text.origin.x, text.origin.x + text.width);
                let nearest_y = center.y.clamp(
                    text.origin.y - metrics.ascent(),
                    text.origin.y + metrics.descent(),
                );
                let distance = (nearest_x - center.x).hypot(nearest_y - center.y);
                assert!(distance >= radius, "{} overlaps the pie", text.text);
            }
            assert_eq!(outside, 2);
        }
    }
}

#[test]
fn outside_leaders_stay_inside_the_panel() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_pie_axes()
        .unwrap()
        .pie(
            ["Web", "App", "Shop", "Other"].map(String::from).to_vec(),
            &[80.0, 15.0, 3.0, 2.0],
            PieOptions {
                annotate: true,
                ..Default::default()
            },
        )
        .unwrap();
    let scene = figure.to_scene(&FontLibrary::bundled()).unwrap();
    let mut leaders = 0;
    for item in leaves(&scene.items) {
        if let Item::Stroke { path, style } = item
            && style.width == 0.75
        {
            leaders += 1;
            for point in endpoints(path) {
                assert!((12.0..=420.0).contains(&point.x), "leader x: {}", point.x);
                assert!((12.0..=276.0).contains(&point.y), "leader y: {}", point.y);
            }
        }
    }
    assert_eq!(leaders, 2);
}
