//! Bar marks: band share, base and end, clipping in axis space and the rounded end; grouped
//! bars side by side and stacked segments.

use super::mark_group;
use crate::geometry::{Path, PathVerb, Point};
use crate::layout::Layout;
use crate::model::{AxisData, BarOptions, Figure, LineOptions, Series};
use crate::render::{SvgText, render_png, render_svg};
use crate::scene::{Item, Paint, Scene};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, LogBase, Rgba, ScaleKind, Theme};

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// A figure with a category x axis, a y axis of `y` and one bar mark.
fn bar_figure(theme: Theme, y: ScaleKind, categories: &[&str], heights: &[f64]) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Category, y)
        .unwrap()
        .bar(names(categories), heights, BarOptions::default())
        .unwrap();
    f
}

fn layout_of(figure: &Figure) -> Layout {
    crate::pipeline::plan_figure(figure, (0, 0), FontLibrary::global()).1
}

/// The bar fills of `scene`, in order: each outline and its color.
fn bars(scene: &Scene) -> Vec<(&Path, Rgba)> {
    mark_group(scene)
        .1
        .iter()
        .map(|item| match item {
            Item::Fill {
                path,
                paint: Paint::Solid(color),
                ..
            } => (path, *color),
            other => panic!("expected a solid bar fill, got {other:?}"),
        })
        .collect()
}

fn cubics(path: &Path) -> usize {
    path.verbs()
        .iter()
        .filter(|v| matches!(v, PathVerb::CubicTo(..)))
        .count()
}

fn bounds(path: &Path) -> (Point, Point) {
    path.bounds().unwrap()
}

#[track_caller]
fn assert_near(actual: f32, expected: f64) {
    assert!(
        (f64::from(actual) - expected).abs() < 1e-3,
        "{actual} != {expected}"
    );
}

#[test]
fn plain_bars_take_their_share_of_the_band_from_the_baseline() {
    let theme = Theme::graphite();
    let heights = [3.0, 0.0, f64::NAN, -1.0];
    let f = bar_figure(
        theme.clone(),
        ScaleKind::Linear,
        &["a", "b", "c", "d"],
        &heights,
    );
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let layout = layout_of(&f);
    let plot = layout.plot;
    // A zero and a NaN height draw nothing; the others draw one fill each, in category order.
    let fills = bars(&scene);
    assert_eq!(fills.len(), 2);
    for ((path, color), (index, height)) in fills.iter().zip([(0, 3.0), (3, -1.0)]) {
        assert_eq!(*color, theme.palette()[0]);
        // Four bands of a quarter of the plot width each; the bar takes 72 % of its band.
        let center = (f64::from(index) + 0.5) / 4.0;
        let (lo, hi) = bounds(path);
        assert_near(lo.x, plot.x + (center - 0.09) * plot.width);
        assert_near(hi.x, plot.x + (center + 0.09) * plot.width);
        // From the baseline to the height.
        let (base, end) = if height > 0.0 {
            (hi.y, lo.y)
        } else {
            (lo.y, hi.y)
        };
        assert_near(base, layout.baseline_y());
        assert!((end - base) * (height as f32) < 0.0, "y points down");
        // Graphite has no corner radius: square corners.
        assert_eq!(cubics(path), 0);
    }
}

#[test]
fn bar_marks_overlap_in_insertion_order() {
    let theme = Theme::pigment();
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .bar(names(&["a", "b"]), &[2.0, 4.0], BarOptions::default())
        .unwrap()
        .bar(names(&["b"]), &[1.0], BarOptions::default())
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let fills = bars(&scene);
    let colors: Vec<Rgba> = fills.iter().map(|(_, c)| *c).collect();
    let palette = theme.palette();
    assert_eq!(colors, [palette[0], palette[0], palette[1]]);
    // The later mark sits in the same place as the earlier one, over it.
    let (b_first, b_second) = (bounds(fills[1].0), bounds(fills[2].0));
    assert_eq!((b_first.0.x, b_first.1.x), (b_second.0.x, b_second.1.x));
    assert_eq!(b_first.1.y, b_second.1.y);
}

