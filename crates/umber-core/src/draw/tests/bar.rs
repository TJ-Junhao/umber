//! Bar marks: band share, base and end, clipping in axis space and the rounded end.

use super::mark_group;
use crate::geometry::{Path, PathVerb, Point};
use crate::layout::Layout;
use crate::model::{AxisData, BarOptions, Figure, LineOptions};
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
    crate::pipeline::plan_figure(figure, FontLibrary::global()).1
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
    let scene = f.to_scene(FontLibrary::global());
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
        assert_near(base, layout.baseline_y);
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
    let scene = f.to_scene(FontLibrary::global());
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
    let scene = f.to_scene(FontLibrary::global());
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
    let scene = f.to_scene(FontLibrary::global());
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
    f.axes_mut().unwrap().ylim(0.0, 10.0).unwrap();
    let scene = f.to_scene(FontLibrary::global());
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
    let scene = f.to_scene(FontLibrary::global());
    let layout = layout_of(&f);
    let fills = bars(&scene);
    let [(path, _)] = fills.as_slice() else {
        panic!("expected one bar, got {fills:?}")
    };
    assert_eq!(cubics(path), 2);
    // The base sits on the baseline, at the top of the plot (the domain ends at 0), and the
    // bar grows downwards; its square corners are at the base.
    let (lo, hi) = bounds(path);
    assert_near(lo.y, layout.baseline_y);
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
        f.axes_mut().unwrap().ylim(1e-11, 1e-10).unwrap();
        let scene = f.to_scene(FontLibrary::global());
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
    f.axes_mut().unwrap().ylim(1.0, 10.0).unwrap();
    let scene = f.to_scene(FontLibrary::global());
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
fn log_bars_grow_from_the_bottom_of_the_plot() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let f = bar_figure(Theme::graphite(), log, &["a", "b"], &[10.0, 1000.0]);
    let scene = f.to_scene(FontLibrary::global());
    let layout = layout_of(&f);
    let fills = bars(&scene);
    assert_eq!(fills.len(), 2);
    for (path, _) in &fills {
        assert_near(bounds(path).1.y, layout.plot.bottom());
    }
    // The baseline is the bottom of the plot too.
    assert_near(layout.plot.bottom() as f32, layout.baseline_y);
}
