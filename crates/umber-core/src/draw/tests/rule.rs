//! Reference lines and spans: the layers, the geometry in plot points, and the edges of the
//! domain, where a value equal to a limit, a span that crosses a limit and a span that covers the
//! whole view each have a defined result.

use super::{mark_group, nums, plot_of, subpaths};
use crate::geometry::PathVerb;
use crate::layout::Area;
use crate::model::{AxisValue, Figure, LineOptions, RuleOptions, SpanOptions};
use crate::model::{Grid, GridIndex, Placement, Share};
use crate::scene::{FillRule, Item, LineCap, Paint, Rect, Scene};
use crate::test_support::scene::assert_scene_bounded;
use crate::{Dash, FontLibrary, LogBase, ScaleKind, SizeSpec, Theme};

/// Linear axes limited to `0..=10` on both, holding nothing else.
fn figure() -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap();
    f
}

fn scene_of(figure: &Figure) -> Scene {
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    scene
}

/// The items of the marks group of `figure`; empty when nothing is visible.
fn items(figure: &Figure) -> Vec<Item> {
    let scene = scene_of(figure);
    flatten(super::mark_items(&scene))
}

/// `items` with the reference layers' own clipped groups opened up.
fn flatten(items: &[Item]) -> Vec<Item> {
    items
        .iter()
        .flat_map(|item| match item {
            Item::Group { items, .. } => flatten(items),
            other => vec![other.clone()],
        })
        .collect()
}

/// The clips of the groups nested in the marks group of `figure`.
fn layer_clips(figure: &Figure) -> Vec<Rect> {
    let scene = scene_of(figure);
    mark_group(&scene)
        .1
        .iter()
        .filter_map(|item| match item {
            Item::Group { clip, .. } => *clip,
            _ => None,
        })
        .collect()
}

/// The plot y of the y value `v` on `0..=10`.
fn y_at(plot: Area, v: f64) -> f64 {
    plot.bottom() - v / 10.0 * plot.height
}

/// The plot x of the x value `v` on `0..=10`.
fn x_at(plot: Area, v: f64) -> f64 {
    plot.x + v / 10.0 * plot.width
}

/// The corners of a filled item, as `(x, y)`.
fn corners(item: &Item) -> Vec<(f64, f64)> {
    let Item::Fill { path, .. } = item else {
        panic!("a fill expected, got {item:?}")
    };
    path.verbs()
        .iter()
        .filter_map(|verb| match *verb {
            PathVerb::MoveTo(p) | PathVerb::LineTo(p) => Some((f64::from(p.x), f64::from(p.y))),
            _ => None,
        })
        .collect()
}

/// The `(left, top, right, bottom)` of the rectangle a span fills.
fn rect(item: &Item) -> (f64, f64, f64, f64) {
    let points = corners(item);
    assert_eq!(points.len(), 4, "{points:?}");
    let min = |f: fn(&(f64, f64)) -> f64| points.iter().map(f).fold(f64::INFINITY, f64::min);
    let max = |f: fn(&(f64, f64)) -> f64| points.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
    (min(|p| p.0), min(|p| p.1), max(|p| p.0), max(|p| p.1))
}

#[track_caller]
fn assert_near(got: f64, expected: f64) {
    assert!((got - expected).abs() < 1e-3, "{got} vs {expected}");
}

/// The end points of a rule's stroke.
fn ends(item: &Item) -> ((f64, f64), (f64, f64)) {
    let Item::Stroke { path, .. } = item else {
        panic!("a stroke expected, got {item:?}")
    };
    let [part] = subpaths(path).try_into().unwrap();
    let [a, b] = part.as_slice() else {
        panic!("two points expected, got {part:?}")
    };
    (
        (f64::from(a.x), f64::from(a.y)),
        (f64::from(b.x), f64::from(b.y)),
    )
}