#[test]
fn bars_are_drawn_in_category_order() {
    // An earlier all-NaN line sets the category order a, b, c; the bars list b, c, a.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Categories(names(&["a", "b", "c"])),
            &[f64::NAN; 3],
            LineOptions::default(),
        )
        .unwrap()
        .bar(
            names(&["b", "c", "a"]),
            &[2.0, 3.0, 1.0],
            BarOptions::default(),
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let plot = layout_of(&f).plot;
    let fills = bars(&scene);
    assert_eq!(fills.len(), 3);
    for (i, (path, _)) in fills.iter().enumerate() {
        let center = (i as f64 + 0.5) / 3.0;
        assert_near(bounds(path).0.x, plot.x + (center - 0.12) * plot.width);
    }
}

#[test]
fn bars_stay_solid_in_a_gradient_theme() {
    let f = bar_figure(Theme::midnight(), ScaleKind::Linear, &["a"], &[2.0]);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    // `bars` panics on anything but a solid fill.
    assert_eq!(bars(&scene).len(), 1);
}

#[test]
fn rounded_end_only_when_visible() {
    // Midnight rounds bar ends by 3.5 pt.
    let theme = Theme::midnight();
    let radius = f64::from(theme.bar_corner_radius());
    let mut f = bar_figure(
        theme,
        ScaleKind::Linear,
        &["inside", "outside", "margin"],
        &[5.0, 1000.0, 10.0 + 1e-3],
    );
    f.single_axes_mut().ylim(0.0, 10.0).unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let plot = layout_of(&f).plot;
    let fills = bars(&scene);
    assert_eq!(fills.len(), 3);
    // Inside the plot: the end is rounded.
    assert_eq!(cubics(fills[0].0), 2);
    // Far above the limits: cut square, `radius` above the plot area, so the cut is never
    // seen through the plot clip.
    assert_eq!(cubics(fills[1].0), 0);
    assert_near(bounds(fills[1].0).0.y, plot.y - radius);
    // Just past the upper limit but inside the clip margin: the true end is kept and rounded.
    assert_eq!(cubics(fills[2].0), 2);
    assert!(f64::from(bounds(fills[2].0).0.y) < plot.y);
}

#[test]
fn negative_bar_rounds_its_bottom() {
    let f = bar_figure(Theme::midnight(), ScaleKind::Linear, &["a"], &[-4.0]);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let layout = layout_of(&f);
    let fills = bars(&scene);
    let [(path, _)] = fills.as_slice() else {
        panic!("expected one bar, got {fills:?}")
    };
    assert_eq!(cubics(path), 2);
    // The base sits on the baseline, at the top of the plot (the domain ends at 0), and the
    // bar grows downwards; its square corners are at the base.
    let (lo, hi) = bounds(path);
    assert_near(lo.y, layout.baseline_y());
    assert_near(lo.y, layout.plot.y);
    assert!(hi.y > lo.y);
    let base_corners = path
        .verbs()
        .iter()
        .filter(|v| matches!(v, PathVerb::MoveTo(p) | PathVerb::LineTo(p) if p.y == lo.y))
        .count();
    assert_eq!(base_corners, 2);
}

