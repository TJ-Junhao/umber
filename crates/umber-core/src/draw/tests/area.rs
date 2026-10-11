//! Area marks: lines never fill, the fill under an area's line in both fill styles, the
//! baseline it ends at, NaN gaps and fills on both sides of the baseline.

use super::{Pixels, line_figure, mark_group, nums, plot_of, subpaths};
use crate::model::{AreaOptions, Figure, LineOptions};
use crate::resolve::NormalizedPos;
use crate::scene::{FillRule, Item, LinearGradient, Paint};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FillStyle, FontLibrary, GridLines, LogBase, ScaleKind, Theme};

const LOG: ScaleKind = ScaleKind::Log { base: LogBase::TEN };

/// Linear x axes, a y axis of `y_scale` and one area of `x` and `y`.
fn area_figure(theme: Theme, y_scale: ScaleKind, x: &[f64], y: &[f64]) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, y_scale)
        .unwrap()
        .area(nums(x), y, AreaOptions::default())
        .unwrap();
    f
}

fn themes() -> [Theme; 3] {
    [Theme::pigment(), Theme::graphite(), Theme::midnight()]
}

#[test]
fn lines_never_fill() {
    // In every theme and under either fill style, a line is its stroke alone.
    for theme in themes() {
        for style in [FillStyle::Solid, FillStyle::Gradient] {
            let theme = theme.with_fill_style(style).unwrap();
            let f = line_figure(
                theme,
                &[0.0, 1.0, 2.0, 3.0],
                &[10.0, 40.0, 25.0, 30.0],
                LineOptions::default(),
            );
            let scene = f.to_scene(FontLibrary::global());
            assert!(
                matches!(mark_group(&scene).1, [Item::Stroke { .. }]),
                "{style:?}: {:?}",
                mark_group(&scene).1
            );
        }
    }
}

#[test]
fn gradient_area_sits_under_its_line() {
    let theme = Theme::midnight();
    let f = area_figure(
        theme.clone(),
        ScaleKind::Linear,
        &[0.0, 1.0, 2.0, 3.0],
        &[10.0, 40.0, 25.0, 30.0],
    );
    let scene = f.to_scene(FontLibrary::global());
    assert_scene_bounded(&scene);
    let (_, items) = mark_group(&scene);
    let [
        Item::Fill {
            path: area,
            paint: Paint::Linear(gradient),
            rule: FillRule::NonZero,
        },
        Item::Stroke { path: line, style },
    ] = items
    else {
        panic!("expected an area fill under the line, got {items:?}")
    };
    let color = theme.palette()[0];
    assert_eq!(style.color, color);
    let (line_min, _) = line.bounds().unwrap();
    let (area_min, area_max) = area.bounds().unwrap();
    let (_, layout) = crate::pipeline::plan_figure(&f, FontLibrary::global());
    let baseline = layout.baseline_y as f32;
    assert_eq!(area_min.y, line_min.y);
    assert_eq!(area_max.y, baseline);
    // The area's outline is the line, then down to the baseline and back.
    let area_points = &subpaths(area)[0];
    let line_points = &subpaths(line)[0];
    assert_eq!(&area_points[..line_points.len()], line_points.as_slice());
    assert_eq!(area_points.len(), line_points.len() + 2);
    assert!(
        area_points[line_points.len()..]
            .iter()
            .all(|p| p.y == baseline)
    );
    // The gradient runs from the line's top at 0.32 alpha to the baseline at 0.
    assert_eq!(gradient.start.y, line_min.y);
    assert_eq!(gradient.end.y, baseline);
    assert_eq!(gradient.start.x, gradient.end.x);
    let alphas: Vec<(f32, u8)> = gradient
        .stops
        .iter()
        .map(|s| (s.offset, s.color.a))
        .collect();
    assert_eq!(alphas, [(0.0, 82), (1.0, 0)]);
    assert!(
        gradient
            .stops
            .iter()
            .all(|s| (s.color.r, s.color.g, s.color.b) == (color.r, color.g, color.b))
    );
}

