//! Histogram marks: one fill for all bins, the gap, square corners, the gradient anchored at the
//! tallest bin and the baseline, the top edges, and clipping in axis space.

use super::{mark_group, subpaths};
use crate::geometry::{Path, PathVerb};
use crate::layout::Layout;
use crate::model::{Figure, HistOptions};
use crate::render::{SvgText, render_png, render_svg};
use crate::resolve::{NormalizedPos, Resolved};
use crate::scene::{Item, LinearGradient, Paint};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FillStyle, FontLibrary, LogBase, MAX_BINS, Rgba, ScaleKind, Theme};

/// Linear x axes, a y axis of `y` and one histogram of `values` in `bins` bins.
fn hist_figure(theme: Theme, y: ScaleKind, values: &[f64], bins: usize) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    let options = HistOptions {
        bins: bins.try_into().unwrap(),
        ..HistOptions::default()
    };
    f.add_axes(ScaleKind::Linear, y)
        .unwrap()
        .hist(values, options)
        .unwrap();
    f
}

fn plan(figure: &Figure) -> (Resolved, Layout) {
    crate::pipeline::plan_figure(figure, FontLibrary::global())
}

/// The items of the marks group of `scene`.
fn mark_items(scene: &crate::scene::Scene) -> &[Item] {
    mark_group(scene).1
}

/// The one fill of a histogram drawn alone: its path and paint.
fn the_fill(items: &[Item]) -> (&Path, &Paint) {
    match items.first() {
        Some(Item::Fill { path, paint, .. }) => (path, paint),
        other => panic!("expected the histogram fill, got {other:?}"),
    }
}

/// The bins of a fill path, in order: each as (left, right, top, base) in plot points.
fn bins(path: &Path) -> Vec<(f32, f32, f32, f32)> {
    subpaths(path)
        .iter()
        .map(|points| {
            let xs = points.iter().map(|p| p.x);
            let ys = points.iter().map(|p| p.y);
            (
                xs.clone().fold(f32::INFINITY, f32::min),
                xs.fold(f32::NEG_INFINITY, f32::max),
                ys.clone().fold(f32::INFINITY, f32::min),
                ys.fold(f32::NEG_INFINITY, f32::max),
            )
        })
        .collect()
}

/// The plot x of the data value `v` on the resolved x axis.
fn x_of(resolved: &Resolved, layout: &Layout, v: f64) -> f64 {
    let scale = resolved.x.continuous().unwrap();
    layout.plot.x_at(NormalizedPos::new(scale.normalize(v)))
}

/// The plot y of the data value `v` on the resolved y axis.
fn y_of(resolved: &Resolved, layout: &Layout, v: f64) -> f64 {
    let scale = &resolved.y.scale;
    layout.plot.y_at(NormalizedPos::new(scale.normalize(v)))
}

#[track_caller]
fn assert_near(actual: f32, expected: f64) {
    assert!(
        (f64::from(actual) - expected).abs() < 1e-3,
        "{actual} != {expected}"
    );
}

/// Values 0..4 in four bins, counted 1, 0, 3, 5: the second bin is empty.
const VALUES: [f64; 9] = [0.0, 2.0, 2.5, 2.9, 3.0, 3.2, 3.5, 3.9, 4.0];

#[test]
fn zero_count_bins_draw_nothing() {
    // Graphite has no gradient: one solid fill for every bin, and nothing else.
    let theme = Theme::graphite();
    let f = hist_figure(theme.clone(), ScaleKind::Linear, &VALUES, 4);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let items = mark_items(&scene);
    assert_eq!(items.len(), 1);
    let (path, paint) = the_fill(items);
    assert_eq!(paint, &Paint::Solid(theme.palette()[0]));
    // Three bins hold values; the empty one draws nothing.
    let (resolved, layout) = plan(&f);
    let drawn = bins(path);
    assert_eq!(drawn.len(), 3);
    let gap = f64::from(theme.hist_gap());
    for (&(left, right, top, base), (i, count)) in drawn.iter().zip([(0, 1.0), (2, 3.0), (3, 5.0)])
    {
        // Each bin spans its edges, inset by half the gap on each side, from 0 to its count.
        let i = f64::from(i);
        assert_near(left, x_of(&resolved, &layout, i) + gap / 2.0);
        assert_near(right, x_of(&resolved, &layout, i + 1.0) - gap / 2.0);
        assert_near(top, y_of(&resolved, &layout, count));
        assert_near(base, layout.baseline_y);
    }
}

