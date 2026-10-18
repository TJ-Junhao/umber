//! Seeded geometry checks independent of the placement algorithm.

use super::*;
use crate::model::pie::Slice;
use crate::{FontLibrary, PieOptions};

fn rectangle(label: &Label) -> [f64; 4] {
    [
        label.x,
        label.y - f64::from(label.layout.ascent()),
        label.x + label.width(),
        label.y + f64::from(label.layout.descent()),
    ]
}

fn overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] - 1e-4 && b[0] < a[2] - 1e-4 && a[1] < b[3] - 1e-4 && b[1] < a[3] - 1e-4
}

fn segments_touch(a: Point, b: Point, c: Point, d: Point) -> bool {
    let cross = |p: Point, q: Point, r: Point| {
        f64::from(q.x - p.x) * f64::from(r.y - p.y) - f64::from(q.y - p.y) * f64::from(r.x - p.x)
    };
    let bbox = |p: Point, q: Point| {
        [
            f64::from(p.x.min(q.x)) - 1e-4,
            f64::from(p.y.min(q.y)) - 1e-4,
            f64::from(p.x.max(q.x)) + 1e-4,
            f64::from(p.y.max(q.y)) + 1e-4,
        ]
    };
    overlap(bbox(a, b), bbox(c, d))
        && cross(a, b, c) * cross(a, b, d) <= 1e-8
        && cross(c, d, a) * cross(c, d, b) <= 1e-8
}

fn crosses_box(a: Point, b: Point, rect: [f64; 4]) -> bool {
    let [x0, y0, x1, y1] = rect;
    let points = [
        Point {
            x: x0 as f32,
            y: y0 as f32,
        },
        Point {
            x: x1 as f32,
            y: y0 as f32,
        },
        Point {
            x: x1 as f32,
            y: y1 as f32,
        },
        Point {
            x: x0 as f32,
            y: y1 as f32,
        },
    ];
    let inside = |p: Point| {
        f64::from(p.x) >= x0 && f64::from(p.x) <= x1 && f64::from(p.y) >= y0 && f64::from(p.y) <= y1
    };
    inside(a) || inside(b) || (0..4).any(|i| segments_touch(a, b, points[i], points[(i + 1) % 4]))
}

#[test]
fn random_labels_and_leaders_are_disjoint_or_fail_with_slice_names() {
    let library = FontLibrary::bundled();
    let mut state = 0x83a9_63c7_01b2_5f41_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut successes = 0;
    let mut failures = 0;
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let fonts = super::super::figure_fonts(&library, &theme).unwrap();
        for hole in [0.0, 0.5] {
            for annotate in [false, true] {
                for (width, height) in [
                    (408.0, 264.0),
                    (600.0, 160.0),
                    (180.0, 480.0),
                    (160.0, 160.0),
                    (720.0, 480.0),
                ] {
                    for sample in 0..20 {
                        let count = 2 + (next() % 29) as usize;
                        let mut values: Vec<_> = (0..count)
                            .map(|i| match (sample + i) % 7 {
                                0 => 0.0,
                                1 => 1e-9,
                                2 => 0.01,
                                _ => (next() % 1000 + 1) as f64,
                            })
                            .collect();
                        values[0] = values[0].max(1.0);
                        let total: f64 = values.iter().sum();
                        let pie = Pie {
                            slices: values
                                .iter()
                                .enumerate()
                                .map(|(i, v)| Slice {
                                    name: format!("Slice {i}"),
                                    share: v / total,
                                    color: theme.palette()[i % theme.palette().len()],
                                })
                                .collect::<Vec<_>>()
                                .into(),
                            options: PieOptions {
                                hole,
                                annotate,
                                ..Default::default()
                            },
                        };
                        let area = Area {
                            x: 12.0,
                            y: 12.0,
                            width,
                            height,
                        };
                        let result = place(&pie, LegendStyle::Direct, area, &theme, &fonts);
                        let layout = match result {
                            Ok(layout) => {
                                successes += 1;
                                layout
                            }
                            Err(error) => {
                                failures += 1;
                                assert!(matches!(error, Error::Style { .. }), "{error}");
                                assert_eq!(error.parameter(), Some("categories"));
                                assert!(error.to_string().contains("Slice "));
                                assert!(
                                    error.hint().unwrap().contains("Other")
                                        && error.hint().unwrap().contains("legend")
                                );
                                continue;
                            }
                        };
                        assert!(layout.circle.radius + 1e-4 >= width.min(height) / 4.0);
                        let rects: Vec<_> =
                            layout.labels.iter().map(|l| rectangle(&l.text)).collect();
                        for (i, label) in layout.labels.iter().enumerate() {
                            let rect = rects[i];
                            assert!(
                                rect[0] >= area.x - 1e-3
                                    && rect[1] >= area.y - 1e-3
                                    && rect[2] <= area.right() + 1e-3
                                    && rect[3] <= area.bottom() + 1e-3,
                                "label outside: {} {rect:?}",
                                label.text.text
                            );
                            for (j, other) in layout.labels.iter().enumerate().skip(i + 1) {
                                assert!(
                                    !overlap(rect, rects[j]),
                                    "{} overlaps {}",
                                    label.text.text,
                                    other.text.text
                                );
                                for a in label.leader.windows(2) {
                                    for b in other.leader.windows(2) {
                                        assert!(
                                            !segments_touch(a[0], a[1], b[0], b[1]),
                                            "crossed leaders: {} and {}",
                                            label.text.text,
                                            other.text.text
                                        );
                                    }
                                }
                            }
                            let cx = layout.circle.x;
                            let cy = layout.circle.y;
                            let near = (cx - cx.clamp(rect[0], rect[2]))
                                .hypot(cy - cy.clamp(rect[1], rect[3]));
                            if label.fill.is_none() {
                                assert!(
                                    near + 1e-3 >= layout.circle.radius,
                                    "{} overlaps circle",
                                    label.text.text
                                );
                            } else {
                                assert!(
                                    near + 1e-3 >= layout.circle.radius * hole,
                                    "{} overlaps hole",
                                    label.text.text
                                );
                                for x in [rect[0], rect[2]] {
                                    for y in [rect[1], rect[3]] {
                                        assert!(
                                            (x - cx).hypot(y - cy) <= layout.circle.radius + 1e-3
                                        );
                                    }
                                }
                            }
                            for segment in label.leader.windows(2) {
                                for (j, rect) in rects.iter().enumerate() {
                                    if i != j {
                                        assert!(
                                            !crosses_box(segment[0], segment[1], *rect),
                                            "leader {} crosses {}",
                                            label.text.text,
                                            layout.labels[j].text.text
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        successes > 30 && failures > 30,
        "{successes} successful, {failures} rejected"
    );
}