#[test]
fn spans_draw_below_data_and_rules_above() {
    // Called in the order rule, line, span, scatter-free: the layers do not follow it.
    let mut f = figure();
    let rule = RuleOptions {
        color: crate::ColorSpec::Palette {
            index: 3,
            alpha: crate::Alpha::new(1.0).unwrap(),
        },
        ..RuleOptions::default()
    };
    f.single_axes_mut()
        .hline(5.0, rule.clone())
        .unwrap()
        .line(nums(&[0.0, 10.0]), &[1.0, 9.0], LineOptions::default())
        .unwrap()
        .vspan(2.0, 4.0, SpanOptions::default())
        .unwrap()
        .vline(AxisValue::Number(7.0), RuleOptions::default())
        .unwrap()
        .hspan(6.0, 8.0, SpanOptions::default())
        .unwrap();
    let items = items(&f);
    let kinds: Vec<&str> = items
        .iter()
        .map(|item| match item {
            Item::Fill { .. } => "span",
            Item::Stroke { style, .. } if style.cap == LineCap::Butt => "rule",
            Item::Stroke { .. } => "line",
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["span", "span", "line", "rule", "rule"]);
    // Within a layer, the order of the calls: the vspan first, then the hspan; the hline first.
    let plot = plot_of(&f);
    assert_near(rect(&items[0]).0, x_at(plot, 2.0));
    assert_near(rect(&items[1]).1, y_at(plot, 8.0));
    assert_near(ends(&items[3]).0.1, y_at(plot, 5.0));
    assert_near(ends(&items[4]).0.0, x_at(plot, 7.0));
}

#[test]
fn rules_run_across_the_whole_plot_and_spans_fill_it_at_the_area_alpha() {
    let mut f = figure();
    f.single_axes_mut()
        .hline(4.0, RuleOptions::default())
        .unwrap()
        .vline(AxisValue::Number(6.0), RuleOptions::default())
        .unwrap()
        .hspan(2.0, 3.0, SpanOptions::default())
        .unwrap();
    let plot = plot_of(&f);
    let theme = Theme::pigment();
    let items = items(&f);
    let (left, top, right, bottom) = rect(&items[0]);
    assert_near(left, plot.x);
    assert_near(right, plot.right());
    assert_near(top, y_at(plot, 3.0));
    assert_near(bottom, y_at(plot, 2.0));
    // The first color of the palette, at 0.32 of its opacity, flat.
    let Item::Fill { paint, rule, .. } = &items[0] else {
        panic!()
    };
    assert_eq!(*rule, FillRule::NonZero);
    assert_eq!(
        *paint,
        Paint::Solid(theme.palette()[2].scale_alpha(0.32)),
        "the span is the third auto-colored mark"
    );
    let ((x0, y0), (x1, y1)) = ends(&items[1]);
    assert_near(x0, plot.x);
    assert_near(x1, plot.right());
    assert_near(y0, y_at(plot, 4.0));
    assert_near(y1, y0);
    let ((x0, y0), (x1, y1)) = ends(&items[2]);
    assert_near(y0, plot.y);
    assert_near(y1, plot.bottom());
    assert_near(x0, x_at(plot, 6.0));
    assert_near(x1, x0);
    // Strokes are as wide as the theme's line, solid ones end square.
    let Item::Stroke { style, .. } = &items[1] else {
        panic!()
    };
    assert_eq!(style.width, theme.line_width());
    assert_eq!((style.cap, style.dash.is_empty()), (LineCap::Butt, true));
}

#[test]
fn dashes_scale_with_the_width_as_on_a_line() {
    let mut f = figure();
    f.single_axes_mut()
        .hline(
            4.0,
            RuleOptions {
                dash: Dash::Dashed,
                width: crate::SizeSpec::times(2.0).unwrap(),
                ..RuleOptions::default()
            },
        )
        .unwrap()
        .vline(
            AxisValue::Number(4.0),
            RuleOptions {
                dash: Dash::Dotted,
                ..RuleOptions::default()
            },
        )
        .unwrap();
    let items = items(&f);
    let [
        Item::Stroke { style: dashed, .. },
        Item::Stroke { style: dotted, .. },
    ] = items.as_slice()
    else {
        panic!("{items:?}")
    };
    let w = 2.0 * Theme::pigment().line_width();
    assert_eq!(dashed.width, w);
    assert_eq!(dashed.dash, [4.0 * w, 3.0 * w]);
    assert_eq!(dotted.cap, LineCap::Round);
    assert_eq!(dotted.dash[0], 0.0);
}

#[test]
fn a_rule_at_a_limit_is_drawn_and_one_past_the_stroke_is_not() {
    let width = f64::from(Theme::pigment().line_width());
    let plot_height = plot_of(&figure()).height;
    // Half a stroke in axis units (the domain is 0..=10 over the plot's height).
    let half = width / 2.0 * 10.0 / plot_height;
    let hline = |y: f64| {
        let mut f = figure();
        f.single_axes_mut()
            .hline(y, RuleOptions::default())
            .unwrap();
        items(&f).len()
    };
    assert_eq!(hline(10.0), 1, "equal to the high limit");
    assert_eq!(hline(0.0), 1, "equal to the low limit");
    assert_eq!(hline(10.0 + half * 0.9), 1, "its inner half still shows");
    assert_eq!(hline(10.0 + half * 1.1), 0);
    assert_eq!(hline(-half * 0.9), 1);
    assert_eq!(hline(-half * 1.1), 0);
    assert_eq!(hline(1e299), 0, "far outside, and nothing overflows");
    let plot_width = plot_of(&figure()).width;
    let half = width / 2.0 * 10.0 / plot_width;
    let vline = |x: f64| {
        let mut f = figure();
        f.single_axes_mut()
            .vline(AxisValue::Number(x), RuleOptions::default())
            .unwrap();
        items(&f).len()
    };
    assert_eq!(vline(10.0), 1);
    assert_eq!(vline(0.0), 1);
    assert_eq!(vline(10.0 + half * 1.1), 0);
    assert_eq!(vline(-half * 1.1), 0);
}

#[test]
fn a_span_is_clipped_at_the_limits() {
    let plot = plot_of(&figure());
    let span = |low: f64, high: f64| {
        let mut f = figure();
        f.single_axes_mut()
            .hspan(low, high, SpanOptions::default())
            .unwrap();
        items(&f)
    };
    // Inside: exactly its values.
    let inside = span(2.0, 3.0);
    assert_near(rect(&inside[0]).1, y_at(plot, 3.0));
    assert_near(rect(&inside[0]).3, y_at(plot, 2.0));
    // Crossing the high limit, crossing the low limit.
    let (_, top, _, bottom) = rect(&span(5.0, 15.0)[0]);
    assert_near(top, plot.y);
    assert_near(bottom, y_at(plot, 5.0));
    let (_, top, _, bottom) = rect(&span(-5.0, 4.0)[0]);
    assert_near(top, y_at(plot, 4.0));
    assert_near(bottom, plot.bottom());
    // Covering the whole view: the plot area.
    let all = span(-5.0, 15.0);
    let (left, top, right, bottom) = rect(&all[0]);
    assert_eq!(all.len(), 1);
    assert_near(left, plot.x);
    assert_near(right, plot.right());
    assert_near(top, plot.y);
    assert_near(bottom, plot.bottom());
    // A span that ends at the domain's end, or lies outside it, or only touches it, is nothing.
    assert_eq!(span(0.0, 10.0).len(), 1, "exactly the view");
    assert_eq!(span(10.0, 20.0).len(), 0, "starts at the high limit");
    assert_eq!(span(-5.0, 0.0).len(), 0, "ends at the low limit");
    assert_eq!(span(11.0, 12.0).len(), 0);
    assert_eq!(span(-3.0, -2.0).len(), 0);
    // The same along x.
    let vspan = |low: f64, high: f64| {
        let mut f = figure();
        f.single_axes_mut()
            .vspan(low, high, SpanOptions::default())
            .unwrap();
        items(&f)
    };
    let (left, top, right, bottom) = rect(&vspan(8.0, 12.0)[0]);
    assert_near(left, x_at(plot, 8.0));
    assert_near(right, plot.right());
    assert_near(top, plot.y);
    assert_near(bottom, plot.bottom());
    assert_eq!(vspan(10.0, 11.0).len(), 0);
    assert_eq!(vspan(-1.0, 0.0).len(), 0);
    assert_eq!(vspan(1e299, 1e300).len(), 0);
}

#[test]
fn a_vline_on_a_category_axis_sits_at_the_band_centre() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let categories = |names: &[&str]| {
        crate::model::AxisData::Categories(names.iter().map(|n| (*n).to_owned()).collect())
    };
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .scatter(
            categories(&["Q1", "Q2", "Q3", "Q4"]),
            &[1.0, 2.0, 3.0, 4.0],
            crate::model::ScatterOptions::default(),
        )
        .unwrap()
        .vline(AxisValue::Category("Q3".into()), RuleOptions::default())
        .unwrap()
        .vline(AxisValue::Category("Q1".into()), RuleOptions::default())
        .unwrap();
    let plot = plot_of(&f);
    let items = items(&f);
    let rules: Vec<f64> = items
        .iter()
        .filter(|item| matches!(item, Item::Stroke { .. }))
        .map(|item| ends(item).0.0)
        .collect();
    assert_eq!(rules.len(), 2);
    assert_near(rules[0], plot.x + 2.5 / 4.0 * plot.width);
    assert_near(rules[1], plot.x + 0.5 / 4.0 * plot.width);
}

