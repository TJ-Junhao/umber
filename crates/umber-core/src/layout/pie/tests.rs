//! Measured slice labels and bounded circular geometry.

use super::*;
use crate::FontLibrary;
use crate::model::pie::Slice;

fn pie(names: &[&str], shares: &[f64], hole: f64, annotate: bool) -> Pie {
    Pie {
        slices: names
            .iter()
            .zip(shares)
            .map(|(name, &share)| Slice {
                name: (*name).into(),
                share,
                color: Theme::pigment().palette()[0],
            })
            .collect::<Vec<_>>()
            .into(),
        options: crate::PieOptions {
            hole,
            annotate,
            ..Default::default()
        },
    }
}

fn area(width: f64, height: f64) -> Area {
    Area {
        x: 12.0,
        y: 12.0,
        width,
        height,
    }
}

fn layout(pie: &Pie, area: Area) -> Result<PieLayout> {
    let theme = Theme::pigment();
    let library = FontLibrary::bundled();
    let fonts = super::super::figure_fonts(&library, &theme).unwrap();
    place(
        pie,
        LegendStyle::Direct,
        PieRoom::new(area, area),
        &theme,
        &fonts,
    )
}

#[test]
fn leaders_keep_each_radial_knees_x() {
    let result = layout(
        &pie(
            &["Web", "App", "Shop", "Other"],
            &[0.8, 0.15, 0.03, 0.02],
            0.0,
            true,
        ),
        area(408.0, 264.0),
    )
    .unwrap();
    for label in result
        .labels
        .iter()
        .filter(|label| !label.leader.is_empty())
    {
        let points = &label.leader;
        assert!(((points[1].x - points[0].x).hypot(points[1].y - points[0].y) - 8.0).abs() < 0.001);
        let rim_distance = (f64::from(points[0].x) - result.circle.x)
            .hypot(f64::from(points[0].y) - result.circle.y);
        assert!((rim_distance - result.circle.radius - 2.0).abs() < 0.001);
        assert!(
            points.len() <= 4,
            "unexpected shared-column route: {points:?}"
        );
        let elbow = points.last().unwrap();
        assert!(((elbow.x - points[1].x).abs() - 6.0).abs() < 0.001);
        if points.len() == 4 {
            assert_eq!(points[1].x, points[2].x);
        }
    }
}

#[test]
fn spreading_minimizes_squared_movement() {
    let result = layout(
        &pie(
            &["Web", "App", "Shop", "Other"],
            &[0.8, 0.15, 0.03, 0.02],
            0.0,
            true,
        ),
        area(408.0, 264.0),
    )
    .unwrap();
    let mut outside: Vec<_> = result
        .labels
        .iter()
        .filter(|label| !label.leader.is_empty())
        .collect();
    outside.sort_by(|a, b| a.leader[1].y.total_cmp(&b.leader[1].y));
    assert_eq!(outside.len(), 2);
    let desired: Vec<_> = outside.iter().map(|l| f64::from(l.leader[1].y)).collect();
    let actual: Vec<_> = outside
        .iter()
        .map(|l| f64::from(l.leader.last().unwrap().y))
        .collect();
    let theme = Theme::pigment();
    let line = BODY_LINE
        * f64::from(
            Styles::new(&theme, TextRole::Title, StyleSizes::default())
                .body
                .size,
        );
    assert!(
        (actual[1] - desired[1]).abs() < 0.001,
        "the lower label stays at its knee"
    );
    assert!(
        (actual[1] - actual[0] - line).abs() < 0.001,
        "the upper label moves only far enough to leave one line of separation"
    );
    assert!(
        actual
            .iter()
            .zip(&desired)
            .all(|(y, knee)| y <= &(knee + 0.001))
    );
}

#[test]
fn outside_labels_cannot_reduce_radius_below_half() {
    let result = layout(
        &pie(&["A", "A long outside label"], &[0.99, 0.01], 0.0, false),
        area(160.0, 160.0),
    );
    let error = match result {
        Ok(layout) => panic!("radius {} should have failed", layout.circle.radius),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Style { .. }));
    assert!(error.to_string().contains("A long outside label"));
    assert!(!error.to_string().contains("\"A\""));
}

