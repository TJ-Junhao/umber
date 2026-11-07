//! Equal aspect on the planned layout: equal scales, the plot in its room, what follows it, the
//! fits that change limits, grids and the refusals.

use crate::layout::{Area, Label, Layout};
use crate::model::Figure;
use crate::pipeline::{plan_figure, try_plan_panels};
use crate::resolve::Resolved;
use crate::scene::Item;
use crate::{
    Align, AxisData, BubbleOptions, ColorBy, ColorbarOptions, ColorbarPosition, Error, FontLibrary,
    Grid, GridIndex, LegendMode, LegendPosition, LegendStyle, LineOptions, LogBase, Place,
    Placement, PointColor, PointLabels, ScaleKind, ScatterOptions, Share, Theme,
};

const LINEAR: ScaleKind = ScaleKind::Linear;
const CLOSE: f64 = 1e-9;

fn lib() -> &'static FontLibrary {
    FontLibrary::global()
}

/// Every text of `items`, groups flattened, in drawing order.
fn texts(items: &[Item]) -> Vec<&crate::scene::TextItem> {
    let mut found = Vec::new();
    for item in items {
        match item {
            Item::Group { items, .. } => found.extend(texts(items)),
            Item::Text(text) => found.push(text),
            _ => {}
        }
    }
    found
}

/// A circle of radius 1 around the origin, as points of a line.
fn circle() -> (Vec<f64>, Vec<f64>) {
    (0..=120)
        .map(|i| {
            let angle = std::f64::consts::TAU * f64::from(i) / 120.0;
            (angle.cos(), angle.sin())
        })
        .unzip()
}

fn draw_circle(axes: &mut crate::AxesMut<'_>, label: &str) {
    let (x, y) = circle();
    axes.line(
        AxisData::Numbers(x),
        &y,
        LineOptions {
            label: label.to_owned(),
            ..LineOptions::default()
        },
    )
    .unwrap();
}

fn circle_figure(width: f64, height: f64, equal: bool, theme: Theme) -> Figure {
    let mut f = Figure::new(width, height, theme).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    draw_circle(&mut axes, "");
    axes.xticks(&[-1.0, 0.0, 1.0])
        .unwrap()
        .yticks(&[-1.0, 0.0, 1.0])
        .unwrap();
    if equal {
        axes.equal_aspect().unwrap();
    }
    f
}

fn spans(resolved: &Resolved) -> (f64, f64) {
    let span = |scale: &crate::ContinuousScale| {
        let (lo, hi) = scale.domain();
        hi - lo
    };
    (
        span(resolved.x.continuous_scale()),
        span(resolved.y.continuous_scale()),
    )
}

#[track_caller]
fn assert_equal_scales(resolved: &Resolved, layout: &Layout) {
    let (x, y) = spans(resolved);
    let (x_scale, y_scale) = (layout.plot.width / x, layout.plot.height / y);
    assert!(
        (x_scale / y_scale - 1.0).abs() < 1e-12,
        "{x_scale} pt per unit on x, {y_scale} on y"
    );
}

#[track_caller]
fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < CLOSE, "{a} and {b}");
}

/// Asserts that `plot` is centred in `room` along both sides and lies inside it.
#[track_caller]
fn assert_centred(plot: &Area, room: &Area) {
    assert_close(plot.x + plot.width / 2.0, room.x + room.width / 2.0);
    assert_close(plot.y + plot.height / 2.0, room.y + room.height / 2.0);
    assert!(plot.width <= room.width && plot.height <= room.height);
}

#[test]
fn the_scales_are_equal_on_wide_and_tall_figures() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for (width, height) in [(9.0, 3.0), (3.0, 9.0), (6.0, 4.0), (4.0, 4.0)] {
            let f = circle_figure(width, height, true, theme.clone());
            let (resolved, layout) = plan_figure(&f, (0, 0), lib());
            assert_equal_scales(&resolved, &layout);
            assert_centred(&layout.plot, &layout.room);
        }
    }
}

#[test]
fn explicit_limits_of_any_shape_get_equal_scales() {
    for (x, y) in [((0.0, 10.0), (0.0, 1.0)), ((-1.0, 1.0), (100.0, 110.0))] {
        for (width, height) in [(9.0, 3.0), (3.0, 9.0)] {
            let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
            let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
            draw_circle(&mut axes, "");
            axes.xlim(x.0, x.1).unwrap().ylim(y.0, y.1).unwrap();
            axes.equal_aspect().unwrap();
            let (resolved, layout) = plan_figure(&f, (0, 0), lib());
            assert_eq!(resolved.x.continuous_scale().domain(), x);
            assert_eq!(resolved.y.continuous_scale().domain(), y);
            assert_equal_scales(&resolved, &layout);
        }
    }
}

#[test]
fn calling_it_twice_is_calling_it_once() {
    let mut twice = circle_figure(8.0, 4.0, true, Theme::pigment());
    twice.single_axes_mut().equal_aspect().unwrap();
    assert_eq!(twice, circle_figure(8.0, 4.0, true, Theme::pigment()));
}

#[test]
fn the_limits_stay_and_the_shrunk_side_is_centred() {
    for (width, height) in [(9.0, 3.0), (3.0, 9.0)] {
        let plain = circle_figure(width, height, false, Theme::pigment());
        let equal = circle_figure(width, height, true, Theme::pigment());
        let (plain_resolved, plain) = plan_figure(&plain, (0, 0), lib());
        let (resolved, layout) = plan_figure(&equal, (0, 0), lib());
        assert_eq!(
            resolved.x.continuous_scale().domain(),
            plain_resolved.x.continuous_scale().domain()
        );
        assert_eq!(
            resolved.y.continuous_scale().domain(),
            plain_resolved.y.continuous_scale().domain()
        );
        // The room is the plot of the same figure without the setting.
        assert_eq!(layout.room, plain.plot);
        assert_centred(&layout.plot, &plain.plot);
    }
}