#[test]
fn an_area_is_drawn_as_its_line_over_the_fill() {
    // With the same limits, an area's stroke is exactly the line mark's stroke: the fill comes
    // first and the line goes over it, unchanged.
    let (x, y) = (
        [0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        [10.0, 40.0, f64::NAN, 30.0, 20.0, f64::NAN],
    );
    for theme in themes() {
        let limited = |mut f: Figure| {
            f.axes_mut()
                .unwrap()
                .xlim(-0.5, 4.5)
                .unwrap()
                .ylim(0.0, 50.0)
                .unwrap();
            f.to_scene(FontLibrary::global())
        };
        let area = limited(area_figure(theme.clone(), ScaleKind::Linear, &x, &y));
        let line = limited(line_figure(theme, &x, &y, LineOptions::default()));
        let (area_clip, area_items) = mark_group(&area);
        let (line_clip, line_items) = mark_group(&line);
        assert_eq!(area_clip, line_clip);
        let [fills @ .., stroke] = area_items else {
            panic!("expected the fills and the stroke")
        };
        assert_eq!(fills.len(), 2);
        assert!(fills.iter().all(|item| matches!(item, Item::Fill { .. })));
        assert_eq!(std::slice::from_ref(stroke), line_items);
    }
}

#[test]
fn solid_area_is_flat_at_the_area_alpha() {
    // Pigment and Graphite fill flat; so does Midnight switched to `Solid`.
    for theme in [
        Theme::pigment(),
        Theme::graphite(),
        Theme::midnight().with_fill_style(FillStyle::Solid).unwrap(),
    ] {
        assert_eq!(theme.fill_style(), FillStyle::Solid);
        let color = theme.palette()[0];
        let f = area_figure(theme, ScaleKind::Linear, &[0.0, 1.0, 2.0], &[3.0, 5.0, 4.0]);
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let [
            Item::Fill {
                paint: Paint::Solid(fill),
                rule: FillRule::NonZero,
                ..
            },
            Item::Stroke { style, .. },
        ] = mark_group(&scene).1
        else {
            panic!("expected a solid fill under the line")
        };
        assert_eq!(style.color, color);
        assert_eq!(*fill, crate::Rgba { a: 82, ..color });
    }
    // A faded color fades the fill with it: 0.32 of its own opacity.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .area(
            nums(&[0.0, 1.0]),
            &[1.0, 2.0],
            AreaOptions {
                color: crate::ColorSpec::Auto(crate::Alpha::new(0.5).unwrap()),
                ..AreaOptions::default()
            },
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global());
    let [
        Item::Fill {
            paint: Paint::Solid(fill),
            ..
        },
        Item::Stroke { style, .. },
    ] = mark_group(&scene).1
    else {
        panic!("expected a solid fill under the line")
    };
    assert_eq!(style.color.a, 128);
    assert_eq!(fill.a, 41);
}

#[test]
fn areas_include_zero_on_a_linear_y_axis() {
    // y from 10 to 40 alone would take the limits 5..40; an area keeps 0 on the axis, at the
    // plot bottom without padding, and the baseline sits there.
    for theme in themes() {
        let f = area_figure(
            theme,
            ScaleKind::Linear,
            &[0.0, 1.0, 2.0, 3.0],
            &[10.0, 40.0, 25.0, 30.0],
        );
        let (resolved, layout) = crate::pipeline::plan_figure(&f, FontLibrary::global());
        let lowest = &resolved.y.ticks[0];
        assert_eq!(lowest.label, "0");
        assert_eq!(resolved.baseline, lowest.pos);
        assert_eq!(resolved.y.continuous().unwrap().domain().0, 0.0);
        assert!((layout.baseline_y - layout.plot.bottom()).abs() < 1e-9);
        let scene = f.to_scene(FontLibrary::global());
        let Some(Item::Fill { path, .. }) = mark_group(&scene).1.first() else {
            panic!("expected an area fill")
        };
        assert_eq!(path.bounds().unwrap().1.y, layout.baseline_y as f32);
    }
    // Negative values too: the axis spans them and 0.
    let f = area_figure(
        Theme::pigment(),
        ScaleKind::Linear,
        &[0.0, 1.0],
        &[-30.0, -10.0],
    );
    let (resolved, _) = crate::pipeline::plan_figure(&f, FontLibrary::global());
    assert_eq!(resolved.y.continuous().unwrap().domain().1, 0.0);
}

