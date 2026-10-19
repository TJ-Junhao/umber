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

pub(super) fn assert_disjoint(layout: &PieLayout, room: PieRoom) {
    let area = room.area;
    assert!(layout.circle.radius + 1e-4 >= room.least_radius);
    let rects: Vec<_> = layout.labels.iter().map(|l| rectangle(&l.text)).collect();
    let cx = layout.circle.x;
    let cy = layout.circle.y;
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
        let near = (cx - cx.clamp(rect[0], rect[2])).hypot(cy - cy.clamp(rect[1], rect[3]));
        if label.fill.is_none() {
            assert!(
                near + 1e-3 >= layout.circle.radius,
                "{} overlaps circle",
                label.text.text
            );
        } else {
            assert!(
                near + 1e-3 >= layout.circle.radius * layout.hole,
                "{} overlaps hole",
                label.text.text
            );
            for x in [rect[0], rect[2]] {
                for y in [rect[1], rect[3]] {
                    assert!((x - cx).hypot(y - cy) <= layout.circle.radius + 1e-3);
                }
            }
        }
        for segment in label.leader.windows(2) {
            // Minimize squared distance along the entire stroked segment, not just its ends.
            // Outside leaders must clear the outer disc even when the pie has a hole.
            let ax = f64::from(segment[0].x) - cx;
            let ay = f64::from(segment[0].y) - cy;
            let dx = f64::from(segment[1].x) - f64::from(segment[0].x);
            let dy = f64::from(segment[1].y) - f64::from(segment[0].y);
            let length = dx * dx + dy * dy;
            let t = if length == 0.0 {
                0.0
            } else {
                (-(ax * dx + ay * dy) / length).clamp(0.0, 1.0)
            };
            assert!(
                (ax + t * dx).hypot(ay + t * dy) + 1e-3 >= layout.circle.radius + 0.375,
                "leader {} enters disc/ring",
                label.text.text
            );
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

#[test]
fn pareto_shares_in_roomy_panels_always_fit() {
    let library = FontLibrary::bundled();
    let mut state = 0x18b7_c932_45a6_0f21_u64;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 11) as f64 + 0.5) / (1_u64 << 53) as f64
    };
    let area = Area {
        x: 12.0,
        y: 12.0,
        width: 408.0,
        height: 264.0,
    };
    let mut rendered = 0;
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let fonts = super::super::figure_fonts(&library, &theme).unwrap();
        for hole in [0.0, 0.5] {
            for annotate in [false, true] {
                for count in 3..=8 {
                    for sample in 0..10 {
                        let values: Vec<f64> = (0..count)
                            .map(|_| (1.0 - next()).powf(-1.0 / 1.2))
                            .collect();
                        for order in 0..3 {
                            let mut shares = values.clone();
                            if order == 1 {
                                shares.sort_by(f64::total_cmp);
                            }
                            if order == 2 {
                                shares.sort_by(|a, b| b.total_cmp(a));
                            }
                            let total: f64 = shares.iter().sum();
                            let pie = Pie {
                                slices: shares
                                    .iter()
                                    .enumerate()
                                    .map(|(i, v)| Slice {
                                        name: [
                                            "Web", "App", "Shop", "API", "Ads", "Labs", "Tools",
                                            "Misc",
                                        ][i]
                                            .into(),
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
                            let layout = place(&pie, LegendStyle::Direct, PieRoom::new(area, area), &theme, &fonts).unwrap_or_else(|error| panic!("sample={sample} order={order} hole={hole} annotate={annotate} shares={shares:?}: {error}"));
                            assert_disjoint(&layout, PieRoom::new(area, area));
                            rendered += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(rendered, 2160);
    eprintln!("Pareto: 0/{rendered} failed");
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
                        let result = place(
                            &pie,
                            LegendStyle::Direct,
                            PieRoom::new(area, area),
                            &theme,
                            &fonts,
                        );
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
                        assert_disjoint(&layout, PieRoom::new(area, area));
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

#[test]
fn more_room_never_turns_a_fitting_pie_into_an_error() {
    let library = FontLibrary::bundled();
    let theme = Theme::pigment();
    let fonts = super::super::figure_fonts(&library, &theme).unwrap();
    let sets: [(&[&str], &[f64]); 6] = [
        (&["Web", "App", "Shop"], &[52.0, 30.0, 18.0]),
        (&["Web", "App", "Shop", "Other"], &[80.0, 15.0, 3.0, 2.0]),
        (&["Large", "Small"], &[99.0, 1.0]),
        (
            &["North", "South", "East", "West", "Other"],
            &[60.0, 20.0, 10.0, 5.0, 5.0],
        ),
        (
            &["Web", "App", "Shop", "API", "Ads", "Labs", "Tools", "Misc"],
            &[50.0, 20.0, 12.0, 8.0, 4.0, 3.0, 2.0, 1.0],
        ),
        (&["A rather long slice name", "B"], &[70.0, 30.0]),
    ];
    let (mut fitting, mut enlarged) = (0, 0);
    for (names, values) in sets {
        let total: f64 = values.iter().sum();
        for hole in [0.0, 0.55, 0.8] {
            for annotate in [false, true] {
                for (size, style) in [8.0, 10.0, 14.0].into_iter().flat_map(|size| {
                    [LegendStyle::Direct, LegendStyle::Chips].map(|style| (size, style))
                }) {
                    let pie = Pie {
                        slices: names
                            .iter()
                            .zip(values)
                            .enumerate()
                            .map(|(i, (name, value))| Slice {
                                name: (*name).into(),
                                share: value / total,
                                color: theme.palette()[i % theme.palette().len()],
                            })
                            .collect::<Vec<_>>()
                            .into(),
                        options: PieOptions {
                            hole,
                            annotate,
                            size: crate::SizeSpec::pt(size).unwrap(),
                            ..Default::default()
                        },
                    };
                    for width in [40.0, 70.0, 100.0, 140.0, 190.0, 260.0] {
                        for height in [30.0, 60.0, 90.0, 130.0, 200.0] {
                            let area = |w: f64, h: f64| Area {
                                x: 12.0,
                                y: 12.0,
                                width: w,
                                height: h,
                            };
                            let base = PieRoom::new(area(width, height), area(width, height));
                            if place(&pie, style, base, &theme, &fonts).is_err() {
                                continue;
                            }
                            fitting += 1;
                            for grow in [0.5, 3.0, 17.0, 60.0, 150.0, 250.0, 400.0] {
                                for (dw, dh) in [(grow, 0.0), (0.0, grow), (grow, grow)] {
                                    let room = PieRoom {
                                        area: area(width + dw, height + dh),
                                        ..base
                                    };
                                    let layout = place(&pie, style, room, &theme, &fonts)
                                        .unwrap_or_else(|error| {
                                            panic!(
                                                "{names:?} hole={hole} annotate={annotate} size={size} {style:?}: \
                                                 fits {width} x {height} but not {} x {}: {error}",
                                                width + dw,
                                                height + dh
                                            )
                                        });
                                    assert_disjoint(&layout, room);
                                    enlarged += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(fitting > 300, "{fitting} fitting pies");
    eprintln!("{fitting} fitting pies, {enlarged} enlarged areas fit");
}