#[test]
fn the_side_that_does_not_shrink_keeps_its_size() {
    let wide = |equal| circle_figure(9.0, 3.0, equal, Theme::pigment());
    let tall = |equal| circle_figure(3.0, 9.0, equal, Theme::pigment());
    let plot = |f: Figure| plan_figure(&f, (0, 0), lib()).1.plot;
    assert_eq!(plot(wide(true)).height, plot(wide(false)).height);
    assert_eq!(plot(wide(true)).y, plot(wide(false)).y);
    assert!(plot(wide(true)).width < plot(wide(false)).width);
    assert_eq!(plot(tall(true)).width, plot(tall(false)).width);
    assert_eq!(plot(tall(true)).x, plot(tall(false)).x);
    assert!(plot(tall(true)).height < plot(tall(false)).height);
}

/// A circle with a title, a subtitle and axis labels, in a figure of the given size.
fn labelled(width: f64, height: f64, equal: bool) -> Figure {
    let mut f = circle_figure(width, height, equal, Theme::pigment());
    f.single_axes_mut()
        .title("Domain")
        .subtitle("Unit circle")
        .xlabel("x (m)")
        .ylabel("y (m)");
    f
}

#[test]
fn ticks_and_axis_labels_follow_the_plot() {
    // Each stands as far from the plot as it does without the setting.
    for (width, height) in [(9.0, 3.0), (3.0, 9.0)] {
        let (_, plain) = plan_figure(&labelled(width, height, false), (0, 0), lib());
        let (_, equal) = plan_figure(&labelled(width, height, true), (0, 0), lib());
        let x_of = |layout: &Layout| {
            (
                layout.xlabel.as_ref().unwrap().x + layout.xlabel.as_ref().unwrap().width() / 2.0
                    - layout.plot.x,
                layout.xlabel.as_ref().unwrap().y - layout.plot.bottom(),
                layout.ylabel.as_ref().unwrap().x - layout.plot.x,
                layout.ylabel.as_ref().unwrap().y - layout.plot.y,
                layout.y_tick_labels[0].x - layout.plot.x,
                layout.x_tick_labels[0].y - layout.plot.bottom(),
            )
        };
        let (a, b) = (x_of(&plain), x_of(&equal));
        assert_close(a.1, b.1);
        assert_close(a.2, b.2);
        assert_close(a.3, b.3);
        assert_close(a.4, b.4);
        assert_close(a.5, b.5);
        assert_close(b.0, equal.plot.width / 2.0);
    }
}

#[test]
fn the_title_keeps_its_alignment_to_the_plot() {
    // Left of the plot by the y tick column, above it by the same distance, as without the
    // setting; on a right or centred title, the other end of the box moves in too.
    for align in [Align::Left, Align::Center, Align::Right] {
        for (width, height) in [(9.0, 3.0), (3.0, 9.0)] {
            let theme = Theme::pigment().with_title_align(align).unwrap();
            let build = |equal: bool| {
                let mut f = circle_figure(width, height, equal, theme.clone());
                f.single_axes_mut().title("Domain").subtitle("Unit circle");
                f
            };
            let (_, plain) = plan_figure(&build(false), (0, 0), lib());
            let (_, equal) = plan_figure(&build(true), (0, 0), lib());
            let inset = (equal.room.width - equal.plot.width) / 2.0;
            let drop = (equal.room.height - equal.plot.height) / 2.0;
            for (a, b) in [
                (&plain.title, &equal.title),
                (&plain.subtitle, &equal.subtitle),
            ] {
                let (a, b) = (a.as_ref().unwrap().first(), b.as_ref().unwrap().first());
                let moved = match align {
                    Align::Left => inset,
                    Align::Center => 0.0,
                    Align::Right => -inset,
                };
                assert_close(b.x, a.x + moved);
                assert_close(b.y, a.y + drop);
            }
        }
    }
}

#[test]
fn a_title_wider_than_the_plot_stays_in_the_room() {
    // A narrow plot in a grid cell: the title does not fit over the plot, and is not refused
    // for that, since the panel's room holds it.
    let title = "A title much wider than the plot below it";
    let mut f = Figure::with_grid(10.0, 2.5, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in [0, 1] {
        let mut axes = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                LINEAR,
                LINEAR,
            )
            .unwrap();
        draw_circle(&mut axes, "");
        axes.title(title);
        if col == 1 {
            axes.equal_aspect().unwrap();
        }
    }
    let planned = try_plan_panels(&f, lib()).unwrap();
    let layout = &planned[1].1;
    let line = layout.title.as_ref().unwrap().first();
    assert!(line.width() > layout.plot.width);
    assert_close(line.x, layout.room.x);
    assert!(line.x + line.width() <= layout.room.right() + CLOSE);
}

#[test]
fn a_long_x_label_uses_the_room_and_stays_centred() {
    let label = "A label much wider than the plot above it, in metres";
    let mut f = circle_figure(9.0, 2.2, true, Theme::pigment());
    f.single_axes_mut().xlabel(label);
    let (_, layout) = plan_figure(&f, (0, 0), lib());
    let drawn = layout.xlabel.as_ref().unwrap();
    assert!(drawn.width() > layout.plot.width);
    assert_close(
        drawn.x + drawn.width() / 2.0,
        layout.plot.x + layout.plot.width / 2.0,
    );
}

#[test]
fn a_long_y_label_is_refused_with_a_hint_that_helps() {
    let mut f = circle_figure(9.0, 2.2, true, Theme::pigment());
    f.single_axes_mut()
        .ylabel("A label much wider than the plot below it, in metres");
    let error = f.to_scene(lib()).unwrap_err();
    assert!(error.to_string().contains("y axis label"), "{error}");
    assert_eq!(
        error.hint(),
        Some("shorten the label, use a smaller size, or remove equal_aspect().")
    );
}