#[test]
fn areas_fill_down_to_the_baseline_on_a_log_y_axis() {
    // y from 10 to 40 takes the nice limits 10..100, but its minimum lands on the lowest tick,
    // so the limit moves one decade down, to 1, and is padded below that: the baseline sits on
    // the lowest tick, above the plot bottom and below the line, and the area fills down to it.
    for theme in themes() {
        let f = area_figure(theme, LOG, &[0.0, 1.0, 2.0, 3.0], &[10.0, 40.0, 25.0, 30.0]);
        let (resolved, layout) = crate::pipeline::plan_figure(&f, FontLibrary::global());
        let lowest = &resolved.y.ticks[0];
        assert_eq!(lowest.label, "1");
        assert_eq!(resolved.baseline, lowest.pos);
        assert!(layout.baseline_y < layout.plot.bottom() - 1.0);
        let scene = f.to_scene(FontLibrary::global());
        let [
            Item::Fill {
                path: area, paint, ..
            },
            Item::Stroke { .. },
        ] = mark_group(&scene).1
        else {
            panic!("expected an area fill under the line");
        };
        let baseline = layout.baseline_y as f32;
        let (top, bottom) = area.bounds().unwrap();
        assert_eq!(bottom.y, baseline);
        assert!(top.y < baseline);
        if let Paint::Linear(gradient) = paint {
            assert_eq!(gradient.end.y, baseline);
        }
        // The line's lowest point, 10, lies above the baseline.
        let ten = resolved.y.continuous().unwrap().normalize(10.0);
        assert!(layout.plot.y_at(NormalizedPos::new(ten)) < layout.baseline_y - 1.0);
    }
}

#[test]
fn nan_breaks_the_line_and_the_fill() {
    let f = area_figure(
        Theme::midnight(),
        ScaleKind::Linear,
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        &[1.0, 2.0, f64::NAN, 3.0, 2.0, 4.0],
    );
    let scene = f.to_scene(FontLibrary::global());
    let [
        Item::Fill { path: first, .. },
        Item::Fill { path: second, .. },
        Item::Stroke { path: line, .. },
    ] = mark_group(&scene).1
    else {
        panic!("expected a fill for each run, then the line")
    };
    let runs = subpaths(line);
    assert_eq!(runs.len(), 2);
    // Each fill is its run, then down to the baseline and back.
    for (fill, run) in [first, second].into_iter().zip(&runs) {
        let outline = &subpaths(fill)[0];
        assert_eq!(&outline[..run.len()], run.as_slice());
        assert_eq!(outline.len(), run.len() + 2);
    }
    // Nothing spans the gap.
    assert!(first.bounds().unwrap().1.x < second.bounds().unwrap().0.x);
}

/// The PNG pixel at data position `(x, y)` of a figure with limits 0..1 by -2..2.
fn data_pixel(f: &Figure, png: &[u8], x: f64, y: f64) -> [u8; 3] {
    let plot = plot_of(f);
    Pixels::decode(png).at_point(
        (
            plot.x_at(NormalizedPos::new(x)),
            plot.y_at(NormalizedPos::new((y + 2.0) / 4.0)),
        ),
        300.0,
    )
}

/// `f` limited to 0..1 by -2..2.
fn limited(mut f: Figure) -> Figure {
    f.axes_mut()
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(-2.0, 2.0)
        .unwrap();
    f
}

/// Whether the area fill shows at each data position: the pixel differs from the same data
/// drawn as a line.
fn area_shows(x: &[f64], y: &[f64], probes: &[(f64, f64)]) -> Vec<bool> {
    let theme = Theme::midnight().with_grid_lines(GridLines::None).unwrap();
    let with = limited(area_figure(theme.clone(), ScaleKind::Linear, x, y));
    let without = limited(line_figure(theme, x, y, LineOptions::default()));
    let (a, b) = (with.to_png(300).unwrap(), without.to_png(300).unwrap());
    probes
        .iter()
        .map(|&(px, py)| data_pixel(&with, &a, px, py) != data_pixel(&without, &b, px, py))
        .collect()
}

#[test]
fn areas_crossing_the_baseline_fill_both_sides() {
    // One line from +1 down to -1: filled above the baseline on the left, below on the
    // right, and nowhere beyond the line.
    let shows = area_shows(
        &[0.0, 0.5, 1.0],
        &[1.0, 0.0, -1.0],
        &[(0.1, 0.4), (0.9, -0.4), (0.1, -0.4), (0.9, 0.4)],
    );
    assert_eq!(shows, [true, true, false, false]);

    // NaN-separated runs on opposite sides, each fading towards the baseline, and nothing in
    // the gap.
    let shows = area_shows(
        &[0.0, 0.4, f64::NAN, 0.6, 1.0],
        &[1.0, 1.0, f64::NAN, -1.0, -1.0],
        &[(0.2, 0.5), (0.8, -0.5), (0.2, -0.5), (0.8, 0.5), (0.5, 0.5)],
    );
    assert_eq!(shows, [true, true, false, false, false]);

    // Each side is its own fill, with its gradient from its extreme to the baseline.
    let f = limited(area_figure(
        Theme::midnight(),
        ScaleKind::Linear,
        &[0.0, 0.5, 1.0],
        &[1.0, 0.0, -1.0],
    ));
    let scene = f.to_scene(FontLibrary::global());
    assert_scene_bounded(&scene);
    let plot = plot_of(&f);
    let baseline = plot.y_at(NormalizedPos::new(0.5)) as f32;
    let gradients: Vec<&LinearGradient> = mark_group(&scene)
        .1
        .iter()
        .filter_map(|item| match item {
            Item::Fill {
                paint: Paint::Linear(g),
                ..
            } => Some(g),
            _ => None,
        })
        .collect();
    let [above, below] = gradients.as_slice() else {
        panic!("expected two area fills, got {gradients:?}")
    };
    assert!((above.start.y - plot.y_at(NormalizedPos::new(0.75)) as f32).abs() < 1e-3);
    assert!((below.start.y - plot.y_at(NormalizedPos::new(0.25)) as f32).abs() < 1e-3);
    for g in [above, below] {
        assert_eq!(g.end.y, baseline);
        assert_eq!((g.stops[0].color.a, g.stops[1].color.a), (82, 0));
    }
}