#[test]
fn bins_have_square_corners_and_start_on_the_baseline() {
    // Midnight rounds bar ends by 3.5 pt, but histogram bins stay square; with 0 on the lower
    // plot edge, no bin reaches below it.
    let f = hist_figure(Theme::midnight(), ScaleKind::Linear, &VALUES, 4);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let (path, _) = the_fill(mark_items(&scene));
    assert!(
        path.verbs()
            .iter()
            .all(|v| !matches!(v, PathVerb::CubicTo(..) | PathVerb::QuadTo(..)))
    );
    let (_, layout) = plan(&f);
    assert_near(layout.plot.bottom() as f32, layout.baseline_y);
    let (_, hi) = path.bounds().unwrap();
    assert_near(hi.y, layout.baseline_y);
}

#[test]
fn the_gap_is_at_most_half_a_bin() {
    // MAX_BINS bins across about 400 pt are far narrower than the 1.5 pt gap: the gap shrinks
    // to half of each bin, which keeps the other half.
    let theme = Theme::midnight().with_fill_style(FillStyle::Solid).unwrap();
    let values: Vec<f64> = (0..=MAX_BINS).map(|i| i as f64).collect();
    let f = hist_figure(theme, ScaleKind::Linear, &values, MAX_BINS);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let items = mark_items(&scene);
    assert_eq!(items.len(), 1);
    let (resolved, layout) = plan(&f);
    let drawn = bins(the_fill(items).0);
    assert_eq!(drawn.len(), MAX_BINS);
    let bin = x_of(&resolved, &layout, 1.0) - x_of(&resolved, &layout, 0.0);
    assert!(bin < 1.5, "the case must cap the gap");
    for &(left, right, _, _) in &drawn[..5] {
        assert!(
            (f64::from(right - left) - bin / 2.0).abs() < 1e-3,
            "{left} {right}"
        );
    }
}

/// The gradient of the histogram fill.
fn gradient(paint: &Paint) -> &LinearGradient {
    match paint {
        Paint::Linear(gradient) => gradient,
        other => panic!("expected a gradient, got {other:?}"),
    }
}

#[test]
fn gradient_runs_top_to_baseline() {
    let theme = Theme::midnight();
    let color = theme.palette()[0];
    let f = hist_figure(theme, ScaleKind::Linear, &VALUES, 4);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let (resolved, layout) = plan(&f);
    let items = mark_items(&scene);
    let (path, paint) = the_fill(items);
    let gradient = gradient(paint);
    // One gradient for the whole histogram: from the top of the tallest bin, at 0.95 of the
    // color's opacity, to the baseline, transparent.
    let tallest = y_of(&resolved, &layout, 5.0);
    assert_near(gradient.start.y, tallest);
    assert_near(gradient.end.y, layout.baseline_y);
    assert_eq!(gradient.start.x, gradient.end.x);
    let alphas: Vec<(f32, u8)> = gradient
        .stops
        .iter()
        .map(|stop| (stop.offset, stop.color.a))
        .collect();
    assert_eq!(alphas, [(0.0, 242), (1.0, 0)]);
    assert!(gradient.stops.iter().all(|stop| Rgba {
        a: 255,
        ..stop.color
    } == color));
    // A shorter bin's top lies partway down the gradient, so it starts dimmer.
    let drawn = bins(path);
    let short_top = f64::from(drawn[0].2);
    let share =
        (short_top - f64::from(gradient.start.y)) / f64::from(gradient.end.y - gradient.start.y);
    assert!(share > 0.5 && share < 1.0, "{share}");
}

/// The top edges of a histogram drawn alone under a gradient: one solid fill and its color.
fn top_edges(items: &[Item]) -> (&Path, Rgba) {
    match items {
        [
            Item::Fill { .. },
            Item::Fill {
                path,
                paint: Paint::Solid(color),
                ..
            },
        ] => (path, *color),
        other => panic!("expected a fill and its top edges, got {other:?}"),
    }
}

#[test]
fn top_edges_follow_counts() {
    let theme = Theme::midnight();
    let f = hist_figure(theme.clone(), ScaleKind::Linear, &VALUES, 4);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let items = mark_items(&scene);
    let drawn = bins(the_fill(items).0);
    let (path, color) = top_edges(items);
    // A 1.6 pt band in the solid color inside each bin: its outer edge is the bin's top, at the
    // count, never above it; it is exactly as wide as the bin.
    assert_eq!(color, theme.palette()[0]);
    let edges = bins(path);
    assert_eq!(edges.len(), drawn.len());
    for (&(left, right, top, bottom), &(bin_left, bin_right, bin_top, _)) in
        edges.iter().zip(&drawn)
    {
        assert_eq!((left, right, top), (bin_left, bin_right, bin_top));
        assert_near(bottom, f64::from(top) + 1.6);
    }
}