#[test]
fn bars_far_outside_limits_are_clipped() {
    // Heights at the largest magnitudes with limits 1e-10 wide: mapping the raw heights would
    // overflow, so every bar is clipped in axis space first.
    for theme in [Theme::pigment(), Theme::midnight()] {
        let mut f = bar_figure(
            theme,
            ScaleKind::Linear,
            &["up", "down", "inside", "below"],
            &[1e300, -1e300, 5e-11, -1e300],
        );
        f.single_axes_mut().ylim(1e-11, 1e-10).unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        assert_scene_bounded(&scene);
        let plot = layout_of(&f).plot;
        let fills = bars(&scene);
        // Bars that lie wholly below the limits (base 0 and end -1e300) draw nothing; the others
        // stay within a few points of the plot area.
        assert_eq!(fills.len(), 2);
        for (path, _) in &fills {
            let (lo, hi) = bounds(path);
            assert!(f64::from(lo.y) >= plot.y - 4.0, "{lo:?}");
            assert!(f64::from(hi.y) <= plot.bottom() + 4.0, "{hi:?}");
        }
        assert!(render_png(&scene, 72).is_ok());
        assert!(!render_svg(&scene, SvgText::Outlines).is_empty());
    }

    // The same on a log y axis, with heights at both ends of the log range.
    let log = ScaleKind::Log { base: LogBase::TEN };
    let mut f = bar_figure(Theme::midnight(), log, &["huge", "tiny"], &[1e300, 1e-300]);
    f.single_axes_mut().ylim(1.0, 10.0).unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let plot = layout_of(&f).plot;
    let fills = bars(&scene);
    assert_eq!(fills.len(), 2);
    // The huge bar fills the plot from the bottom up; the tiny one runs from the bottom down,
    // into the clip margin below the plot area, where the marks clip hides it.
    let (huge, tiny) = (bounds(fills[0].0), bounds(fills[1].0));
    assert_near(huge.1.y, plot.bottom());
    assert!(f64::from(huge.0.y) >= plot.y - 4.0, "{huge:?}");
    assert_near(tiny.0.y, plot.bottom());
    assert!(f64::from(tiny.1.y) <= plot.bottom() + 4.0, "{tiny:?}");
}

#[test]
fn log_bars_grow_from_the_baseline() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    // The nice limits are 1..1000: the baseline sits at 1, the lowest tick, above the padded
    // plot bottom, and both bars grow from it.
    let f = bar_figure(Theme::graphite(), log, &["a", "b"], &[3.0, 1000.0]);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let layout = layout_of(&f);
    assert!(
        layout.baseline_y() < layout.plot.bottom() - 1.0,
        "{layout:?}"
    );
    let fills = bars(&scene);
    assert_eq!(fills.len(), 2);
    for (path, _) in &fills {
        assert_near(bounds(path).1.y, layout.baseline_y());
    }
    // A bar as high as the nice lower limit, 10, moves that limit a decade down, to 1: the
    // bar rises above the baseline and is drawn.
    let f = bar_figure(Theme::graphite(), log, &["a", "b"], &[10.0, 1000.0]);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let layout = layout_of(&f);
    let fills = bars(&scene);
    assert_eq!(fills.len(), 2);
    for (path, _) in &fills {
        let (top, base) = bounds(path);
        assert_near(base.y, layout.baseline_y());
        assert!(f64::from(top.y) < layout.baseline_y() - 1.0, "{top:?}");
    }
    // Explicit limits are not padded: the baseline, and the bars, start at the plot bottom.
    let mut f = bar_figure(Theme::graphite(), log, &["a", "b"], &[10.0, 1000.0]);
    f.single_axes_mut().ylim(5.0, 2000.0).unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let layout = layout_of(&f);
    let fills = bars(&scene);
    assert_eq!(fills.len(), 2);
    assert_near(layout.plot.bottom() as f32, layout.baseline_y());
    for (path, _) in &fills {
        assert_near(bounds(path).1.y, layout.baseline_y());
    }
}

fn series(values: &[f64]) -> Series {
    Series {
        values: values.to_vec(),
        ..Series::default()
    }
}

/// A figure with a category x axis, a y axis of `y` and one grouped (or stacked) bar mark.
fn group_figure(
    theme: Theme,
    y: ScaleKind,
    stacked: bool,
    categories: &[&str],
    heights: &[&[f64]],
) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    let mut axes = f.add_axes(ScaleKind::Category, y).unwrap();
    let list: Vec<Series> = heights.iter().map(|h| series(h)).collect();
    if stacked {
        axes.stacked_bar(names(categories), list).unwrap();
    } else {
        axes.grouped_bar(names(categories), list).unwrap();
    }
    f
}