#[test]
fn opposite_runs_do_not_cancel_their_areas() {
    let shows = area_shows(
        &[0.0, 1.0, f64::NAN, 1.0, 0.0],
        &[1.0, 1.0, f64::NAN, 1.0, 1.0],
        &[(0.5, 0.5)],
    );
    assert_eq!(shows, [true]);
    // A reversed run alone is filled too.
    assert_eq!(area_shows(&[1.0, 0.0], &[1.0, 1.0], &[(0.5, 0.5)]), [true]);
}

/// The mark items of an area of `x` and `y` in Midnight, on a y axis of `y_scale` with the
/// limits `xlim` and `ylim`, with the plot area.
fn limited_area(
    y_scale: ScaleKind,
    x: &[f64],
    y: &[f64],
    ylim: (f64, f64),
) -> (crate::scene::Scene, crate::layout::Layout) {
    let mut f = area_figure(Theme::midnight(), y_scale, x, y);
    f.axes_mut()
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(ylim.0, ylim.1)
        .unwrap();
    let (_, layout) = crate::pipeline::plan_figure(&f, FontLibrary::global());
    (f.to_scene(FontLibrary::global()), layout)
}

/// The line margin of Midnight's line width, in points.
fn margin() -> f32 {
    Theme::midnight()
        .drawn_line_width(crate::SizeSpec::Auto)
        .max(2.0)
}

#[test]
fn an_area_whose_line_runs_above_the_plot_fills_it() {
    // The line at 2 (100 on log) lies above a y range of 0..1 (1..10): no stroke shows, but
    // the region down to the baseline covers the whole plot, on a linear and on a log y axis.
    for (y_scale, y, ylim) in [
        (ScaleKind::Linear, [2.0, 2.0], (0.0, 1.0)),
        (LOG, [100.0, 100.0], (1.0, 10.0)),
    ] {
        let (scene, layout) = limited_area(y_scale, &[0.0, 1.0], &y, ylim);
        assert_scene_bounded(&scene);
        let plot = layout.plot;
        let [
            Item::Fill {
                path,
                paint: Paint::Linear(gradient),
                ..
            },
        ] = mark_group(&scene).1
        else {
            panic!(
                "expected one fill and no stroke, got {:?}",
                mark_group(&scene).1
            )
        };
        let (lo, hi) = path.bounds().unwrap();
        let top = plot.y as f32 - margin();
        assert_eq!((lo.x, hi.x), (plot.x as f32, plot.right() as f32));
        assert_eq!((lo.y, hi.y), (top, layout.baseline_y as f32));
        assert_eq!(layout.baseline_y, plot.bottom());
        // The gradient stays inside the clip, from its edge to the baseline, and keeps the true
        // ramp from the line at 2 (100): the plot top, halfway up the ramp in axis space,
        // gets 0.16 of the line's opacity, not the 0.32 of the line itself.
        assert_eq!((gradient.start.y, gradient.end.y), (top, hi.y));
        assert_ramp(gradient, plot.y as f32, 0.16);
        assert_ramp(gradient, layout.baseline_y as f32, 0.0);
    }
}

/// The opacity of the vertical gradient `g` at plot y `y`, as a share of the full opacity:
/// its two stops interpolated, as the renderers do.
fn share_at(g: &LinearGradient, y: f32) -> f32 {
    let [from, to] = g.stops.as_slice() else {
        panic!("expected two stops, got {:?}", g.stops)
    };
    let t = ((y - g.start.y) / (g.end.y - g.start.y)).clamp(0.0, 1.0);
    (f32::from(from.color.a) + t * (f32::from(to.color.a) - f32::from(from.color.a))) / 255.0
}