/// A scatter colored by value on a figure of the given size.
fn mapped(width: f64, height: f64, equal: bool, position: ColorbarPosition) -> Figure {
    let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    let (x, y) = circle();
    let values: Vec<f64> = (0..x.len()).map(|i| i as f64).collect();
    axes.scatter(
        AxisData::Numbers(x),
        &y,
        ScatterOptions {
            color: PointColor::By(ColorBy::new(values)),
            ..ScatterOptions::default()
        },
    )
    .unwrap();
    axes.colorbar(ColorbarOptions {
        label: "Phase".into(),
        position: Some(position),
        ..ColorbarOptions::default()
    })
    .unwrap();
    if equal {
        axes.equal_aspect().unwrap();
    }
    f
}

#[test]
fn a_colorbar_follows_the_plot() {
    for (width, height) in [(9.0, 3.5), (3.5, 9.0)] {
        let f = mapped(width, height, true, ColorbarPosition::Right);
        let (resolved, layout) = plan_figure(&f, (0, 0), lib());
        assert_equal_scales(&resolved, &layout);
        let bar = layout.colorbar.as_ref().unwrap().bar;
        assert_eq!(bar.y, layout.plot.y);
        assert_eq!(bar.height, layout.plot.height);
        assert!(bar.x > layout.plot.right() && bar.x < layout.plot.right() + 20.0);

        let f = mapped(width, height, true, ColorbarPosition::Bottom);
        let (resolved, layout) = plan_figure(&f, (0, 0), lib());
        assert_equal_scales(&resolved, &layout);
        let bar = layout.colorbar.as_ref().unwrap().bar;
        assert_eq!(bar.x, layout.plot.x);
        assert_eq!(bar.width, layout.plot.width);
        assert!(bar.y > layout.plot.bottom() && bar.y < layout.plot.bottom() + 60.0);
    }
}

/// Two labelled circles with the legend at `position`, on a figure of the given size.
fn legended(width: f64, height: f64, position: LegendPosition) -> Figure {
    let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    draw_circle(&mut axes, "Inner");
    draw_circle(&mut axes, "Outer");
    axes.legend_at(LegendStyle::Header, position, crate::SizeSpec::Auto)
        .unwrap();
    axes.equal_aspect().unwrap();
    f
}

#[test]
fn an_outside_legend_stays_next_to_the_plot() {
    for (width, height) in [(9.0, 3.5), (3.5, 9.0)] {
        let (resolved, layout) = plan_figure(
            &legended(width, height, LegendPosition::Right),
            (0, 0),
            lib(),
        );
        assert_equal_scales(&resolved, &layout);
        let first = &layout.legend.entries()[0];
        assert!(first.swatch.x >= layout.plot.right());
        assert!(first.swatch.x < layout.plot.right() + 20.0);
        assert!(first.label.y > layout.plot.y && first.label.y < layout.plot.bottom());

        let (resolved, layout) = plan_figure(
            &legended(width, height, LegendPosition::Bottom),
            (0, 0),
            lib(),
        );
        assert_equal_scales(&resolved, &layout);
        let first = &layout.legend.entries()[0];
        assert!(first.label.y > layout.plot.bottom());
        assert!(first.label.y < layout.plot.bottom() + 50.0);
    }
}