#[test]
fn group_fills_72_percent_of_band() {
    let theme = Theme::graphite();
    let f = group_figure(
        theme.clone(),
        ScaleKind::Linear,
        false,
        &["a", "b"],
        &[&[3.0, 1.0], &[2.0, -1.0], &[4.0, 2.0]],
    );
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let plot = layout_of(&f).plot;
    let fills = bars(&scene);
    assert_eq!(fills.len(), 6);
    let palette = theme.palette();
    for (category, group) in fills.chunks(3).enumerate() {
        // Two bands of half the plot width each; the group takes 72 % of its band.
        let center = plot.x + (category as f64 + 0.5) / 2.0 * plot.width;
        let half = 0.36 * plot.width / 2.0;
        let spans: Vec<(f32, f32)> = group
            .iter()
            .map(|(path, _)| {
                let (lo, hi) = bounds(path);
                (lo.x, hi.x)
            })
            .collect();
        assert_near(spans[0].0, center - half);
        assert_near(spans[2].1, center + half);
        // Equal widths, 2 pt apart, in series order and colors.
        let width = (2.0 * half - 2.0 * 2.0) / 3.0;
        for (j, &(left, right)) in spans.iter().enumerate() {
            assert_near(right - left, width);
            assert_near(left, center - half + j as f64 * (width + 2.0));
            assert_eq!(group[j].1, palette[j]);
        }
    }
}

#[test]
fn group_gap_closes_when_bars_would_be_thinner_than_it() {
    // Twelve series in twelve categories: with 2 pt gaps the bars would be narrower than the
    // gap, so they touch and share the group evenly.
    let heights: Vec<Vec<f64>> = (0..12).map(|j| vec![f64::from(j) + 1.0; 12]).collect();
    let heights: Vec<&[f64]> = heights.iter().map(Vec::as_slice).collect();
    let categories: Vec<String> = (0..12).map(|i| format!("c{i}")).collect();
    let categories: Vec<&str> = categories.iter().map(String::as_str).collect();
    let f = group_figure(
        Theme::pigment(),
        ScaleKind::Linear,
        false,
        &categories,
        &heights,
    );
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let plot = layout_of(&f).plot;
    let fills = bars(&scene);
    assert_eq!(fills.len(), 144);
    let group = 0.72 * plot.width / 12.0;
    assert!(
        (group - 11.0 * 2.0) / 12.0 < 2.0,
        "the case must close the gap"
    );
    let first: Vec<(Point, Point)> = fills[..12].iter().map(|(path, _)| bounds(path)).collect();
    assert_near(first[0].0.x, plot.x + (0.5 - 0.36) * plot.width / 12.0);
    for pair in first.windows(2) {
        assert_near(pair[1].0.x - pair[0].1.x, 0.0);
        assert_near(pair[0].1.x - pair[0].0.x, group / 12.0);
    }
}

