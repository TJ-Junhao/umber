//! Scatter marks: culling, fill alpha and the background-colored outline.

use super::super::MarkContext;
use super::super::mapping::{AxisMap, AxisMaps};
use super::super::marker::{extent, marker_shape};
use super::super::scatter::scatter_item;
use super::{Pixels, mark_group, near, nums, plot_of, scatter_figure};
use crate::layout::Area;
use crate::model::{Figure, ScatterOptions};
use crate::resolve::{Baseline, NormalizedPos, ValueAxis};
use crate::scene::Item;
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, Marker, Rgba, ScaleKind, SizeSpec, Theme};

#[test]
fn scatter_drops_points_far_outside() {
    let theme = Theme::graphite();
    let probe = scatter_figure(theme.clone(), &[0.5], &[0.5], Marker::Circle);
    let plot = plot_of(&probe);
    let r = f64::from(theme.marker_size()) / 2.0;
    // In data units: just inside and just outside the radius + 0.8 pt outline + 1 pt slack
    // on the right.
    let inside = 1.0 + (r + 1.7) / plot.width;
    let outside = 1.0 + (r + 1.9) / plot.width;
    let f = scatter_figure(
        theme.clone(),
        &[0.5, 1e9, -1e9, f64::NAN, 0.25, inside, outside, 0.5],
        &[0.5, 0.5, 0.5, 0.5, f64::NAN, 0.5, 0.5, -1e12],
        Marker::Circle,
    );
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let (_, items) = mark_group(&scene);
    let [
        Item::Markers {
            positions,
            fill: Some(fill),
            stroke: Some(stroke),
            ..
        },
    ] = items
    else {
        panic!("expected one marker batch, got {items:?}")
    };
    assert_eq!(positions.len(), 2, "{positions:?}");
    assert!((f64::from(positions[0].x) - plot.x_at(NormalizedPos::new(0.5))).abs() < 1e-3);
    assert!((f64::from(positions[1].x) - plot.x_at(NormalizedPos::new(inside))).abs() < 1e-3);
    // Fill at the theme's marker alpha, outline 0.8 pt in the background color.
    let color = theme.palette()[0];
    assert_eq!(fill.a, (0.9_f32 * 255.0).round() as u8);
    assert_eq!((fill.r, fill.g, fill.b), (color.r, color.g, color.b));
    assert_eq!((stroke.color, stroke.width), (theme.background(), 0.8));
    assert!(stroke.dash.is_empty());

    // Every point outside: no marks group at all.
    let f = scatter_figure(theme, &[5.0, -5.0], &[0.5, 0.5], Marker::Circle);
    assert_eq!(f.to_scene(FontLibrary::global()).unwrap().items.len(), 2);
}

#[test]
fn scatter_alpha_multiplies_the_color_alpha() {
    let theme = Theme::pigment();
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(
            nums(&[0.5]),
            &[0.5],
            ScatterOptions {
                color: crate::ColorSpec::AUTO.with_alpha(crate::Alpha::new(0.5).unwrap()),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let [
        Item::Markers {
            fill: Some(fill), ..
        },
    ] = mark_group(&scene).1
    else {
        panic!("expected one marker batch")
    };
    // 128 (alpha 0.5) * 0.85.
    assert_eq!(fill.a, (128.0_f32 * 0.85).round() as u8);
}

#[test]
fn scatter_outline_uses_background() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let theme = theme
            .with_marker_size(40.0)
            .unwrap()
            .with_marker_alpha(1.0)
            .unwrap();
        // Two overlapping markers: the second's outline crosses the first's fill.
        let probe = scatter_figure(theme.clone(), &[0.5], &[0.5], Marker::Circle);
        let plot = plot_of(&probe);
        let dx = 16.0 / plot.width;
        let f = scatter_figure(theme.clone(), &[0.5, 0.5 + dx], &[0.5, 0.5], Marker::Circle);
        let dpi = 300.0;
        let pixels = Pixels::decode(&f.to_png(dpi as i64).unwrap());
        let (cx, cy) = (
            plot.x_at(NormalizedPos::new(0.5 + dx)),
            plot.y_at(NormalizedPos::new(0.5)),
        );
        let color = theme.palette()[0];
        // The second marker's left edge, 20 pt left of its center, inside the first marker.
        let edge = pixels.at_point((cx - 20.0, cy), dpi);
        assert!(near(edge, theme.background(), 24), "{edge:?}");
        // Fill on either side of the outline.
        assert!(near(pixels.at_point((cx - 23.0, cy), dpi), color, 2));
        assert!(near(pixels.at_point((cx - 17.0, cy), dpi), color, 2));
    }
}