#[test]
fn top_edges_of_short_bins_stop_at_the_baseline() {
    // Counts 1 and 1000: the short bin is far less than 1.6 pt high, so its edge fills it from
    // its top to the baseline and paints nothing beyond.
    let mut values = vec![0.0];
    values.extend(std::iter::repeat_n(1.0, 1000));
    let f = hist_figure(Theme::midnight(), ScaleKind::Linear, &values, 2);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let layout = plan(&f).1;
    let items = mark_items(&scene);
    let drawn = bins(the_fill(items).0);
    let edges = bins(top_edges(items).0);
    let (short, short_edge) = (drawn[0], edges[0]);
    assert!(f64::from(short.3 - short.2) < 1.6, "{short:?}");
    assert_eq!(short_edge, short);
    assert_near(short_edge.3, layout.baseline_y);
    // The tall bin keeps the full 1.6 pt edge.
    assert_near(edges[1].3, f64::from(drawn[1].2) + 1.6);
}

#[test]
fn cropped_limits_keep_the_gradient_opacity() {
    // One bin of count 100 under limits 50..60: the gradient is clipped to the plot, but each
    // visible count keeps the opacity of the full ramp from 100 (0.95) down to 0 (none).
    let mut f = hist_figure(Theme::midnight(), ScaleKind::Linear, &[0.0; 100], 1);
    f.axes_mut().unwrap().ylim(50.0, 60.0).unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let (resolved, layout) = plan(&f);
    let (_, paint) = the_fill(mark_items(&scene));
    let gradient = gradient(paint);
    let full = f64::from(Theme::midnight().palette()[0].a) * 0.95;
    // The opacity the gradient paints at plot y `at`, interpolated between its stops.
    let alpha_at = |at: f64| {
        let (y0, y1) = (f64::from(gradient.start.y), f64::from(gradient.end.y));
        let t = ((at - y0) / (y1 - y0)).clamp(0.0, 1.0);
        let (a0, a1) = (
            f64::from(gradient.stops[0].color.a),
            f64::from(gradient.stops[1].color.a),
        );
        a0 + (a1 - a0) * t
    };
    for count in [50.0, 52.5, 55.0, 60.0] {
        let alpha = alpha_at(y_of(&resolved, &layout, count));
        let expected = full * count / 100.0;
        assert!(
            (alpha - expected).abs() < 1.5,
            "{count}: {alpha} != {expected}"
        );
    }
    // Both ends lie within the clip margin of the plot.
    assert!(f64::from(gradient.start.y) >= layout.plot.y - 2.0 - 1e-3);
    assert!(f64::from(gradient.end.y) <= layout.plot.bottom() + 2.0 + 1e-3);
}

#[test]
fn one_gradient_item_per_histogram() {
    // However many bins: one gradient fill holding all of them, and one solid fill holding
    // their top edges.
    for bins_count in [1, 30, MAX_BINS] {
        let values: Vec<f64> = (0..2 * bins_count)
            .map(|i| (i % 7) as f64 * 1.5 + i as f64)
            .collect();
        let f = hist_figure(Theme::midnight(), ScaleKind::Linear, &values, bins_count);
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        assert_scene_bounded(&scene);
        let items = mark_items(&scene);
        let (edges, _) = top_edges(items);
        let (fill, paint) = the_fill(items);
        gradient(paint);
        let drawn = bins(fill).len();
        assert!(drawn > 0 && drawn <= bins_count);
        assert_eq!(subpaths(edges).len(), drawn);
    }
}

#[test]
fn log_y_bins_grow_from_the_baseline() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let f = hist_figure(Theme::midnight(), log, &VALUES, 4);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let (resolved, layout) = plan(&f);
    // The counts 1, 0, 3, 5 would take the nice limits 1..10, but the smallest count lands on
    // the lowest tick, so the axis starts a decade lower, at 0.1: the baseline sits there,
    // above the padded plot bottom and below every count.
    assert_eq!(resolved.y.ticks[0].label, "0.1");
    assert_near(layout.baseline_y as f32, y_of(&resolved, &layout, 0.1));
    assert!(layout.baseline_y < layout.plot.bottom() - 1.0, "{layout:?}");
    let items = mark_items(&scene);
    let (path, paint) = the_fill(items);
    let drawn = bins(path);
    // The empty bin draws nothing; the bin of count 1 still shows, from the baseline.
    assert_eq!(drawn.len(), 3);
    for (&(_, _, top, base), count) in drawn.iter().zip([1.0, 3.0, 5.0]) {
        assert_near(base, layout.baseline_y);
        assert_near(top, y_of(&resolved, &layout, count));
        assert!(f64::from(top) < layout.baseline_y - 1.0, "{top}");
    }
    let gradient = gradient(paint);
    assert_near(gradient.end.y, layout.baseline_y);
    assert_near(gradient.start.y, y_of(&resolved, &layout, 5.0));
}

