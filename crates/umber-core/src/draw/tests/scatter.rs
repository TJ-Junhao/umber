//! Scatter marks: culling, fill alpha and the background-colored outline.

use super::super::MarkContext;
use super::super::mapping::{AxisMap, AxisMaps};
use super::super::marker::{extent, marker_shape};
use super::super::scatter::scatter_layers;
use super::{Pixels, mark_group, merged_layers, near, nums, plot_of, scatter_figure};
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
    // The fill has the theme's marker size less the 0.8 pt outline.
    let r = f64::from(theme.drawn_fill_size(SizeSpec::Auto)) / 2.0;
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
    let items = merged_layers(mark_group(&scene).1);
    let [
        Item::Markers {
            positions,
            fill: Some(fill),
            stroke: Some(stroke),
            ..
        },
    ] = items.as_slice()
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

    // Every point outside: only chrome, with neither marks nor a baseline.
    let f = scatter_figure(theme, &[5.0, -5.0], &[0.5, 0.5], Marker::Circle);
    assert_eq!(f.to_scene(FontLibrary::global()).unwrap().items.len(), 1);
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
                color: crate::PointColor::Uniform(
                    crate::ColorSpec::AUTO.with_alpha(crate::Alpha::new(0.5).unwrap()),
                ),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let layers = merged_layers(mark_group(&scene).1);
    let [
        Item::Markers {
            fill: Some(fill), ..
        },
    ] = layers.as_slice()
    else {
        panic!("expected one marker batch")
    };
    // 128 (alpha 0.5) * 0.85.
    assert_eq!(fill.a, (128.0_f32 * 0.85).round() as u8);
}

#[test]
fn scatter_outlines_lie_under_every_fill() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let theme = theme
            .with_marker_size(40.0)
            .unwrap()
            .with_marker_alpha(1.0)
            .unwrap();
        // Two overlapping markers: the second's outline stays under the first's fill.
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
        // The second marker's fill is 39.2 pt across, so its left edge lies 19.6 pt left of its
        // center, inside the first marker: the whole edge is the series color.
        for offset in [23.0, 20.0, 19.8, 19.6, 19.4, 17.0] {
            let pixel = pixels.at_point((cx - offset, cy), dpi);
            assert!(near(pixel, color, 2), "{offset}: {pixel:?}");
        }
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
        let layers = scatter_layers(
            &identity_context(&theme, plot),
            &[210.0],
            &[y],
            Rgba::opaque(255, 0, 0),
            shape,
            d as f32,
        );
        assert!(!layers.is_empty(), "{shape:?} was dropped");
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
            let layers = scatter_layers(
                &identity_context(&theme, plot),
                &[x],
                &[y],
                Rgba::BLACK,
                shape,
                10.0,
            );
            assert_eq!(!layers.is_empty(), kept, "{shape:?} at ({x}, {y})");
        }
    }
}

#[test]
fn per_series_size_sets_the_marker_shape() {
    // Graphite markers are 5.6 pt across by default, outline included: the fill is 0.8 pt less
    // than the theme's size, alone or times a factor, and a fixed size is the fill's.
    let auto = 5.6_f32 - 0.8;
    for (spec, d) in [
        (SizeSpec::Auto, auto),
        (SizeSpec::times(1.0).unwrap(), auto),
        (SizeSpec::times(2.0).unwrap(), 2.0 * auto),
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
            let layers = merged_layers(mark_group(&scene).1);
            let [Item::Markers { shape, .. }] = layers.as_slice() else {
                panic!("expected one marker batch")
            };
            assert_eq!(shape, &marker_shape(m, d), "{spec:?} {m:?}");
        }
    }
}

#[test]
fn a_dense_scatter_shows_the_series_color_across_the_cloud() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let theme = theme.with_marker_alpha(1.0).unwrap();
        // 10,000 points about 0.4 pt apart, much closer than a marker is wide.
        let steps: Vec<f64> = (0..100).map(|i| 0.45 + f64::from(i) * 0.001).collect();
        let xs: Vec<f64> = steps.iter().flat_map(|_| steps.iter().copied()).collect();
        let ys: Vec<f64> = steps
            .iter()
            .flat_map(|&y| steps.iter().map(move |_| y))
            .collect();
        let f = scatter_figure(theme.clone(), &xs, &ys, Marker::Circle);
        let plot = plot_of(&f);
        let dpi = 300.0;
        let pixels = Pixels::decode(&f.to_png(dpi as i64).unwrap());
        let color = theme.palette()[0];
        for (u, v) in [
            (0.5, 0.5),
            (0.47, 0.47),
            (0.53, 0.47),
            (0.47, 0.53),
            (0.53, 0.53),
        ] {
            let at = (
                plot.x_at(NormalizedPos::new(u)),
                plot.y_at(NormalizedPos::new(v)),
            );
            let pixel = pixels.at_point(at, dpi);
            assert!(near(pixel, color, 2), "({u}, {v}): {pixel:?} vs {color:?}");
        }
    }
}

#[test]
fn a_small_point_shows_its_fill_at_the_centre() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let theme = theme.with_marker_alpha(1.0).unwrap();
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                nums(&[0.5]),
                &[0.5],
                ScatterOptions {
                    size: SizeSpec::pt(1.5).unwrap(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        let plot = plot_of(&f);
        let dpi = 720.0;
        let pixels = Pixels::decode(&f.to_png(dpi as i64).unwrap());
        let at = (
            plot.x_at(NormalizedPos::new(0.5)),
            plot.y_at(NormalizedPos::new(0.5)),
        );
        let pixel = pixels.at_point(at, dpi);
        assert!(near(pixel, theme.palette()[0], 2), "{pixel:?}");
    }
}

#[test]
fn an_isolated_default_point_keeps_its_visible_diameter() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let theme = theme.with_marker_alpha(1.0).unwrap();
        let f = scatter_figure(theme.clone(), &[0.5], &[0.5], Marker::Circle);
        let plot = plot_of(&f);
        let dpi = 720.0;
        let pixels = Pixels::decode(&f.to_png(dpi as i64).unwrap());
        let (cx, cy) = (
            plot.x_at(NormalizedPos::new(0.5)),
            plot.y_at(NormalizedPos::new(0.5)),
        );
        // The pixels along a row through the centre that differ from the background, in
        // points: the marker's size less its 0.8 pt outline, which is background-colored.
        let scale = dpi / 72.0;
        let row = (cy * scale) as usize;
        let start = ((cx - 6.0) * scale) as usize;
        let end = ((cx + 6.0) * scale) as usize;
        let drawn = (start..end)
            .filter(|&x| !near(pixels.at(x, row), theme.background(), 8))
            .count();
        let diameter = f64::from(theme.marker_size()) - 0.8;
        let measured = drawn as f64 / scale;
        assert!(
            (measured - diameter).abs() < 0.25,
            "{measured} vs {diameter}"
        );
    }
}

#[test]
fn bubbles_keep_each_fill_with_its_outline_in_order() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .bubbles(
            &[1.0, 1.5, 2.0],
            &[1.0, 1.0, 1.0],
            &[1.0, 1.0, 1.0],
            crate::BubbleOptions::default(),
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let kinds: Vec<&str> = mark_group(&scene)
        .1
        .iter()
        .map(|item| match item {
            Item::Fill { .. } => "fill",
            Item::Stroke { .. } => "outline",
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        ["fill", "outline", "fill", "outline", "fill", "outline"]
    );
}