#[test]
fn rules_and_spans_on_log_axes_are_placed_in_log_space() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let log = ScaleKind::Log { base: LogBase::TEN };
    f.add_axes(log, log)
        .unwrap()
        .xlim(1.0, 1000.0)
        .unwrap()
        .ylim(1.0, 100.0)
        .unwrap()
        .hline(10.0, RuleOptions::default())
        .unwrap()
        .vspan(10.0, 100.0, SpanOptions::default())
        .unwrap();
    let plot = plot_of(&f);
    let items = items(&f);
    let (left, _, right, _) = rect(&items[0]);
    assert_near(left, plot.x + plot.width / 3.0);
    assert_near(right, plot.x + 2.0 * plot.width / 3.0);
    assert_near(ends(&items[1]).0.1, plot.y + plot.height / 2.0);
}

#[test]
fn references_alone_draw_no_data_marks_and_nothing_visible_draws_no_group() {
    let mut f = figure();
    f.single_axes_mut()
        .hline(50.0, RuleOptions::default())
        .unwrap()
        .vspan(20.0, 30.0, SpanOptions::default())
        .unwrap();
    assert!(items(&f).is_empty());
}

/// A line chart over the y values 33 to 68 with every reference mark of the kind: auto y limits,
/// so the baseline sits at the padded lower limit, on the plot bottom.
fn lower_limit_figure(ylim: Option<(f64, f64)>) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes.line(
        nums(&[0.0, 30.0, 60.0]),
        &[33.0, 50.0, 68.0],
        LineOptions::default(),
    )
    .unwrap()
    .vline(AxisValue::Number(31.0), RuleOptions::default())
    .unwrap()
    .vspan(40.0, 50.0, SpanOptions::default())
    .unwrap();
    if let Some((low, high)) = ylim {
        axes.ylim(low, high).unwrap();
    }
    f
}