#[test]
fn stacked_signs_stack_separately() {
    // Midnight rounds bar ends by 3.5 pt; only the outermost segment on each side is rounded.
    let f = group_figure(
        Theme::midnight(),
        ScaleKind::Linear,
        true,
        &["a"],
        &[&[3.0], &[-2.0], &[f64::NAN], &[4.0], &[0.0]],
    );
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let layout = layout_of(&f);
    let fills = bars(&scene);
    // NaN and 0 add no segment.
    let [(up, _), (down, _), (top, _)] = fills.as_slice() else {
        panic!("expected three segments, got {fills:?}")
    };
    let (up, down, top) = (*up, *down, *top);
    let base = layout.baseline_y();
    // Points per unit, from the outermost positive segment, whose end at 7 is not inset.
    let (top_lo, top_hi) = bounds(top);
    let unit = (base - f64::from(top_lo.y)) / 7.0;
    // The first segment runs from the baseline to 3, less half the stack gap at its inner end.
    let (up_lo, up_hi) = bounds(up);
    assert_near(up_hi.y, base);
    assert_near(up_lo.y, base - 3.0 * unit + 0.5);
    // The negative segment grows down from the baseline, with no gap at 0; the third stacks on
    // the first, half the gap above their shared boundary.
    let (down_lo, down_hi) = bounds(down);
    assert_near(down_lo.y, base);
    assert_near(down_hi.y, base + 2.0 * unit);
    assert_near(top_hi.y, base - 3.0 * unit - 0.5);
    // One stack: every segment spans the same x range.
    assert_eq!((up_lo.x, up_hi.x), (top_lo.x, top_hi.x));
    assert_eq!((up_lo.x, up_hi.x), (down_lo.x, down_hi.x));
    assert_eq!((cubics(up), cubics(down), cubics(top)), (0, 2, 2));
    // The colors are those of the series that drew: the first, second and fourth.
    let palette = Theme::midnight().palette().to_vec();
    let colors: Vec<Rgba> = fills.iter().map(|(_, c)| *c).collect();
    assert_eq!(colors, [palette[0], palette[1], palette[3]]);
}

#[test]
fn grouped_and_stacked_bars_are_drawn_by_category_then_series() {
    // An earlier all-NaN line sets the category order a, b; the marks list b, a.
    for stacked in [false, true] {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        axes.line(
            AxisData::Categories(names(&["a", "b"])),
            &[f64::NAN; 2],
            LineOptions::default(),
        )
        .unwrap();
        let list = vec![series(&[2.0, 1.0]), series(&[3.0, 4.0])];
        if stacked {
            axes.stacked_bar(names(&["b", "a"]), list).unwrap();
        } else {
            axes.grouped_bar(names(&["b", "a"]), list).unwrap();
        }
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let plot = layout_of(&f).plot;
        let palette = Theme::pigment().palette().to_vec();
        let fills = bars(&scene);
        let order: Vec<(bool, Rgba)> = fills
            .iter()
            .map(|(path, color)| {
                (
                    f64::from(bounds(path).0.x) < plot.x + plot.width / 2.0,
                    *color,
                )
            })
            .collect();
        assert_eq!(
            order,
            // The line takes the first palette slot.
            [
                (true, palette[1]),
                (true, palette[2]),
                (false, palette[1]),
                (false, palette[2]),
            ],
            "stacked: {stacked}"
        );
    }
}

#[test]
fn log_stacks_start_at_the_baseline() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let f = group_figure(
        Theme::graphite(),
        log,
        true,
        &["a", "b"],
        &[&[10.0, 3.0], &[90.0, 300.0]],
    );
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let layout = layout_of(&f);
    let fills = bars(&scene);
    assert_eq!(fills.len(), 4);
    // The first segment of each stack starts at the baseline, at the nice lower limit 1, above
    // the padded plot bottom; the second starts where the first ends.
    assert!(
        layout.baseline_y() < layout.plot.bottom() - 1.0,
        "{layout:?}"
    );
    for pair in fills.chunks(2) {
        let (first, second) = (bounds(pair[0].0), bounds(pair[1].0));
        assert_near(first.1.y, layout.baseline_y());
        // One point apart: the stack gap.
        assert_near(first.0.y - second.1.y, 1.0);
        assert!(second.0.y < first.0.y);
    }
}

#[test]
fn stacks_and_groups_far_outside_limits_are_clipped() {
    // Totals at the largest magnitude, under limits 1e-10 wide.
    for stacked in [false, true] {
        let mut f = group_figure(
            Theme::midnight(),
            ScaleKind::Linear,
            stacked,
            &["up", "down", "inside"],
            &[&[5e299, -5e299, 2e-11], &[5e299, -5e299, 3e-11]],
        );
        f.single_axes_mut().ylim(1e-11, 1e-10).unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        assert_scene_bounded(&scene);
        let plot = layout_of(&f).plot;
        let fills = bars(&scene);
        assert!(!fills.is_empty());
        for (path, _) in &fills {
            let (lo, hi) = bounds(path);
            assert!(f64::from(lo.y) >= plot.y - 4.0, "{lo:?}");
            assert!(f64::from(hi.y) <= plot.bottom() + 4.0, "{hi:?}");
        }
        assert!(render_png(&scene, 72).is_ok());
        assert!(!render_svg(&scene, SvgText::Outlines).is_empty());
    }
}