#[test]
fn hist_far_outside_limits_is_clipped() {
    // Values at the largest magnitudes with limits 1e-10 wide on both axes: mapping the raw bin
    // edges or counts would overflow, so every bin is clipped in axis space first.
    let values = [-1e300, -1e300, 1e300, 0.0, 5e-11, 5e-11];
    for theme in [Theme::pigment(), Theme::midnight()] {
        let mut f = hist_figure(theme, ScaleKind::Linear, &values, 3);
        f.axes_mut()
            .unwrap()
            .xlim(1e-11, 1e-10)
            .unwrap()
            .ylim(1e-11, 1e-10)
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        assert_scene_bounded(&scene);
        let plot = plan(&f).1.plot;
        let items = mark_items(&scene);
        let (path, paint) = the_fill(items);
        // The middle bin, which holds 0 and 5e-11, covers the whole plot and is cut 2 pt
        // beyond each side; the outer bins lie wholly outside the x limits.
        let drawn = bins(path);
        assert_eq!(drawn.len(), 1);
        let (left, right, top, base) = drawn[0];
        assert_near(left, plot.x - 2.0);
        assert_near(right, plot.x + plot.width + 2.0);
        assert_near(top, plot.y - 2.0);
        assert_near(base, plot.bottom() + 2.0);
        if let Paint::Linear(gradient) = paint {
            // The gradient is clipped as the bins are, and its top edge is never drawn.
            assert_near(gradient.start.y, plot.y - 2.0);
            assert_near(gradient.end.y, plot.bottom() + 2.0);
            assert_eq!(items.len(), 1);
        }
        assert!(render_png(&scene, 72).is_ok());
        assert!(!render_svg(&scene, SvgText::Outlines).is_empty());
    }
}

#[test]
fn a_histogram_without_finite_values_draws_nothing() {
    for values in [&[][..], &[f64::NAN][..]] {
        let f = hist_figure(Theme::midnight(), ScaleKind::Linear, values, 30);
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        // Only the chrome and the baseline: no marks group.
        assert_eq!(scene.items.len(), 2, "{:?}", scene.items);
    }
}

#[test]
fn hist_reads_the_fill_style() {
    // The fill style alone decides how bins are painted, in every theme: `Solid` gives flat
    // bins in the series color and no top edges; `Gradient` gives the gradient fill and the
    // solid top edges. Each theme's own style is the one it had as an area fill setting
    // (Pigment and Graphite flat, Midnight gradient), so its histograms are unchanged; the
    // output gate pins their bytes.
    for (theme, own) in [
        (Theme::pigment(), FillStyle::Solid),
        (Theme::graphite(), FillStyle::Solid),
        (Theme::midnight(), FillStyle::Gradient),
    ] {
        assert_eq!(theme.fill_style(), own);
        let color = theme.palette()[0];
        let scene_of = |style: FillStyle| {
            let theme = theme.with_fill_style(style).unwrap();
            hist_figure(theme, ScaleKind::Linear, &VALUES, 4)
                .to_scene(FontLibrary::global())
                .unwrap()
        };
        let own_scene = hist_figure(theme.clone(), ScaleKind::Linear, &VALUES, 4)
            .to_scene(FontLibrary::global())
            .unwrap();
        assert_eq!(scene_of(own), own_scene);

        let solid = scene_of(FillStyle::Solid);
        let items = mark_items(&solid);
        assert_eq!(items.len(), 1);
        assert_eq!(the_fill(items).1, &Paint::Solid(color));

        let gradient_scene = scene_of(FillStyle::Gradient);
        let items = mark_items(&gradient_scene);
        assert_eq!(items.len(), 2);
        let (fill, paint) = the_fill(items);
        gradient(paint);
        let (edges, edge_color) = top_edges(items);
        assert_eq!(edge_color, color);
        assert_eq!(subpaths(edges).len(), bins(fill).len());
        // The bins themselves are the same under either style.
        assert_eq!(bins(the_fill(mark_items(&solid)).0), bins(fill));
    }
}