fn baseline_y_of(figure: &Figure) -> (Area, f64) {
    let layout = crate::pipeline::plan_figure(figure, (0, 0), FontLibrary::global()).1;
    (layout.plot, layout.baseline_y())
}

#[test]
fn vertical_references_end_at_a_lower_limit_baseline() {
    let f = lower_limit_figure(None);
    let (plot, baseline) = baseline_y_of(&f);
    assert_near(baseline, plot.bottom());
    let items = items(&f);
    let (_, top, _, bottom) = rect(&items[0]);
    assert_near(top, plot.y);
    assert_near(bottom, baseline);
    let ((_, y0), (_, y1)) = ends(items.last().unwrap());
    assert_near(y0, plot.y);
    assert_near(y1, baseline);
}

#[test]
fn vertical_references_keep_the_full_height_when_zero_is_in_the_domain() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(
            nums(&[0.0, 30.0, 60.0]),
            &[-10.0, 20.0, 68.0],
            LineOptions::default(),
        )
        .unwrap()
        .vline(AxisValue::Number(31.0), RuleOptions::default())
        .unwrap()
        .vspan(40.0, 50.0, SpanOptions::default())
        .unwrap();
    let (plot, baseline) = baseline_y_of(&f);
    assert!(
        baseline < plot.bottom() - 1.0,
        "zero is mid-plot: {baseline}"
    );
    let items = items(&f);
    assert_near(rect(&items[0]).3, plot.bottom());
    assert_near(ends(items.last().unwrap()).1.1, plot.bottom());
    assert!(layer_clips(&f).is_empty(), "no cut at the baseline");
}

#[test]
fn vertical_references_fill_an_explicit_ylim_and_end_at_a_log_lower_limit() {
    // Explicit limits have no padding: the baseline is the plot bottom.
    let f = lower_limit_figure(Some((20.0, 80.0)));
    let (plot, baseline) = baseline_y_of(&f);
    assert_near(baseline, plot.bottom());
    let drawn = items(&f);
    assert_near(rect(&drawn[0]).3, plot.bottom());
    assert_near(ends(drawn.last().unwrap()).1.1, plot.bottom());

    // A log y axis never holds 0, so its baseline is the lower limit before padding.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN })
        .unwrap()
        .line(
            nums(&[0.0, 30.0, 60.0]),
            &[33.0, 50.0, 680.0],
            LineOptions::default(),
        )
        .unwrap()
        .vline(AxisValue::Number(31.0), RuleOptions::default())
        .unwrap()
        .vspan(40.0, 50.0, SpanOptions::default())
        .unwrap();
    let (plot, baseline) = baseline_y_of(&f);
    assert!(baseline < plot.bottom() - 1.0, "{baseline} {plot:?}");
    let drawn = items(&f);
    assert_near(rect(&drawn[0]).3, baseline);
    assert_near(ends(drawn.last().unwrap()).1.1, baseline);
}