/// Axis maps that leave plot coordinates unchanged, for testing in points.
fn identity_maps(plot: Area) -> AxisMaps {
    AxisMaps {
        x: AxisMap {
            lo: plot.x,
            hi: plot.right(),
            start: plot.x,
            extent: plot.width,
        },
        y: AxisMap {
            lo: plot.y,
            hi: plot.bottom(),
            start: plot.y,
            extent: plot.height,
        },
        frame: crate::resolve::Orientation::Vertical,
    }
}

/// The mark context of `theme` with [`identity_maps`] onto `plot`.
fn identity_context(theme: &Theme, plot: Area) -> MarkContext<'_> {
    MarkContext {
        theme,
        maps: identity_maps(plot),
        plot,
        baseline: Baseline {
            axis: ValueAxis::Y,
            pos: NormalizedPos::LOW,
        },
        baseline_at: plot.bottom(),
        widest_line: 0.0,
    }
}

#[test]
fn markers_are_culled_by_their_true_extent() {
    // The visible top of a triangle, and a diamond, centered just below the plot area.
    let plot = Area {
        x: 10.0,
        y: 20.0,
        width: 400.0,
        height: 240.0,
    };
    for (shape, d, offset) in [(Marker::Triangle, 12.0, 7.5), (Marker::Diamond, 40.0, 23.0)] {
        let theme = Theme::graphite().with_marker_size(d).unwrap();
        let y = plot.bottom() + offset;
        let (min, _) = marker_shape(shape, d as f32).bounds().unwrap();
        assert!(
            y + f64::from(min.y) < plot.bottom(),
            "{shape:?} reaches the plot"
        );
        let item = scatter_item(
            &identity_context(&theme, plot),
            &[210.0],
            &[y],
            Rgba::opaque(255, 0, 0),
            shape,
            d as f32,
        );
        assert!(item.is_some(), "{shape:?} was dropped");
    }

    // Per shape and axis: kept just inside the reach, dropped just outside it.
    let theme = Theme::graphite().with_marker_size(10.0).unwrap();
    for shape in [
        Marker::Circle,
        Marker::Square,
        Marker::Triangle,
        Marker::Diamond,
        Marker::Cross,
    ] {
        let (w, h) = extent(&marker_shape(shape, 10.0));
        let reach = 0.8 + 1.0;
        let cases = [
            (plot.x - w - reach + 0.01, 100.0, true),
            (plot.x - w - reach - 0.01, 100.0, false),
            (plot.right() + w + reach - 0.01, 100.0, true),
            (plot.right() + w + reach + 0.01, 100.0, false),
            (200.0, plot.y - h - reach + 0.01, true),
            (200.0, plot.y - h - reach - 0.01, false),
            (200.0, plot.bottom() + h + reach - 0.01, true),
            (200.0, plot.bottom() + h + reach + 0.01, false),
        ];
        for (x, y, kept) in cases {
            let item = scatter_item(
                &identity_context(&theme, plot),
                &[x],
                &[y],
                Rgba::BLACK,
                shape,
                10.0,
            );
            assert_eq!(item.is_some(), kept, "{shape:?} at ({x}, {y})");
        }
    }
}

#[test]
fn per_series_size_sets_the_marker_shape() {
    // Graphite markers are 5.6 pt across by default.
    for (spec, d) in [
        (SizeSpec::Auto, 5.6_f32),
        (SizeSpec::times(2.0).unwrap(), 2.0 * 5.6),
        (SizeSpec::pt(12.0).unwrap(), 12.0),
    ] {
        for m in [Marker::Circle, Marker::Cross] {
            let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .scatter(
                    nums(&[0.5]),
                    &[0.5],
                    ScatterOptions {
                        marker: m,
                        size: spec,
                        ..ScatterOptions::default()
                    },
                )
                .unwrap();
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let [Item::Markers { shape, .. }] = mark_group(&scene).1 else {
                panic!("expected one marker batch")
            };
            assert_eq!(shape, &marker_shape(m, d), "{spec:?} {m:?}");
        }
    }
}