#[test]
fn small_slices_above_and_below_the_circle_fit() {
    let cases: &[(&[&str], &[f64], crate::SizeSpec)] = &[
        (
            &["Web", "App", "Shop", "API", "Ads", "Labs", "Tools", "Misc"],
            &[50.0, 20.0, 12.0, 8.0, 4.0, 3.0, 2.0, 1.0],
            crate::SizeSpec::Auto,
        ),
        (
            &["Everything", "a", "b"],
            &[1e6, 1.0, 1.0],
            crate::SizeSpec::Auto,
        ),
        (
            &["A", "B", "C", "D"],
            &[93.3, 2.3, 2.2, 2.2],
            crate::SizeSpec::Auto,
        ),
        (
            &["Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5"],
            &[0.5, 0.5, 0.5, 0.5, 0.5, 97.5],
            crate::SizeSpec::Auto,
        ),
        (
            &["Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5"],
            &[48.0, 0.5, 0.5, 0.5, 0.5, 50.0],
            crate::SizeSpec::Auto,
        ),
        (
            &["Web", "App", "Shop", "Other"],
            &[80.0, 15.0, 3.0, 2.0],
            crate::SizeSpec::pt(11.0).unwrap(),
        ),
    ];
    let library = FontLibrary::bundled();
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let fonts = super::super::figure_fonts(&library, &theme).unwrap();
        for hole in [0.0, 0.5] {
            for &(names, values, size) in cases {
                let total: f64 = values.iter().sum();
                let shares: Vec<_> = values.iter().map(|v| v / total).collect();
                let mut pie = pie(names, &shares, hole, true);
                pie.options.size = size;
                let area = area(408.0, 264.0);
                let result = place(
                    &pie,
                    LegendStyle::Direct,
                    PieRoom::new(area, area),
                    &theme,
                    &fonts,
                )
                .unwrap_or_else(|error| panic!("{names:?} hole={hole}: {error}"));
                super::property_tests::assert_disjoint(&result, PieRoom::new(area, area));
            }
        }
    }
}

#[test]
fn collision_rejects_leaders_through_the_disc_and_ring() {
    for hole in [0.0, 0.5] {
        let mut result = layout(
            &pie(&["A", "B"], &[0.99, 0.01], hole, false),
            area(408.0, 264.0),
        )
        .unwrap();
        result.labels.retain(|label| label.fill.is_none());
        assert_eq!(result.labels.len(), 1);
        let circle = result.circle;
        let y = circle.y - circle.radius - 0.25;
        for leader in [
            vec![
                circle.point(0.0, circle.radius + 2.0),
                circle.point(PI, circle.radius + 2.0),
            ],
            vec![
                Point {
                    x: (circle.x - 10.0) as f32,
                    y: y as f32,
                },
                Point {
                    x: (circle.x + 10.0) as f32,
                    y: y as f32,
                },
            ],
        ] {
            result.labels[0].leader = leader;
            assert!(
                !collision::fits(&result.labels, circle, area(408.0, 264.0)),
                "leader must clear the disc/ring, including stroke width"
            );
        }
    }
}