#[test]
fn a_header_legend_keeps_to_the_plot_ends() {
    // On a wide figure the plot is narrower than its room: the row under the title moves in
    // with it, on the left as on the right.
    let (_, layout) = plan_figure(&legended(9.0, 3.5, LegendPosition::TopRight), (0, 0), lib());
    let last = layout.legend.entries().last().unwrap();
    assert_close(last.label.x + last.label.width(), layout.plot.right());

    let plain = {
        let mut f = Figure::new(9.0, 3.5, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
        draw_circle(&mut axes, "Inner");
        draw_circle(&mut axes, "Outer");
        axes.legend_at(
            LegendStyle::Header,
            LegendPosition::TopLeft,
            crate::SizeSpec::Auto,
        )
        .unwrap();
        f
    };
    let (_, plain) = plan_figure(&plain, (0, 0), lib());
    let (_, layout) = plan_figure(&legended(9.0, 3.5, LegendPosition::TopLeft), (0, 0), lib());
    let inset = (layout.room.width - layout.plot.width) / 2.0;
    assert!(inset > 0.0);
    let x = |layout: &Layout| layout.legend.entries()[0].swatch.x;
    assert_close(x(&layout), x(&plain) + inset);
}

/// A right legend of eight entries beside limits that make the plot wide and short.
fn tall_legend(height: f64) -> Figure {
    let mut f = Figure::new(9.0, height, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    axes.xlim(0.0, 10.0).unwrap().ylim(0.0, 1.0).unwrap();
    for i in 0..8 {
        draw_circle(&mut axes, &format!("Series {i}"));
    }
    axes.legend_at(
        LegendStyle::Header,
        LegendPosition::Right,
        crate::SizeSpec::Auto,
    )
    .unwrap();
    axes.equal_aspect().unwrap();
    f
}

#[test]
fn a_legend_taller_than_the_plot_is_refused_with_advice_that_helps() {
    // The width sets the plot, so a taller figure leaves the message as it is.
    let refused = |height| tall_legend(height).to_scene(lib()).unwrap_err();
    let (short, tall) = (refused(4.0), refused(12.0));
    assert_eq!(short.to_string(), tall.to_string());
    assert!(short.to_string().contains("the legend is"), "{short}");
    assert_eq!(
        short.hint(),
        Some(
            "make the figure wider, use fewer legend entries or another legend position, or \
             remove equal_aspect()."
        )
    );
}

#[test]
fn a_value_row_follows_the_plot() {
    let mut f = Figure::new(9.0, 3.5, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    axes.scatter(
        AxisData::Numbers(vec![0.0, 1.0, 2.0]),
        &[0.5, 1.37, 1.0],
        ScatterOptions {
            labels: PointLabels {
                annotate: true,
                place: Place::Column,
                ..PointLabels::default()
            },
            ..ScatterOptions::default()
        },
    )
    .unwrap();
    axes.equal_aspect().unwrap();
    let (resolved, layout) = plan_figure(&f, (0, 0), lib());
    assert_equal_scales(&resolved, &layout);
    let column = layout.value_column.as_ref().unwrap();
    let (top, height) = column.cells[0];
    assert!(top + height <= layout.plot.y);
    assert!(top > layout.plot.y - 40.0);
    // The values are written above their points, on the smaller plot.
    let scene = f.to_scene(lib()).unwrap();
    let text = texts(&scene.items)
        .into_iter()
        .find(|text| text.text == "1.37")
        .unwrap();
    let centre = f64::from(text.origin.x) + f64::from(text.width) / 2.0;
    let position = resolved.x.continuous_scale().normalize(1.0);
    let point = layout
        .plot
        .x_at(crate::resolve::NormalizedPos::new(position));
    assert!((centre - point).abs() < 1e-3, "{centre} and {point}");
}

#[test]
fn direct_labels_follow_the_plot() {
    let mut f = Figure::new(9.0, 3.5, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    axes.line(
        AxisData::Numbers(vec![0.0, 1.0]),
        &[0.0, 1.0],
        LineOptions {
            label: "Rise".into(),
            ..LineOptions::default()
        },
    )
    .unwrap();
    axes.legend(LegendStyle::Direct, crate::SizeSpec::Auto)
        .unwrap();
    axes.equal_aspect().unwrap();
    let (resolved, layout) = plan_figure(&f, (0, 0), lib());
    assert_equal_scales(&resolved, &layout);
    assert!(layout.plot.width < layout.room.width);
    let label = layout.direct[0].label.as_ref().unwrap();
    assert!(label.x > layout.plot.right() && label.x < layout.plot.right() + 20.0);
    assert!(label.y > layout.plot.y - 1.0 && label.y < layout.plot.bottom() + 10.0);
}

#[test]
fn the_caption_stays_under_the_figure() {
    let build = |equal| {
        let mut f = circle_figure(3.5, 9.0, equal, Theme::pigment());
        f.caption("Unit circle, in metres.");
        f
    };
    let text = |f: &Figure| {
        let scene = f.to_scene(lib()).unwrap();
        let item = texts(&scene.items)
            .into_iter()
            .find(|text| text.text.starts_with("Unit circle"))
            .unwrap()
            .clone();
        (item.origin.x, item.origin.y)
    };
    assert_eq!(text(&build(true)), text(&build(false)));
}

/// `count` deterministic points in a wide or a tall cloud.
fn cloud(count: usize, (x_span, y_span): (f64, f64)) -> (Vec<f64>, Vec<f64>) {
    (0..count)
        .map(|i| {
            let u = (i as f64 * 0.618_033_988_7).fract();
            let v = (i as f64 * 0.414_213_562_3).fract();
            (u * x_span, v * y_span)
        })
        .unzip()
}

#[test]
fn bubbles_widen_the_limits_and_the_scales_stay_equal() {
    for (span, size) in [
        ((10.0, 2.0), (6.0, 4.0)),
        ((2.0, 10.0), (6.0, 4.0)),
        ((10.0, 10.0), (3.0, 7.0)),
    ] {
        let (x, y) = cloud(40, span);
        let sizes: Vec<f64> = (0..x.len()).map(|i| 1.0 + (i % 7) as f64).collect();
        let mut f = Figure::new(size.0, size.1, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
        axes.bubbles(&x, &y, &sizes, BubbleOptions::default())
            .unwrap();
        axes.equal_aspect().unwrap();
        let (resolved, layout) = plan_figure(&f, (0, 0), lib());
        assert_equal_scales(&resolved, &layout);
        let extent = |values: &[f64]| {
            let min = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            min - 0.05 * (max - min)..max + 0.05 * (max - min)
        };
        let (lo, hi) = resolved.x.continuous_scale().domain();
        assert!(lo < extent(&x).start && hi > extent(&x).end, "{lo} {hi}");
        // Two renders of the same figure are the same.
        assert_eq!(
            f.to_svg(crate::SvgText::Editable).unwrap(),
            f.to_svg(crate::SvgText::Editable).unwrap()
        );
    }
}

/// Bubbles of two sizes on axes with equal aspect: the large ones at the top and bottom widen y,
/// which narrows the plot under the small ones at the left and right.
fn crossed_bubbles(equal: bool) -> Figure {
    let mut f = Figure::new(6.0, 1.6, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    axes.bubbles(
        &[0.0, 10.0, 5.0, 5.0],
        &[5.0, 5.0, 0.0, 10.0],
        &[1.0, 1.0, 12.0, 12.0],
        BubbleOptions::default(),
    )
    .unwrap();
    if equal {
        axes.equal_aspect().unwrap();
    }
    f
}

#[test]
fn one_fit_holds_bubbles_that_change_the_shape_of_the_plot() {
    let (resolved, layout) = plan_figure(&crossed_bubbles(true), (0, 0), lib());
    assert_equal_scales(&resolved, &layout);
}

#[test]
fn an_axis_with_explicit_limits_sets_the_scale_of_the_other() {
    // x has no fit, so its limits are one of the things the scale has to hold.
    let mut f = Figure::new(9.0, 3.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    axes.bubbles(
        &[0.0, 10.0],
        &[0.0, 1.0],
        &[1.0, 1.0],
        BubbleOptions::default(),
    )
    .unwrap();
    axes.xlim(-5.0, 15.0).unwrap();
    axes.equal_aspect().unwrap();
    let (resolved, layout) = plan_figure(&f, (0, 0), lib());
    assert_eq!(resolved.x.continuous_scale().domain(), (-5.0, 15.0));
    assert_equal_scales(&resolved, &layout);
    let (lo, hi) = resolved.y.continuous_scale().domain();
    let scale = layout.plot.height / (hi - lo);
    assert!(lo * scale <= -18.0 && (hi - 1.0) * scale >= 18.0);
}

#[test]
fn bubbles_that_do_not_settle_are_an_error() {
    // Without the setting the same figure falls back, and the plot clips the bubbles.
    crate::bubble_fit::FITS_ALLOWED.with(|allowed| allowed.set(0));
    assert!(crossed_bubbles(false).to_scene(lib()).is_ok());
    let error = crossed_bubbles(true).to_scene(lib()).unwrap_err();
    assert!(matches!(error, Error::Style { .. }));
    assert!(
        error.to_string().contains("do not settle"),
        "the message: {error}"
    );
    assert_eq!(
        error.hint(),
        Some("make the figure larger, or set explicit limits, which clip the bubbles.")
    );
}

#[test]
fn bubbles_fit_a_flat_domain_at_its_own_figure_size() {
    // The plot of the data alone is about 10 pt tall, too short for one bubble, but the limits
    // that hold the bubbles give a taller plot at the same scale.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    axes.bubbles(
        &[0.0, 40.0],
        &[0.0, 1.0],
        &[1.0, 1.0],
        BubbleOptions::default(),
    )
    .unwrap();
    axes.equal_aspect().unwrap();
    let (resolved, layout) = plan_figure(&f, (0, 0), lib());
    assert_equal_scales(&resolved, &layout);
    // The width sets the scale, and the height is what the bubbles need of it.
    assert_eq!(layout.plot.width, layout.room.width);
    assert!(layout.plot.height > 2.0 * 18.0 && layout.plot.height < 60.0);
    let (lo, hi) = resolved.y.continuous_scale().domain();
    let scale = layout.plot.height / (hi - lo);
    assert!(lo * scale <= -18.0 && (hi - 1.0) * scale >= 18.0);
}

/// The size a bubbles error names, in points.
fn named_size(message: &str) -> [f64; 2] {
    let sides: Vec<f64> = message
        .split("at least ")
        .nth(1)
        .unwrap()
        .split(" pt")
        .next()
        .unwrap()
        .split(" × ")
        .map(|side| side.parse().unwrap())
        .collect();
    [sides[0], sides[1]]
}

#[test]
fn bubbles_too_large_grow_only_the_side_they_need() {
    // A bubble taller than the room needs a taller figure, whatever the width: a failed try at
    // one size proves nothing about the other side.
    let build = |width: f64, height: f64| {
        let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
        axes.bubbles(
            &[0.0, 40.0],
            &[0.0, 1.0],
            &[1.0, 1.0],
            BubbleOptions::default(),
        )
        .unwrap();
        axes.equal_aspect().unwrap();
        f
    };
    let f = build(6.0, 1.0);
    let error = f.to_scene(lib()).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("need a figure at least"), "{message}");
    let [width, height] = named_size(&message);
    assert_eq!(width, 6.0 * 72.0);
    assert!(height > 72.0 && height < 2.0 * 72.0, "{message}");
    let named = build(width / 72.0, height / 72.0);
    let (resolved, layout) = plan_figure(&named, (0, 0), lib());
    assert_equal_scales(&resolved, &layout);
    // A point less, and the figure no longer holds them.
    assert!(
        build(width / 72.0, (height - 1.0) / 72.0)
            .to_scene(lib())
            .is_err()
    );
}

/// A 1 × 2 grid of circles with labelled ticks of one width, equal aspect where `equal` says.
fn pair(width: f64, height: f64, equal: [bool; 2]) -> Figure {
    let grid = Grid::new(1, 2).unwrap().with_legend(LegendMode::PerPanel);
    let mut f = Figure::with_grid(width, height, Theme::pigment(), grid).unwrap();
    for col in [0, 1] {
        let mut axes = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                LINEAR,
                LINEAR,
            )
            .unwrap();
        draw_circle(&mut axes, "");
        axes.xticks(&[-1.0, 0.0, 1.0])
            .unwrap()
            .yticks(&[-1.0, 0.0, 1.0])
            .unwrap()
            .title("Panel")
            .xlabel("x");
        if equal[col as usize] {
            axes.equal_aspect().unwrap();
        }
    }
    f
}

#[test]
fn a_panel_of_a_grid_shrinks_in_its_own_cell() {
    for (width, height) in [(12.0, 3.0), (5.0, 7.0)] {
        for equal in [[false, true], [true, false]] {
            let plain = try_plan_panels(&pair(width, height, [false; 2]), lib()).unwrap();
            let planned = try_plan_panels(&pair(width, height, equal), lib()).unwrap();
            for (i, (resolved, layout)) in planned.iter().enumerate() {
                if equal[i] {
                    assert_equal_scales(resolved, layout);
                    assert_eq!(layout.room, plain[i].1.plot);
                    assert_centred(&layout.plot, &plain[i].1.plot);
                    assert_ne!(layout.plot, plain[i].1.plot);
                } else {
                    // The panel without the setting has the plot it has without it anywhere.
                    assert_eq!(layout.plot, plain[i].1.plot);
                    assert_eq!(layout.frame, plain[i].1.frame);
                }
            }
        }
    }
}

/// A 1 × 2 grid of circles with automatic ticks, equal aspect where `equal` says.
fn automatic_pair(width: f64, height: f64, equal: [bool; 2]) -> Figure {
    let grid = Grid::new(1, 2).unwrap().with_legend(LegendMode::PerPanel);
    let mut f = Figure::with_grid(width, height, Theme::pigment(), grid).unwrap();
    for col in [0, 1] {
        let mut axes = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                LINEAR,
                LINEAR,
            )
            .unwrap();
        draw_circle(&mut axes, "");
        axes.title("Panel").xlabel("x").ylabel("y");
        if equal[col as usize] {
            axes.equal_aspect().unwrap();
        }
    }
    f
}

#[test]
fn another_panel_keeps_its_layout_when_tick_labels_change() {
    // On a tall figure the plot that shrinks loses its finer ticks, and with them its widest
    // labels, which on the bands of the grid would widen the panel beside it.
    let (width, height) = (6.0, 12.0);
    let widest = |layout: &Layout| {
        layout
            .y_tick_labels
            .iter()
            .map(Label::width)
            .fold(0.0, f64::max)
    };
    let plain = try_plan_panels(&automatic_pair(width, height, [false; 2]), lib()).unwrap();
    let planned = try_plan_panels(&automatic_pair(width, height, [false, true]), lib()).unwrap();
    assert!(widest(&planned[1].1) < widest(&plain[1].1));
    assert_eq!(planned[0].1.plot, plain[0].1.plot);
    assert_eq!(planned[0].1.room, plain[0].1.room);
    let labels = |layout: &Layout| {
        layout
            .y_tick_labels
            .iter()
            .chain(&layout.x_tick_labels)
            .map(|label| (label.text.clone(), label.x, label.y))
            .collect::<Vec<_>>()
    };
    assert_eq!(labels(&planned[0].1), labels(&plain[0].1));
    assert_eq!(planned[1].1.room, plain[1].1.plot);
    assert_equal_scales(&planned[1].0, &planned[1].1);
    assert_centred(&planned[1].1.plot, &planned[1].1.room);
}

#[test]
fn automatic_ticks_keep_the_side_that_does_not_shrink() {
    // One cell as many: the room is the plot without the setting, whatever labels the smaller
    // plot gets, so nothing on the side that does not shrink moves.
    let build = |width: f64, height: f64, equal: bool| {
        let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
        draw_circle(&mut axes, "");
        if equal {
            axes.equal_aspect().unwrap();
        }
        f
    };
    let plots = |width, height| {
        let plot = |equal| {
            plan_figure(&build(width, height, equal), (0, 0), lib())
                .1
                .plot
        };
        (plot(false), plot(true))
    };
    let (plain, equal) = plots(3.0, 9.0);
    assert_eq!(equal.width, plain.width);
    assert_eq!(equal.x, plain.x);
    assert!(equal.height < plain.height);
    let (plain, equal) = plots(9.0, 3.0);
    assert_eq!(equal.height, plain.height);
    assert_eq!(equal.y, plain.y);
    assert!(equal.width < plain.width);
}

/// A 1 × 2 grid with `left` drawn on the left panel, with equal aspect where `equal` says, and
/// `right` on the right one.
fn left_and_right(
    (width, height): (f64, f64),
    equal: bool,
    left: impl Fn(&mut crate::AxesMut<'_>),
    right: impl Fn(&mut crate::AxesMut<'_>),
) -> Figure {
    let grid = Grid::new(1, 2).unwrap().with_legend(LegendMode::PerPanel);
    let mut f = Figure::with_grid(width, height, Theme::pigment(), grid).unwrap();
    let mut axes = f.add_axes_at(at(0), LINEAR, LINEAR).unwrap();
    left(&mut axes);
    if equal {
        axes.equal_aspect().unwrap();
    }
    let mut axes = f.add_axes_at(at(1), LINEAR, LINEAR).unwrap();
    right(&mut axes);
    f
}

fn plain_line(axes: &mut crate::AxesMut<'_>) {
    axes.line(
        AxisData::Numbers(vec![0.0, 1.0]),
        &[0.0, 1.0],
        LineOptions::default(),
    )
    .unwrap();
}

#[test]
fn tick_labels_that_grow_stay_in_their_bounds() {
    // The limits the bubbles get need decimals that the limits without the setting do not, on a
    // plot whose width the limits fill: its labels would reach into the padding.
    let left = |axes: &mut crate::AxesMut<'_>| {
        axes.bubbles(
            &[0.0, 20.0],
            &[1.0, 6.0],
            &[1.0, 1.0],
            BubbleOptions::default(),
        )
        .unwrap();
    };
    let plain =
        try_plan_panels(&left_and_right((12.0, 4.0), false, left, plain_line), lib()).unwrap();
    let planned =
        try_plan_panels(&left_and_right((12.0, 4.0), true, left, plain_line), lib()).unwrap();
    let (resolved, layout) = &planned[0];
    assert_equal_scales(resolved, layout);
    let widest = |layout: &Layout| {
        layout
            .y_tick_labels
            .iter()
            .map(Label::width)
            .fold(0.0, f64::max)
    };
    assert!(widest(layout) > widest(&plain[0].1));
    // The plot gives up what the labels need on their side, and the rest stays.
    assert_close(
        layout.plot.x - plain[0].1.plot.x,
        widest(layout) - widest(&plain[0].1),
    );
    assert_close(layout.plot.right(), plain[0].1.plot.right());
    for label in &layout.y_tick_labels {
        assert!(label.x >= crate::layout::PADDING - 1e-9, "{}", label.x);
    }
    assert_eq!(planned[1].1.plot, plain[1].1.plot);
    // The title keeps its place over the plot.
    assert_close(layout.room.x, plain[0].1.plot.x);
}

#[test]
fn signed_tick_labels_stay_out_of_the_padding() {
    let left = |axes: &mut crate::AxesMut<'_>| {
        axes.bubbles(
            &[0.0, 6.0],
            &[0.0, 1.0],
            &[4.0, 1.0],
            BubbleOptions::default(),
        )
        .unwrap();
        axes.ytick_format(crate::TickFormat::fixed(1).unwrap())
            .unwrap();
    };
    let plain =
        try_plan_panels(&left_and_right((12.0, 4.0), false, left, plain_line), lib()).unwrap();
    let planned =
        try_plan_panels(&left_and_right((12.0, 4.0), true, left, plain_line), lib()).unwrap();
    assert_equal_scales(&planned[0].0, &planned[0].1);
    for label in &planned[0].1.y_tick_labels {
        assert!(label.x >= crate::layout::PADDING - 1e-9, "{}", label.x);
    }
    assert_eq!(planned[1].1.plot, plain[1].1.plot);
}

#[test]
fn hexagons_set_the_limits_before_the_size_is_judged() {
    // On the first plan the flat data would make the plot far thinner than a point; the whole
    // cells of the lattice give it its height.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    axes.hexbin(
        &[0.0, 1.0e6],
        &[0.0, 0.0],
        crate::HexbinOptions {
            gridsize: 30,
            ..crate::HexbinOptions::default()
        },
    )
    .unwrap();
    axes.equal_aspect().unwrap();
    let (resolved, layout) = plan_figure(&f, (0, 0), lib());
    assert_equal_scales(&resolved, &layout);
    assert!(layout.plot.height >= 1.0);
    assert_eq!(layout.plot.width, layout.room.width);
}

fn small_hexbin(axes: &mut crate::AxesMut<'_>) {
    axes.hexbin(
        &[0.0, 0.6, 0.2],
        &[0.0, 4.0, 8.0],
        crate::HexbinOptions {
            gridsize: 3,
            ..crate::HexbinOptions::default()
        },
    )
    .unwrap();
}

#[test]
fn a_hexbin_with_equal_aspect_leaves_the_panel_beside_it() {
    let build = |equal| left_and_right((6.0, 9.0), equal, small_hexbin, plain_line);
    let plain = try_plan_panels(&build(false), lib()).unwrap();
    let planned = try_plan_panels(&build(true), lib()).unwrap();
    assert_eq!(planned[1].1.plot, plain[1].1.plot);
    assert_equal_scales(&planned[0].0, &planned[0].1);
}

#[test]
fn a_hexbin_beside_equal_aspect_keeps_its_layout() {
    let circle_panel = |equal: bool| {
        left_and_right(
            (6.0, 9.0),
            equal,
            |axes| draw_circle(axes, ""),
            small_hexbin,
        )
    };
    let plain = try_plan_panels(&circle_panel(false), lib()).unwrap();
    let planned = try_plan_panels(&circle_panel(true), lib()).unwrap();
    assert_eq!(planned[1].1.plot, plain[1].1.plot);
    assert_eq!(
        planned[1].0.x.continuous_scale().domain(),
        plain[1].0.x.continuous_scale().domain()
    );
    assert_eq!(
        planned[1].0.y.continuous_scale().domain(),
        plain[1].0.y.continuous_scale().domain()
    );
}

#[test]
fn a_direct_label_only_has_to_fit_the_final_plot() {
    // Without the setting the label runs past its panel; on the smaller plot it fits.
    let left = |axes: &mut crate::AxesMut<'_>| {
        axes.line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions {
                label: "W".repeat(14),
                ..LineOptions::default()
            },
        )
        .unwrap();
        axes.legend(LegendStyle::Direct, crate::SizeSpec::Auto)
            .unwrap();
    };
    assert!(
        left_and_right((12.0, 3.0), false, left, plain_line)
            .to_scene(lib())
            .is_err()
    );
    let f = left_and_right((12.0, 3.0), true, left, plain_line);
    let planned = try_plan_panels(&f, lib()).unwrap();
    assert_equal_scales(&planned[0].0, &planned[0].1);
    let label = planned[0].1.direct[0].label.as_ref().unwrap();
    assert!(label.x + label.width() <= planned[0].1.frame.right());
}

