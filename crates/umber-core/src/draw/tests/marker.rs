//! Marker shapes.

use super::super::marker::marker_shape;
use super::{mark_group, scatter_figure, subpaths};
use crate::geometry::Point;
use crate::scene::Item;
use crate::{FontLibrary, Marker, Theme};

#[test]
fn marker_shapes_have_expected_bounds() {
    let d = 6.0_f32;
    let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
    let bounds = |m: Marker| marker_shape(m, d).bounds().unwrap();

    let (min, max) = bounds(Marker::Circle);
    assert!(close(min.x, -3.0) && close(min.y, -3.0) && close(max.x, 3.0) && close(max.y, 3.0));
    assert_eq!(marker_shape(Marker::Circle, d).verbs().len(), 6);

    let h = 0.886 * d / 2.0;
    let (min, max) = bounds(Marker::Square);
    assert!(close(min.x, -h) && close(min.y, -h) && close(max.x, h) && close(max.y, h));

    let (min, max) = bounds(Marker::Diamond);
    let k = h * std::f32::consts::SQRT_2;
    assert!(close(min.x, -k) && close(min.y, -k) && close(max.x, k) && close(max.y, k));

    let (min, max) = bounds(Marker::Cross);
    assert!(close(min.x, -3.0) && close(min.y, -3.0) && close(max.x, 3.0) && close(max.y, 3.0));
    // Arms are d / 3 wide.
    let xs: Vec<f32> = subpaths(&marker_shape(Marker::Cross, d))[0]
        .iter()
        .map(|p| p.x)
        .collect();
    assert!(
        xs.iter()
            .all(|&x| close(x.abs(), 1.0) || close(x.abs(), 3.0))
    );

    // The triangle: equilateral, centroid at 0, with the circle's area.
    let tri = &subpaths(&marker_shape(Marker::Triangle, d))[0];
    assert_eq!(tri.len(), 3);
    let (cx, cy) = tri
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p.x / 3.0, y + p.y / 3.0));
    assert!(close(cx, 0.0) && close(cy, 0.0));
    let side = |a: Point, b: Point| (a.x - b.x).hypot(a.y - b.y);
    let s = side(tri[0], tri[1]);
    assert!(close(s, side(tri[1], tri[2])) && close(s, side(tri[2], tri[0])));
    let area = 3.0_f32.sqrt() / 4.0 * s * s;
    assert!((area - std::f32::consts::PI * 9.0).abs() < 1e-3);
    // Pointing up: the apex has the smallest y.
    assert!(tri[0].y < tri[1].y && close(tri[1].y, tri[2].y));

    // Every shape is used as the scene's marker shape.
    for m in [
        Marker::Circle,
        Marker::Square,
        Marker::Triangle,
        Marker::Diamond,
        Marker::Cross,
    ] {
        let theme = Theme::graphite();
        let scene =
            scatter_figure(theme.clone(), &[0.5], &[0.5], m).to_scene(FontLibrary::global());
        let [Item::Markers { shape, .. }] = mark_group(&scene).1 else {
            panic!("expected one marker batch")
        };
        assert_eq!(shape, &marker_shape(m, theme.marker_size()));
    }
}
