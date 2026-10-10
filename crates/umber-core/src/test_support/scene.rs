//! Scene checks shared by the unit tests of `draw` and `layout`.

use crate::geometry::{Path, PathVerb, Point};
use crate::scene::{Item, Scene};

/// The renderers' coordinate limit, restated so the tests do not depend on `normalize`.
const MAX_COORD: f32 = 1.0e6;

fn check(p: Point) {
    assert!(
        p.x.is_finite() && p.y.is_finite() && p.x.abs() <= MAX_COORD && p.y.abs() <= MAX_COORD,
        "{p:?}"
    );
}

fn check_path(path: &Path) {
    for verb in path.verbs() {
        match *verb {
            PathVerb::MoveTo(p) | PathVerb::LineTo(p) => check(p),
            PathVerb::QuadTo(c, p) => {
                check(c);
                check(p);
            }
            PathVerb::CubicTo(c1, c2, p) => {
                check(c1);
                check(c2);
                check(p);
            }
            PathVerb::Close => {}
        }
    }
}

fn check_items(items: &[Item]) {
    for item in items {
        match item {
            Item::Fill { path, .. } => check_path(path),
            Item::Stroke { path, style } => {
                check_path(path);
                assert!(style.width.is_finite());
            }
            Item::Markers {
                shape, positions, ..
            } => {
                check_path(shape);
                positions.iter().copied().for_each(check);
            }
            Item::Text(t) => {
                check(t.origin);
                check_path(&t.outline);
                assert!(t.width.is_finite());
            }
            Item::Group { clip, items } => {
                if let Some(r) = clip {
                    check(Point { x: r.x, y: r.y });
                    check(Point {
                        x: r.x + r.width,
                        y: r.y + r.height,
                    });
                }
                check_items(items);
            }
        }
    }
}

/// Asserts every coordinate of `scene` is finite and within `MAX_COORD`.
pub(crate) fn assert_scene_bounded(scene: &Scene) {
    assert!(scene.width.is_finite() && scene.height.is_finite());
    check_items(&scene.items);
}