/// Asserts that `g` gives `expected` of the full opacity at plot y `y`, within the rounding of
/// its 8-bit stops.
fn assert_ramp(g: &LinearGradient, y: f32, expected: f32) {
    let share = share_at(g, y);
    assert!(
        (share - expected).abs() <= 1.0 / 255.0,
        "share {share} at {y}, expected {expected}: {g:?}"
    );
}

#[test]
fn clipped_ramps_keep_the_lines_opacity_at_every_height() {
    // Each case's line lies at twice the distance from the baseline that the plot edge does,
    // so the edge of the plot gets half of the line's 0.32: below the baseline for negative
    // values, and on both sides of a baseline that the line crosses.
    let fills = |y: &[f64], ylim: (f64, f64)| {
        let (scene, layout) = limited_area(ScaleKind::Linear, &[0.0, 1.0], y, ylim);
        assert_scene_bounded(&scene);
        let gradients: Vec<LinearGradient> = mark_group(&scene)
            .1
            .iter()
            .filter_map(|item| match item {
                Item::Fill {
                    paint: Paint::Linear(g),
                    ..
                } => Some(g.clone()),
                _ => None,
            })
            .collect();
        (gradients, layout)
    };
    // Negative values: 0 is the top of a -1..0 axis, and the line at -2 lies below it.
    let (gradients, layout) = fills(&[-2.0, -2.0], (-1.0, 0.0));
    let [below] = gradients.as_slice() else {
        panic!("one fill expected, got {gradients:?}")
    };
    assert!((layout.baseline_y - layout.plot.y).abs() < 1e-9);
    assert_ramp(below, layout.plot.bottom() as f32, 0.16);
    assert_ramp(below, layout.plot.y as f32, 0.0);
    // A baseline crossing: from 2 down to -2 on a -1..1 axis, with the baseline in the middle.
    let (gradients, layout) = fills(&[2.0, -2.0], (-1.0, 1.0));
    let [above, below] = gradients.as_slice() else {
        panic!("two fills expected, got {gradients:?}")
    };
    assert_ramp(above, layout.plot.y as f32, 0.16);
    assert_ramp(below, layout.plot.bottom() as f32, 0.16);
    for g in [above, below] {
        assert_ramp(g, layout.baseline_y as f32, 0.0);
    }
    // The ramp's own extreme: a line just inside the plot keeps exactly 0.32 there.
    let (gradients, _) = fills(&[0.75, 0.5], (0.0, 1.0));
    let [inside] = gradients.as_slice() else {
        panic!("one fill expected, got {gradients:?}")
    };
    assert_eq!(inside.stops[0].color.a, 82);
}

#[test]
fn an_area_entering_from_above_fills_from_its_first_point() {
    // From 2 down to 0.5 on a y range of 0..1: the stroke starts where the line enters the
    // clip, but the fill starts at x = 0, along the top of the clip.
    let (scene, layout) = limited_area(ScaleKind::Linear, &[0.0, 1.0], &[2.0, 0.5], (0.0, 1.0));
    assert_scene_bounded(&scene);
    let plot = layout.plot;
    let [
        Item::Fill {
            path: area,
            paint: Paint::Linear(gradient),
            ..
        },
        Item::Stroke { path: line, .. },
    ] = mark_group(&scene).1
    else {
        panic!(
            "expected a fill and the stroke, got {:?}",
            mark_group(&scene).1
        )
    };
    let (area_lo, area_hi) = area.bounds().unwrap();
    let (line_lo, _) = line.bounds().unwrap();
    assert_eq!(area_lo.x, plot.x as f32);
    assert_eq!(area_hi.x, plot.right() as f32);
    assert!(line_lo.x > plot.x as f32 + 100.0, "{line_lo:?}");
    // The fill's top runs along the clip edge until the line enters, then follows it.
    let top = plot.y as f32 - margin();
    let outline = &subpaths(area)[0];
    let entry = subpaths(line)[0][0];
    assert_eq!(
        outline[0],
        crate::geometry::Point {
            x: area_lo.x,
            y: top
        }
    );
    assert_eq!(outline[1], entry);
    assert_eq!(entry.y, top);
    // The gradient stays inside the clip and keeps the true ramp from the line's start at 2:
    // the plot top gets 0.16 of the line's opacity.
    assert_eq!(gradient.start.y, top);
    assert_eq!(gradient.end.y, layout.baseline_y as f32);
    assert_ramp(gradient, plot.y as f32, 0.16);
}
