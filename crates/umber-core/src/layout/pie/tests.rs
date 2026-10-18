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
    place(pie, LegendStyle::Direct, area, &theme, &fonts)
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
    let desired_mean = outside
        .iter()
        .map(|l| f64::from(l.leader[1].y))
        .sum::<f64>()
        / 2.0;
    let actual_mean = outside
        .iter()
        .map(|l| f64::from(l.leader.last().unwrap().y))
        .sum::<f64>()
        / 2.0;
    assert!(
        (desired_mean - actual_mean).abs() < 0.001,
        "unbounded cluster should spread symmetrically"
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