#[test]
fn neighbouring_leaders_may_be_close_but_must_not_cross_or_coincide() {
    let area = area(408.0, 264.0);
    let mut result = layout(
        &pie(
            &["Web", "App", "Shop", "Other"],
            &[0.8, 0.15, 0.03, 0.02],
            0.0,
            true,
        ),
        area,
    )
    .unwrap();
    result.labels.retain(|label| label.fill.is_none());
    assert_eq!(result.labels.len(), 2);
    result.labels[0].leader = vec![Point { x: 20.0, y: 100.0 }, Point { x: 20.0, y: 200.0 }];
    result.labels[1].leader = vec![Point { x: 20.01, y: 100.0 }, Point { x: 20.01, y: 200.0 }];
    assert!(collision::fits(&result.labels, result.circle, area));
    for leader in [
        vec![Point { x: 15.0, y: 150.0 }, Point { x: 25.0, y: 150.0 }],
        vec![Point { x: 20.0, y: 150.0 }, Point { x: 20.0, y: 210.0 }],
        vec![Point { x: 20.01, y: 100.0 }, Point { x: 19.9, y: 200.0 }],
        vec![Point { x: 20.0, y: 100.01 }, Point { x: 20.0, y: 210.0 }],
        vec![
            Point { x: 22.0, y: 100.0 },
            Point { x: 20.1, y: 150.0 },
            Point { x: 20.1, y: 200.0 },
        ],
    ] {
        result.labels[1].leader = leader;
        assert!(!collision::fits(&result.labels, result.circle, area));
    }
}

#[test]
fn single_slice_is_centered() {
    let result = layout(&pie(&["Only"], &[1.0], 0.0, false), area(300.0, 240.0)).unwrap();
    let label = &result.labels[0].text;
    assert!((label.x + label.width() / 2.0 - result.circle.x).abs() < 0.001);
    let center_y = label.y + f64::from(label.layout.descent() - label.layout.ascent()) / 2.0;
    assert!((center_y - result.circle.y).abs() < 0.001);
}

#[test]
fn outside_side_label_needs_no_vertical_reservation() {
    let result = layout(
        &pie(&["Side label", "Rest"], &[0.5, 0.5], 0.99, false),
        area(600.0, 200.0),
    )
    .unwrap();
    assert!((result.circle.radius - 100.0).abs() < 0.001);
}

#[test]
fn inside_fit_uses_ink_height_not_line_height() {
    let theme = Theme::pigment();
    let library = FontLibrary::bundled();
    let fonts = super::super::figure_fonts(&library, &theme).unwrap();
    let style = Styles::new(&theme, TextRole::Title, StyleSizes::default()).body;
    let label = measure(&fonts, "I", style).unwrap();
    let ink = f64::from(label.layout.ascent() + label.layout.descent());
    let line = BODY_LINE * f64::from(style.size);
    assert!(ink < line);
    let near_x = 65.0 - label.width() / 2.0 - FIT_GAP;
    let ink_angle = ((ink / 2.0 + FIT_GAP) / near_x).atan();
    let line_angle = ((line / 2.0 + FIT_GAP) / near_x).atan();
    let half_angle = (ink_angle + line_angle) / 2.0;
    let slice = MeasuredSlice {
        name: "I".into(),
        start: PI / 2.0 - half_angle,
        end: PI / 2.0 + half_angle,
        color: theme.palette()[0],
        inside: vec![label],
        outside: None,
    };
    assert!(slice.fits(
        Circle {
            x: 0.0,
            y: 0.0,
            radius: 100.0
        },
        0.0,
        line
    ));
    assert!(line_angle > half_angle, "the old full-line box cannot fit");
}

#[test]
fn opposite_radial_leaders_on_one_line_do_not_cross() {
    // The shares of a 30/70 pie sit outside at opposite angles, so their radial leaders lie on
    // one line through the centre; moving the panel must not make them count as crossing.
    let theme = Theme::pigment();
    let library = FontLibrary::bundled();
    let fonts = super::super::figure_fonts(&library, &theme).unwrap();
    let mut pie = pie(&["A rather long slice name", "B"], &[0.7, 0.3], 0.0, true);
    pie.options.size = crate::SizeSpec::pt(14.0).unwrap();
    for width in (250..=400).step_by(5).map(f64::from) {
        let area = area(width, 60.0);
        let layout = place(
            &pie,
            LegendStyle::Chips,
            PieRoom::new(area, area),
            &theme,
            &fonts,
        )
        .unwrap_or_else(|error| panic!("{width}: {error}"));
        assert_eq!(layout.labels.iter().filter(|l| l.fill.is_none()).count(), 2);
    }
}