#[test]
fn horizontal_references_never_reach_below_a_lower_limit_baseline() {
    // Their values extend the automatic y domain, so the lower limit lies at or below them.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(
            nums(&[0.0, 30.0, 60.0]),
            &[33.0, 50.0, 68.0],
            LineOptions::default(),
        )
        .unwrap()
        .hspan(20.0, 40.0, SpanOptions::default())
        .unwrap()
        .hline(25.0, RuleOptions::default())
        .unwrap();
    let (plot, baseline) = baseline_y_of(&f);
    assert_near(baseline, plot.bottom());
    let drawn = items(&f);
    // The span's bottom is the value 20, the lowest value of the chart; the padded lower
    // limit lies below it.
    let bottom = rect(&drawn[0]).3;
    assert!(bottom <= baseline + 1e-3, "{bottom} vs {baseline}");
    let ((_, y0), (_, y1)) = ends(drawn.last().unwrap());
    assert!(y0 < baseline && y1 < baseline, "{y0} {y1} vs {baseline}");
}

#[test]
fn a_shared_y_grid_ends_every_panel_at_the_common_baseline() {
    let grid = Grid::new(1, 2).unwrap().with_share_y(Share::All);
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), grid).unwrap();
    // The panels' own data would give them different baselines; shared, both use the one
    // domain of the pair.
    for (col, top) in [(0, 68.0), (1, 40.0)] {
        let at = Placement {
            row: GridIndex::At(0),
            col: GridIndex::At(col),
            ..Placement::default()
        };
        f.add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                nums(&[0.0, 30.0, 60.0]),
                &[33.0, 50.0, top],
                LineOptions::default(),
            )
            .unwrap()
            .vline(AxisValue::Number(31.0), RuleOptions::default())
            .unwrap()
            .vspan(40.0, 50.0, SpanOptions::default())
            .unwrap();
    }
    let scene = scene_of(&f);
    let groups: Vec<Vec<Item>> = scene
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Group {
                clip: Some(_),
                items,
            } => Some(flatten(items)),
            _ => None,
        })
        .collect();
    assert_eq!(groups.len(), 2);
    for (col, items) in groups.into_iter().enumerate() {
        let (plot, baseline) = {
            let layout = crate::pipeline::plan_figure(&f, (0, col), FontLibrary::global()).1;
            (layout.plot, layout.baseline_y())
        };
        assert_near(baseline, plot.bottom());
        assert_near(rect(&items[0]).3, baseline);
        assert_near(ends(items.last().unwrap()).1.1, baseline);
    }
}

/// Log data 33 to 68 on y ticks 20 to 80: the baseline remains at the unpadded tick 20,
/// above the plot bottom, so references need an additional clip there.
fn ticked_figure(rule: RuleOptions, vertical: bool) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f
        .add_axes(ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN })
        .unwrap();
    axes.line(
        nums(&[0.0, 30.0, 60.0]),
        &[33.0, 50.0, 68.0],
        LineOptions::default(),
    )
    .unwrap()
    .yticks(&[20.0, 40.0, 60.0, 80.0])
    .unwrap();
    if vertical {
        axes.vline(AxisValue::Number(31.0), rule).unwrap();
    } else {
        axes.hline(20.0, rule).unwrap();
    }
    f
}

#[test]
fn a_thick_hline_on_the_baseline_is_cut_at_it() {
    let width = SizeSpec::pt(6.0).unwrap();
    let f = ticked_figure(
        RuleOptions {
            width,
            ..RuleOptions::default()
        },
        false,
    );
    let (plot, baseline) = baseline_y_of(&f);
    assert!(baseline < plot.bottom() - 1.0, "{baseline} {plot:?}");
    let drawn = items(&f);
    let ((_, y), _) = ends(drawn.last().unwrap());
    assert_near(y, baseline);
    // The lower half of the 6 pt stroke would reach 3 pt below the baseline: the layer's clip
    // is what stops it.
    let [clip] = layer_clips(&f).try_into().unwrap();
    assert_near(f64::from(clip.y + clip.height), baseline);
    assert_near(f64::from(clip.y), plot.y);
}

#[test]
fn a_dotted_vline_cap_is_cut_at_the_baseline() {
    let f = ticked_figure(
        RuleOptions {
            dash: Dash::Dotted,
            width: SizeSpec::pt(4.2).unwrap(),
            ..RuleOptions::default()
        },
        true,
    );
    let (_, baseline) = baseline_y_of(&f);
    let drawn = items(&f);
    let ((_, _), (_, end)) = ends(drawn.last().unwrap());
    assert_near(end, baseline);
    // The round cap of the last dot reaches 2.1 pt past the path's end; the clip stops it.
    let [clip] = layer_clips(&f).try_into().unwrap();
    assert_near(f64::from(clip.y + clip.height), baseline);
}
