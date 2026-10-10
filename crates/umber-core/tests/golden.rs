//! Golden-image tests for the PNG backend.
//!
//! References live in `tests/golden/{name}.png`. Run with `UPDATE_GOLDEN=1` to write or
//! overwrite them. A failing comparison writes `tests/golden/{name}.diff.png`, a red overlay
//! on the differing pixels, next to the reference.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here.
#![allow(clippy::unwrap_used, clippy::panic)]

#[path = "support/golden_compare.rs"]
mod golden_compare;

use golden_compare::{assert_golden, compare_golden, golden_dir};
use umber_core::render::render_png;
use umber_core::scene::{
    FillRule, GradientStop, Item, LineCap, LineJoin, LinearGradient, Paint, Rect, Scene,
    StrokeStyle,
};
use umber_core::{Font, FontLibrary, Path, Point, Rgba, TextStyle, Weight};

const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba { r, g, b, a: 255 }
}

fn polygon(points: &[(f32, f32)], dx: f32) -> Path {
    let mut path = Path::new();
    for (i, (x, y)) in points.iter().enumerate() {
        let p = Point { x: x + dx, y: *y };
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    path.close();
    path
}

/// A circle of radius `r` centered at (0, 0), as four cubic curves.
fn circle(r: f32) -> Path {
    const K: f32 = 0.552_284_8;
    let k = r * K;
    let p = |x: f32, y: f32| Point { x, y };
    let mut path = Path::new();
    path.move_to(p(r, 0.0));
    path.cubic_to(p(r, k), p(k, r), p(0.0, r));
    path.cubic_to(p(-k, r), p(-r, k), p(-r, 0.0));
    path.cubic_to(p(-r, -k), p(-k, -r), p(0.0, -r));
    path.cubic_to(p(k, -r), p(r, -k), p(r, 0.0));
    path.close();
    path
}

/// The scene behind `golden_primitives`: 4 x 3 in. Everything is shifted right by `dx` points.
fn primitives_scene(dx: f32) -> Scene {
    let mut scene = Scene::new(288.0, 216.0, rgb(255, 255, 255));

    scene.push(Item::Fill {
        path: polygon(&[(20.0, 90.0), (80.0, 90.0), (50.0, 30.0)], dx),
        paint: Paint::Solid(rgb(31, 78, 121)),
        rule: FillRule::NonZero,
    });

    scene.push(Item::Fill {
        path: polygon(
            &[(100.0, 30.0), (200.0, 30.0), (200.0, 90.0), (100.0, 90.0)],
            dx,
        ),
        paint: Paint::Linear(LinearGradient {
            start: Point {
                x: 100.0 + dx,
                y: 30.0,
            },
            end: Point {
                x: 200.0 + dx,
                y: 90.0,
            },
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: rgb(230, 120, 30),
                },
                GradientStop {
                    offset: 0.5,
                    color: rgb(240, 220, 90),
                },
                GradientStop {
                    offset: 1.0,
                    color: rgb(30, 130, 120),
                },
            ],
        }),
        rule: FillRule::NonZero,
    });

    let mut wave = Path::new();
    wave.move_to(Point {
        x: 215.0 + dx,
        y: 85.0,
    });
    wave.cubic_to(
        Point {
            x: 235.0 + dx,
            y: 20.0,
        },
        Point {
            x: 255.0 + dx,
            y: 100.0,
        },
        Point {
            x: 275.0 + dx,
            y: 35.0,
        },
    );
    scene.push(Item::Stroke {
        path: wave,
        style: StrokeStyle {
            color: rgb(150, 30, 60),
            width: 3.0,
            dash: vec![6.0, 5.0],
            cap: LineCap::Round,
            join: LineJoin::Round,
        },
    });

    // 50 markers on a deterministic scatter, from a small integer generator.
    let mut state: u32 = 12345;
    let mut next = move || {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
        ((state >> 16) & 0x7fff) as f32 / 32767.0
    };
    let positions = (0..50)
        .map(|_| Point {
            x: 20.0 + dx + next() * 130.0,
            y: 110.0 + next() * 50.0,
        })
        .collect();
    scene.push(Item::Markers {
        shape: circle(4.0),
        fill: Some(rgb(40, 120, 190)),
        stroke: Some(StrokeStyle {
            color: rgb(255, 255, 255),
            width: 0.8,
            dash: vec![],
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }),
        positions,
    });

    scene.push(Item::Group {
        clip: Some(Rect {
            x: 170.0 + dx,
            y: 115.0,
            width: 60.0,
            height: 40.0,
        }),
        items: vec![Item::Fill {
            path: circle(25.0).translated(205.0 + dx, 135.0),
            paint: Paint::Solid(rgb(110, 60, 160)),
            rule: FillRule::NonZero,
        }],
    });

    let library = FontLibrary::global();
    let black = rgb(20, 20, 20);
    scene.push_text(
        library,
        "Revenue \u{2212}12.5%",
        TextStyle {
            font: Font::Inter,
            weight: Weight::SemiBold,
            size: 13.0,
            tabular_figures: false,
        },
        Point {
            x: 20.0 + dx,
            y: 185.0,
        },
        black,
    );
    scene.push_text(
        library,
        "Q1 Q2 Q3",
        TextStyle {
            font: Font::SourceSerif4,
            weight: Weight::Regular,
            size: 10.0,
            tabular_figures: false,
        },
        Point {
            x: 20.0 + dx,
            y: 203.0,
        },
        black,
    );
    scene
}

#[test]
fn golden_primitives() {
    let bytes = render_png(&primitives_scene(0.0), 100).unwrap();
    assert_golden("golden_primitives", &bytes);
}

#[test]
fn golden_comparison_detects_a_one_pixel_shift() {
    // 0.72 pt is one pixel at 100 dpi.
    let shifted = render_png(&primitives_scene(0.72), 100).unwrap();
    let diff_dir = std::env::temp_dir().join(format!("umber-golden-shift-{}", std::process::id()));
    std::fs::create_dir_all(&diff_dir).unwrap();
    let result = compare_golden("golden_primitives", &shifted, &golden_dir(), &diff_dir);
    let _ = std::fs::remove_dir_all(&diff_dir);
    assert!(result.is_err(), "a one pixel shift went unnoticed");
}