/// The segment fills of a one-category stack of `heights` on a linear y axis in `theme`, with
/// the baseline y and the plot points per unit, measured from the stack's outermost positive
/// end at `total`.
fn one_stack(
    theme: Theme,
    heights: &[&[f64]],
    total: f64,
) -> (Vec<(Point, Point)>, Vec<usize>, f64, f64) {
    let f = group_figure(theme, ScaleKind::Linear, true, &["a"], heights);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let base = layout_of(&f).baseline_y();
    let fills = bars(&scene);
    let spans: Vec<(Point, Point)> = fills.iter().map(|(path, _)| bounds(path)).collect();
    let curves: Vec<usize> = fills.iter().map(|(path, _)| cubics(path)).collect();
    let top = spans
        .iter()
        .map(|(lo, _)| f64::from(lo.y))
        .fold(f64::INFINITY, f64::min);
    (spans, curves, base, (base - top) / total)
}

#[test]
fn stacked_segments_are_one_point_apart() {
    let (spans, _, base, unit) = one_stack(Theme::graphite(), &[&[3.0], &[4.0], &[2.0]], 9.0);
    assert_eq!(spans.len(), 3);
    for pair in spans.windows(2) {
        // The lower segment's top and the upper one's bottom, y down.
        assert_near(pair[0].0.y - pair[1].1.y, 1.0);
    }
    // Each inner end moves half the gap from the true boundary; the base does not move.
    assert_near(spans[0].1.y, base);
    assert_near(spans[0].0.y, base - 3.0 * unit + 0.5);
    assert_near(spans[1].1.y, base - 3.0 * unit - 0.5);
    assert_near(spans[1].0.y, base - 7.0 * unit + 0.5);
    assert_near(spans[2].1.y, base - 7.0 * unit - 0.5);
}

#[test]
fn a_segment_thinner_than_the_gap_disappears_without_moving_its_neighbours() {
    // About 9 pt per unit here, so 0.05 is under half a point.
    let (spans, _, base, unit) = one_stack(Theme::graphite(), &[&[10.0], &[0.05], &[10.0]], 20.05);
    assert!(0.05 * unit < 1.0, "{unit}");
    assert_eq!(spans.len(), 2);
    assert_near(spans[0].0.y, base - 10.0 * unit + 0.5);
    assert_near(spans[1].1.y, base - 10.05 * unit - 0.5);
    assert_near(spans[1].0.y, base - 20.05 * unit);
}

#[test]
fn the_outermost_segment_keeps_its_rounding() {
    // Midnight rounds bar ends by 3.5 pt; the inner ends stay square.
    let (spans, curves, base, unit) =
        one_stack(Theme::midnight(), &[&[3.0], &[4.0], &[-2.0], &[-1.0]], 7.0);
    assert_eq!(curves, [0, 2, 0, 2]);
    // The rounded ends lie at the true stack ends, not inset.
    assert_near(spans[1].0.y, base - 7.0 * unit);
    assert_near(spans[3].1.y, base + 3.0 * unit);
}

#[test]
fn opposite_signs_meet_at_the_baseline() {
    let (spans, _, base, unit) = one_stack(Theme::graphite(), &[&[3.0], &[-2.0]], 3.0);
    assert_eq!(spans.len(), 2);
    // Both start on the baseline: no gap at 0.
    assert_near(spans[0].1.y, base);
    assert_near(spans[1].0.y, base);
    assert_near(spans[0].0.y, base - 3.0 * unit);
    assert_near(spans[1].1.y, base + 2.0 * unit);
}