#[test]
fn both_panels_of_a_grid_can_have_equal_aspect() {
    let plain = try_plan_panels(&pair(12.0, 3.0, [false; 2]), lib()).unwrap();
    let planned = try_plan_panels(&pair(12.0, 3.0, [true; 2]), lib()).unwrap();
    for (i, (resolved, layout)) in planned.iter().enumerate() {
        assert_equal_scales(resolved, layout);
        assert_centred(&layout.plot, &plain[i].1.plot);
    }
}

#[test]
fn a_shared_colorbar_stays_with_the_grid() {
    // The bar of a row belongs to the row: it keeps the place and the size it has without the
    // setting.
    let build = |equal: bool| {
        let grid = Grid::new(1, 2)
            .unwrap()
            .with_share_color(Share::Row)
            .with_legend(LegendMode::PerPanel);
        let mut f = Figure::with_grid(12.0, 3.0, Theme::pigment(), grid).unwrap();
        for col in [0, 1] {
            let mut axes = f
                .add_axes_at(
                    Placement {
                        col: GridIndex::At(col),
                        ..Placement::default()
                    },
                    LINEAR,
                    LINEAR,
                )
                .unwrap();
            let (x, y) = circle();
            let values: Vec<f64> = (0..x.len()).map(|i| i as f64).collect();
            axes.scatter(
                AxisData::Numbers(x),
                &y,
                ScatterOptions {
                    color: PointColor::By(ColorBy::new(values)),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
            axes.xticks(&[-1.0, 1.0])
                .unwrap()
                .yticks(&[-1.0, 1.0])
                .unwrap();
            if equal && col == 1 {
                axes.equal_aspect().unwrap();
            }
        }
        f
    };
    let strip = |f: &Figure| {
        let scene = f.to_scene(lib()).unwrap();
        texts(&scene.items)
            .into_iter()
            .map(|text| (text.text.clone(), text.origin.x, text.origin.y))
            .filter(|(text, _, _)| text == "120")
            .collect::<Vec<_>>()
    };
    let bar = strip(&build(false));
    assert_eq!(bar.len(), 1);
    assert_eq!(strip(&build(true)), bar);
}

fn usage_message(error: &Error) -> String {
    assert!(matches!(error, Error::Usage { .. }), "{error}");
    error.to_string()
}

#[test]
fn log_time_and_category_axes_are_refused_at_the_call() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    for (x, y, needle) in [
        (log, LINEAR, "x is a log axis"),
        (LINEAR, log, "y is a log axis"),
        (ScaleKind::Time, LINEAR, "x is a time axis"),
        (LINEAR, ScaleKind::Time, "y is a time axis"),
        (ScaleKind::Category, LINEAR, "x is a category axis"),
        (LINEAR, ScaleKind::Category, "y is a category axis"),
    ] {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(x, y).unwrap();
        let error = axes.equal_aspect().unwrap_err();
        let message = usage_message(&error);
        assert!(message.contains(needle), "{message}");
        assert!(message.starts_with("Axes.equal_aspect(): "), "{message}");
        assert!(error.hint().is_some());
        assert!(!axes.has_equal_aspect());
    }
}

#[test]
fn twins_are_refused_in_either_order() {
    // On the twin itself.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut left = f.add_axes(LINEAR, LINEAR).unwrap();
    let error = left.twin_y(LINEAR).unwrap().equal_aspect().unwrap_err();
    assert!(usage_message(&error).contains("these axes are a twin; a twin y axis"));
    // On the axes that have one.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut left = f.add_axes(LINEAR, LINEAR).unwrap();
    left.twin_y(LINEAR).unwrap();
    let error = left.equal_aspect().unwrap_err();
    assert!(usage_message(&error).contains("these axes have a twin; a twin y axis"));
    assert!(!left.has_equal_aspect());
    // A twin of axes that have the setting.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut left = f.add_axes(LINEAR, LINEAR).unwrap();
    left.equal_aspect().unwrap();
    let error = left.twin_y(LINEAR).unwrap_err();
    let message = usage_message(&error);
    assert!(message.starts_with("Axes.twin_y(): "), "{message}");
    assert!(message.contains("these axes have equal aspect; a twin y axis"));
}

fn shared_grid(x: Share, y: Share) -> Figure {
    let grid = Grid::new(1, 2)
        .unwrap()
        .with_share_x(x)
        .with_share_y(y)
        .with_legend(LegendMode::PerPanel);
    Figure::with_grid(9.0, 4.0, Theme::pigment(), grid).unwrap()
}

fn at(col: i64) -> Placement {
    Placement {
        col: GridIndex::At(col),
        ..Placement::default()
    }
}

#[test]
fn a_shared_axis_is_refused_at_the_call_when_its_panels_are_there() {
    for (x, y, needle) in [
        (Share::All, Share::None, "x is shared with another panel"),
        (Share::None, Share::Row, "y is shared with another panel"),
    ] {
        let mut f = shared_grid(x, y);
        f.add_axes_at(at(0), LINEAR, LINEAR).unwrap();
        let mut axes = f.add_axes_at(at(1), LINEAR, LINEAR).unwrap();
        let error = axes.equal_aspect().unwrap_err();
        assert!(usage_message(&error).contains(needle));
        assert!(!axes.has_equal_aspect());
    }
}

#[test]
fn a_panel_added_later_is_refused_when_rendering() {
    let mut f = shared_grid(Share::None, Share::Row);
    let mut axes = f.add_axes_at(at(0), LINEAR, LINEAR).unwrap();
    draw_circle(&mut axes, "");
    // Alone in its group, the axis is not shared yet.
    axes.equal_aspect().unwrap();
    let mut other = f.add_axes_at(at(1), LINEAR, LINEAR).unwrap();
    draw_circle(&mut other, "");
    let error = f.to_scene(lib()).unwrap_err();
    let message = usage_message(&error);
    assert!(message.contains("row 0, col 0: "), "{message}");
    assert!(
        message.contains("y is shared with another panel"),
        "{message}"
    );
}

#[test]
fn an_unshared_axis_of_a_sharing_grid_is_accepted() {
    // A column of its own under Share.COL has no other panel to follow.
    let mut f = shared_grid(Share::Col, Share::None);
    for col in [0, 1] {
        let mut axes = f.add_axes_at(at(col), LINEAR, LINEAR).unwrap();
        draw_circle(&mut axes, "");
    }
    f.axes_at_mut(GridIndex::Auto, GridIndex::At(1))
        .unwrap()
        .equal_aspect()
        .unwrap();
    let planned = try_plan_panels(&f, lib()).unwrap();
    assert_equal_scales(&planned[1].0, &planned[1].1);
}

#[test]
fn limits_that_leave_no_room_are_refused() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(LINEAR, LINEAR).unwrap();
    axes.xlim(0.0, 1.0e6).unwrap().ylim(0.0, 1.0).unwrap();
    axes.equal_aspect().unwrap();
    let error = f.to_scene(lib()).unwrap_err();
    assert!(matches!(error, Error::Style { .. }), "{error}");
    assert!(error.to_string().contains("equal aspect leaves the plot"));
    assert_eq!(
        error.hint(),
        Some("bring the x and y ranges closer together, or remove equal_aspect().")
    );
}

#[test]
fn text_at_plot_positions_uses_the_smaller_plot() {
    let mut f = circle_figure(9.0, 3.5, true, Theme::pigment());
    f.single_axes_mut()
        .text(
            "Middle",
            crate::TextPosition::InPlot(crate::InPlot::new(0.5, 0.5).unwrap()),
            crate::TextOptions {
                align: Align::Center,
                ..crate::TextOptions::default()
            },
        )
        .unwrap();
    let (_, layout) = plan_figure(&f, (0, 0), lib());
    let scene = f.to_scene(lib()).unwrap();
    let text = texts(&scene.items)
        .into_iter()
        .find(|text| text.text == "Middle")
        .unwrap();
    let centre = f64::from(text.origin.x) + f64::from(text.width) / 2.0;
    assert!((centre - (layout.plot.x + layout.plot.width / 2.0)).abs() < 1e-3);
}
