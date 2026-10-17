//! Bubble area scales, size legends, geometry and validation.

use crate::bubble_scale::{nice_125_below, nice_125_round, radius, references};
use crate::legend::{Legend, Swatch};
use crate::pipeline::{plan_figure, try_plan_panels};
use crate::resolve::{Limits, ResolvedMark, resolve_marks};
use crate::{
    AxisData, BubbleOptions, ColorBy, ColorSpec, Error, Figure, FontLibrary, Grid, GridIndex,
    Instant, Item, LegendMode, LegendPosition, LegendStyle, Mark, Marker, Paint, Placement,
    PointColor, Rgba, ScaleKind, SvgText, Theme, TickFormat,
};

fn options(label: &str) -> BubbleOptions {
    BubbleOptions {
        label: label.into(),
        ..BubbleOptions::default()
    }
}

fn figure(theme: Theme, maximum: f64) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .bubbles(
            &[1.0, 2.0, 3.0],
            &[2.0, 4.0, 3.0],
            &[maximum / 4.0, maximum, 0.0],
            options("Cities"),
        )
        .unwrap();
    f
}

#[test]
fn area_proportional_to_value() {
    let f = figure(Theme::pigment(), 400.0);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let Item::Group { items, .. } = &scene.items[1] else {
        panic!("marks")
    };
    let radii: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => {
                let (lo, hi) = path.bounds().unwrap();
                Some((hi.x - lo.x) / 2.0)
            }
            _ => None,
        })
        .collect();
    assert_eq!(radii.len(), 2);
    for (radius, expected) in radii.iter().zip([9.0, 18.0]) {
        assert!((radius - expected).abs() < 1e-5, "{radii:?}");
    }
    assert!(((radii[1] / radii[0]).powi(2) - 4.0).abs() < 1e-5);
    assert!(items.chunks_exact(2).all(|pair| matches!((&pair[0], &pair[1]),
        (Item::Fill { .. }, Item::Stroke { style, .. }) if style.width == 0.8 && style.color == f.theme().background())));
}

#[test]
fn zero_size_positions_still_count_in_axis_domains() {
    let make = |size| {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .bubbles(&[1.0, 1000.0], &[2.0, -1000.0], &[1.0, size], options(""))
            .unwrap();
        f
    };
    let zero = make(0.0);
    let visible = make(1.0);
    let zero_axes = plan_figure(&zero, (0, 0), FontLibrary::global()).0;
    let visible_axes = plan_figure(&visible, (0, 0), FontLibrary::global()).0;
    // Both positions contribute, but only visible circles need radius-aware limits.
    for axes in [zero_axes, visible_axes] {
        assert!(axes.x.continuous().unwrap().domain().1 >= 1000.0);
        assert!(axes.y.continuous_scale().domain().0 <= -1000.0);
    }
    let scene = zero.to_scene(FontLibrary::global()).unwrap();
    let Item::Group { items, .. } = &scene.items[1] else {
        panic!("marks")
    };
    assert_eq!(items.len(), 2);
}

#[test]
fn best_avoids_bubble_area_when_its_center_is_outside() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .legend_at(LegendStyle::Header, LegendPosition::Best)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .bubbles(&[2.0], &[2.0], &[1.0], options("Cities"))
        .unwrap();
    let initial = plan_figure(&f, (0, 0), FontLibrary::global()).1;
    let plot = initial.plot;
    let first = initial.legend.background.unwrap();
    let center_x = first.x - 9.0;
    let center_y = first.y + first.height / 2.0;
    f.single_axes_mut()
        .bubbles(
            &[(center_x - plot.x) / plot.width],
            &[(plot.bottom() - center_y) / plot.height],
            &[1.0],
            options(""),
        )
        .unwrap();
    let (resolved, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    let item = crate::draw::marks(f.theme(), &resolved, &layout, &marks[1..]).unwrap();
    let geometry = crate::coverage::Geometry::of(&item);
    let candidate = crate::layout::Frame {
        x: first.x,
        y: first.y,
        width: first.width,
        height: first.height,
    };
    assert!(center_x < candidate.x);
    assert!(geometry.coverage(&plot, &candidate) > 0.0);
    let chosen = layout.legend.background.unwrap();
    assert!((chosen.x - (plot.x + 6.0)).abs() < 1e-7);
    assert!((chosen.y - (plot.y + 6.0)).abs() < 1e-7);
}

#[test]
fn size_legend_containment_errors_include_circle_height() {
    for style in [LegendStyle::Header, LegendStyle::Chips] {
        for position in [LegendPosition::Right, LegendPosition::Best] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            axes.legend_at(style, position)
                .unwrap()
                .xlim(0.0, 1.0)
                .unwrap()
                .ylim(0.0, 1.0)
                .unwrap();
            for i in 0..4 {
                axes.bubbles(&[0.5], &[0.5], &[1.0], options(&format!("Cities {i}")))
                    .unwrap();
            }
            let err = f.to_scene(FontLibrary::global()).unwrap_err();
            let available = if position == LegendPosition::Right {
                "the plot has 243 pt"
            } else {
                "the plot leaves 219 pt for the legend"
            };
            assert_eq!(
                err.to_string(),
                format!("Figure.to_scene(): the legend is 282 pt tall; {available}.")
            );
            assert!(matches!(err, Error::Style { .. }));
            assert_eq!(err.parameter(), None);
            assert_eq!(
                err.hint(),
                Some("make the figure taller, or use fewer legend entries.")
            );
        }
    }
}

#[test]
fn size_legend_values_are_nice() {
    assert_eq!(references(870.0), [500.0, 200.0, 100.0]);
    assert_eq!(references(1.0), [1.0, 0.5, 0.2]);
    assert_eq!(nice_125_below(870.0), 500.0);
    assert_eq!(nice_125_round(250.0), 200.0);
    for maximum in [
        f64::from_bits(1),
        1e-320,
        1e-300,
        0.01,
        1.0,
        870.0,
        1e300,
        f64::MAX,
    ] {
        let top = nice_125_below(maximum);
        assert!(top > 0.0 && top <= maximum, "{maximum}: {top}");
        let refs = references(maximum);
        assert!(
            refs.iter()
                .all(|v| v.is_finite() && *v >= 0.0 && *v <= maximum)
        );
        assert_eq!(radius(maximum, maximum, 18.0), 18.0);
        assert!(refs.iter().all(|v| radius(*v, maximum, 18.0).is_finite()));
    }
}

#[test]
fn bubble_sizes_above_the_magnitude_limit_are_atomic() {
    let mut f = figure(Theme::pigment(), 1e300);
    let before = f.to_svg(SvgText::Outlines).unwrap();
    for value in [1.1e300, f64::MAX] {
        let error = f
            .single_axes_mut()
            .bubbles(&[1.0], &[2.0], &[value], options("Large"))
            .unwrap_err();
        assert!(matches!(error, Error::Data { .. }));
        assert_eq!(error.parameter(), Some("sizes"));
        assert_eq!(error.hint(), None);
        assert_eq!(
            error.to_string(),
            format!(
                "Axes.bubbles(): sizes[0] is {}; magnitudes must not exceed 1e300.",
                crate::error::number(value)
            )
        );
        assert_eq!(f.to_svg(SvgText::Outlines).unwrap(), before);
    }
}

#[test]
fn subnormal_size_references_are_positive_and_unique() {
    assert!(references(0.0).is_empty());
    for bits in 1..=8 {
        let maximum = f64::from_bits(bits);
        let refs = references(maximum);
        assert!(!refs.is_empty());
        assert!(refs.iter().all(|v| *v > 0.0 && *v <= maximum), "{refs:?}");
        assert!(refs.windows(2).all(|p| p[0] > p[1]), "{refs:?}");
    }
}

#[test]
fn subnormal_size_legends_have_no_zero_or_repeated_entries() {
    for bits in 1..=8 {
        let maximum = f64::from_bits(bits);
        let f = figure(Theme::pigment(), maximum);
        let marks = resolve_marks(f.single_axes().marks(), f.theme());
        for format in [
            TickFormat::default(),
            TickFormat::fixed(0).unwrap(),
            TickFormat::percent(0).unwrap(),
        ] {
            let entries = marks[0].size_legend(&format);
            assert!(!entries.is_empty());
            for (i, entry) in entries.iter().enumerate() {
                assert_ne!(entry.label(), "0");
                assert_ne!(entry.label(), "0%");
                assert!(entries[..i].iter().all(|e| e.label() != entry.label()));
                assert!(matches!(entry.swatch(), Swatch::Bubble { radius } if radius > 0.0));
            }
        }
    }
}

#[test]
fn bubble_maximum_ignores_nan_coordinates_across_marks() {
    let mut f = figure(Theme::pigment(), 100.0);
    f.single_axes_mut()
        .bubbles(
            &[f64::NAN, 4.0],
            &[4.0, f64::NAN],
            &[1e300, 1e300],
            options("Missing"),
        )
        .unwrap();
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    for mark in &marks {
        assert!(
            matches!(mark, ResolvedMark::Bubbles { maximum: 100.0, .. }),
            "{mark:?}"
        );
    }
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let Item::Group { items, .. } = &scene.items[1] else {
        panic!("marks")
    };
    let radii: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => {
                let (lo, hi) = path.bounds().unwrap();
                Some((hi.x - lo.x) / 2.0)
            }
            _ => None,
        })
        .collect();
    assert_eq!(radii.len(), 2);
    for (radius, expected) in radii.iter().zip([9.0, 18.0]) {
        assert!((radius - expected).abs() < 1e-5, "{radii:?}");
    }
}

#[test]
fn several_bubble_marks_share_one_scale() {
    let mut f = figure(Theme::pigment(), 100.0);
    f.single_axes_mut()
        .bubbles(&[4.0], &[4.0], &[400.0], options("Towns"))
        .unwrap();
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    for mark in &marks {
        assert!(matches!(
            mark,
            ResolvedMark::Bubbles {
                maximum: 400.0,
                radius: 18.0,
                ..
            }
        ));
    }
    for style in [LegendStyle::Header, LegendStyle::Chips, LegendStyle::Direct] {
        let legend = Legend::new(style, &marks, &Limits::of(f.single_axes()));
        let labels: Vec<_> = legend.row().iter().map(|e| e.label()).collect();
        assert_eq!(labels, ["Cities", "200", "100", "50", "Towns"]);
        assert_ne!(legend.row()[0].color(), legend.row()[4].color());
        for (entry, value) in legend.row()[1..4].iter().zip(references(400.0)) {
            assert_eq!(entry.color(), legend.row()[0].color());
            assert_eq!(
                entry.swatch(),
                Swatch::Bubble {
                    radius: radius(value, 400.0, 18.0)
                }
            );
        }
    }
}

#[test]
fn size_legend_in_every_position() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for style in [LegendStyle::Header, LegendStyle::Chips] {
            for position in [
                LegendPosition::TopLeft,
                LegendPosition::TopRight,
                LegendPosition::Right,
                LegendPosition::Bottom,
                LegendPosition::Best,
            ] {
                let mut f = figure(theme.clone(), 870.0);
                f.single_axes_mut().legend_at(style, position).unwrap();
                let layout = plan_figure(&f, (0, 0), FontLibrary::global()).1;
                let entries = layout.legend.entries();
                assert_eq!(entries.len(), 4);
                assert_eq!(
                    entries
                        .iter()
                        .map(|e| e.label.text.as_str())
                        .collect::<Vec<_>>(),
                    ["Cities", "500", "200", "100"]
                );
                for (entry, value) in entries[1..].iter().zip(references(870.0)) {
                    let r = radius(value, 870.0, 18.0);
                    assert_eq!(entry.swatch.kind, Swatch::Bubble { radius: r });
                    assert_eq!(
                        entry.swatch.width,
                        2.0 * f64::from(r) + f64::from(crate::draw::MARKER_OUTLINE)
                    );
                    assert_eq!(entry.swatch.width, entry.swatch.height);
                    assert!(entry.chip.is_none());
                    assert!(entry.label.x >= entry.swatch.x + entry.swatch.width + 4.0);
                }
                if matches!(position, LegendPosition::Right | LegendPosition::Best) {
                    assert!(
                        entries
                            .windows(2)
                            .all(|p| p[1].swatch.center_y - p[0].swatch.center_y
                                >= entries[1].swatch.height + 4.0)
                    );
                }
                let Item::Group { items, .. } =
                    crate::draw::legend::legend(f.theme(), &layout).unwrap()
                else {
                    panic!("legend")
                };
                let circles: Vec<_> = items
                    .iter()
                    .filter_map(|item| match item {
                        Item::Markers {
                            shape,
                            positions,
                            stroke: Some(stroke),
                            ..
                        } => Some((shape, positions, stroke)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(circles.len(), 3);
                for ((shape, positions, stroke), entry) in circles.iter().zip(&entries[1..]) {
                    let (lo, hi) = shape.bounds().unwrap();
                    let center = positions[0];
                    let half = f64::from(stroke.width) / 2.0;
                    let tolerance = 1e-4;
                    assert!(f64::from(center.x + lo.x) - half >= entry.swatch.x - tolerance);
                    assert!(
                        f64::from(center.x + hi.x) + half
                            <= entry.swatch.x + entry.swatch.width + tolerance
                    );
                    assert!(
                        f64::from(center.y + lo.y) - half
                            >= entry.swatch.center_y - entry.swatch.height / 2.0 - tolerance
                    );
                    assert!(
                        f64::from(center.y + hi.y) + half
                            <= entry.swatch.center_y + entry.swatch.height / 2.0 + tolerance
                    );
                }
                let scene = f.to_scene(FontLibrary::global()).unwrap();
                crate::test_support::scene::assert_scene_bounded(&scene);
            }
        }
    }
}

#[test]
fn size_legend_uses_y_tick_format_and_direct_fallback() {
    let mut f = figure(Theme::graphite(), 1.0);
    f.single_axes_mut()
        .ytick_format(
            TickFormat::percent(0)
                .unwrap()
                .affix("~", " share")
                .unwrap(),
        )
        .unwrap();
    let layout = plan_figure(&f, (0, 0), FontLibrary::global()).1;
    assert!(layout.direct.is_empty());
    assert_eq!(
        layout
            .legend
            .entries()
            .iter()
            .map(|e| e.label.text.as_str())
            .collect::<Vec<_>>(),
        ["Cities", "~100% share", "~50% share", "~20% share"]
    );
}

#[test]
fn size_legend_formats_keep_nonzero_distinct_labels() {
    for (maximum, format, labels, values) in [
        (1.0, TickFormat::fixed(0).unwrap(), vec!["1"], vec![1.0]),
        (0.9, TickFormat::fixed(0).unwrap(), vec!["0.5"], vec![0.5]),
        (
            0.9,
            TickFormat::fixed(0).unwrap().affix("~", " units").unwrap(),
            vec!["0.5"],
            vec![0.5],
        ),
        (
            0.09,
            TickFormat::fixed(2).unwrap(),
            vec!["0.05", "0.02", "0.01"],
            vec![0.05, 0.02, 0.01],
        ),
        (
            0.01,
            TickFormat::percent(0)
                .unwrap()
                .affix("~", " share")
                .unwrap(),
            vec!["~1% share"],
            vec![0.01],
        ),
        (
            0.009,
            TickFormat::percent(0)
                .unwrap()
                .affix("~", " share")
                .unwrap(),
            vec!["0.005"],
            vec![0.005],
        ),
    ] {
        let f = figure(Theme::pigment(), maximum);
        let marks = resolve_marks(f.single_axes().marks(), f.theme());
        let entries = marks[0].size_legend(&format);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label())
                .collect::<Vec<_>>(),
            labels,
            "maximum {maximum}, format {format:?}"
        );
        for (entry, value) in entries.iter().zip(values) {
            assert_eq!(
                entry.swatch(),
                Swatch::Bubble {
                    radius: radius(value, maximum, f.theme().bubble_max_radius()),
                }
            );
        }
    }
}

#[test]
fn merged_legend_keeps_references_with_the_first_label() {
    let mut f = Figure::with_grid(
        10.0,
        5.0,
        Theme::midnight(),
        Grid::new(1, 2).unwrap().with_legend(LegendMode::Merged),
    )
    .unwrap();
    for col in 0..2 {
        let mut axes = f
            .add_axes_at(
                Placement {
                    row: GridIndex::At(0),
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        axes.bubbles(&[1.0], &[2.0], &[1.0], options(""))
            .unwrap()
            .bubbles(
                &[1.0, 2.0],
                &[2.0, 3.0],
                &[100.0, if col == 0 { 870.0 } else { 1.0 }],
                options("Cities"),
            )
            .unwrap();
        if col == 1 {
            axes.bubbles(&[2.0], &[2.0], &[50.0], options("Towns"))
                .unwrap();
        }
    }
    let marks = crate::resolve::resolve_figure_marks(&f);
    for ((axes, panel_marks), expected) in f.panels().zip(&marks).zip([870.0, 100.0]) {
        assert!(panel_marks.iter().all(|mark| matches!(
            mark,
            ResolvedMark::Bubbles { maximum, .. } if *maximum == expected
        )));
        let panel = Legend::new(LegendStyle::Header, panel_marks, &Limits::of(axes));
        for (entry, value) in panel.row()[1..4].iter().zip(references(expected)) {
            assert_eq!(
                entry.swatch(),
                Swatch::Bubble {
                    radius: radius(value, expected, 18.0)
                }
            );
        }
    }
    let legend = Legend::merged(&f, marks.iter().map(Vec::as_slice));
    assert_eq!(
        legend.row().iter().map(|e| e.label()).collect::<Vec<_>>(),
        ["Cities", "500", "200", "100", "Towns"]
    );
    assert!(try_plan_panels(&f, FontLibrary::global()).is_ok());
    crate::test_support::scene::assert_scene_bounded(&f.to_scene(FontLibrary::global()).unwrap());
}

#[test]
fn empty_zero_and_unlabelled_scales_have_no_reference_circles() {
    for sizes in [&[][..], &[0.0][..]] {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let x = vec![1.0; sizes.len()];
        a.bubbles(&x, &x, sizes, options("Empty")).unwrap();
        let layout = plan_figure(&f, (0, 0), FontLibrary::global()).1;
        assert_eq!(layout.legend.entries().len(), 1);
    }
    for style in [
        LegendStyle::Header,
        LegendStyle::Direct,
        LegendStyle::Chips,
        LegendStyle::None,
    ] {
        let mut f = figure(Theme::pigment(), 1.0);
        f.single_axes_mut().legend(style);
        let layout = plan_figure(&f, (0, 0), FontLibrary::global()).1;
        assert_eq!(
            layout.legend.entries().len(),
            if style == LegendStyle::None { 0 } else { 4 }
        );
    }
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .bubbles(&[1.0], &[1.0], &[1.0], options(""))
        .unwrap();
    assert!(
        plan_figure(&f, (0, 0), FontLibrary::global())
            .1
            .legend
            .is_empty()
    );
}

#[test]
fn invalid_bubbles_are_atomic_and_name_the_first_size() {
    let mut f = figure(Theme::pigment(), 1.0);
    let before = f.clone();
    for (value, text) in [
        (-1.0, "-1"),
        (f64::NAN, "NaN"),
        (f64::INFINITY, "inf"),
        (f64::NEG_INFINITY, "-inf"),
    ] {
        let err = f
            .single_axes_mut()
            .bubbles(
                &[0.0, 1.0, 2.0],
                &[1.0; 3],
                &[0.0, value, -2.0],
                options(""),
            )
            .unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(
            err.to_string(),
            format!(
                "Axes.bubbles(): sizes[1] is {text}; bubble sizes must be finite and at least 0."
            )
        );
        assert_eq!(err.parameter(), Some("sizes"));
        assert_eq!(err.hint(), Some("use 0 for a point to leave out."));
        assert_eq!(f, before);
    }
    for (y, sizes, message) in [
        (
            &[][..],
            &[][..],
            "x has 1 values but y has 0; x and y must have the same length.",
        ),
        (
            &[1.0][..],
            &[][..],
            "x has 1 values but sizes has 0; x and sizes must have the same length.",
        ),
    ] {
        let err = f
            .single_axes_mut()
            .bubbles(&[f64::INFINITY], y, sizes, options(""))
            .unwrap_err();
        assert_eq!(err.to_string(), format!("Axes.bubbles(): {message}"));
        assert_eq!((err.parameter(), err.hint()), (None, None));
        assert_eq!(f, before);
    }
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    let before = f.clone();
    let err = f
        .single_axes_mut()
        .bubbles(&[], &[], &[], options(""))
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }));
    assert_eq!(
        err.to_string(),
        "Axes.bubbles(): this is a category axis; bubbles apply to linear and log axes only."
    );
    assert_eq!(err.parameter(), Some("x"));
    assert_eq!(
        err.hint(),
        Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().")
    );
    assert_eq!(f, before);
}

#[test]
fn bubbles_keep_order_options_and_shared_copies() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let opts = BubbleOptions {
        label: "Data".into(),
        color: crate::PointColor::Uniform(ColorSpec::Fixed(Rgba::RED)),
        marker: Marker::Diamond,
    };
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .bubbles(
            &[3.0, 1.0, 2.0],
            &[2.0, 3.0, 1.0],
            &[400.0, 100.0, 0.0],
            opts.clone(),
        )
        .unwrap();
    let copy = f.clone();
    let Mark::Bubbles(a) = &f.single_axes().marks()[0] else {
        panic!("bubbles")
    };
    let Mark::Bubbles(b) = &copy.single_axes().marks()[0] else {
        panic!("bubbles")
    };
    assert!(
        std::ptr::eq(a.x(), b.x())
            && std::ptr::eq(a.y(), b.y())
            && std::ptr::eq(a.sizes(), b.sizes())
    );
    assert_eq!(a.options(), &opts);
    assert_eq!(a.x(), &[3.0, 1.0, 2.0]);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let Item::Group { items, .. } = &scene.items[1] else {
        panic!("marks")
    };
    let boxes: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => path.bounds(),
            _ => None,
        })
        .collect();
    assert_eq!(boxes.len(), 2);
    assert!(boxes[0].0.x > boxes[1].1.x);
    assert!(((boxes[0].1.x - boxes[0].0.x) / (boxes[1].1.x - boxes[1].0.x) - 2.0).abs() < 1e-5);
    assert_eq!(
        f.to_svg(SvgText::Outlines).unwrap(),
        copy.to_svg(SvgText::Outlines).unwrap()
    );
}

/// `n` days after 1970-01-01, as a present time.
fn day(n: i64) -> Option<Instant> {
    Some(Instant::from_micros(n * 86_400_000_000).unwrap())
}

#[test]
fn time_bubbles_map_colors_and_label_their_size_references_as_numbers() {
    for (x, y) in [
        (ScaleKind::Time, ScaleKind::Linear),
        (ScaleKind::Linear, ScaleKind::Time),
        (ScaleKind::Time, ScaleKind::Time),
    ] {
        let along = |kind: ScaleKind, numbers: [f64; 3]| {
            if kind == ScaleKind::Time {
                AxisData::Times(vec![day(0), day(3), day(9)])
            } else {
                AxisData::Numbers(numbers.to_vec())
            }
        };
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(x, y)
            .unwrap()
            .bubbles_time(
                along(x, [1.0, 2.0, 3.0]),
                along(y, [3.0, 1.0, 2.0]),
                [100.0, 400.0, 900.0].into(),
                BubbleOptions {
                    label: "Cities".into(),
                    color: PointColor::By(ColorBy::new(vec![0.1, 0.5, 0.9])),
                    ..BubbleOptions::default()
                },
            )
            .unwrap();
        // Each bubble takes its own color.
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        crate::test_support::scene::assert_scene_bounded(&scene);
        let mut fills: Vec<Rgba> = scene
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Group {
                    clip: Some(_),
                    items,
                } => Some(items),
                _ => None,
            })
            .flatten()
            .filter_map(|item| match item {
                Item::Fill {
                    paint: Paint::Solid(color),
                    ..
                } => Some(*color),
                _ => None,
            })
            .collect();
        assert_eq!(fills.len(), 3, "{x:?} {y:?}");
        fills.dedup();
        assert_eq!(fills.len(), 3, "{x:?} {y:?}");
        // The legend names the series and shows three size references, labelled as numbers
        // whatever the axes are.
        let marks = resolve_marks(f.single_axes().marks(), f.theme());
        let legend = Legend::new(LegendStyle::Header, &marks, &Limits::of(f.single_axes()));
        let labels: Vec<_> = legend.row().iter().map(|e| e.label()).collect();
        assert_eq!(labels, ["Cities", "500", "200", "100"], "{x:?} {y:?}");
        let marks = [marks];
        let merged = Legend::merged(&f, marks.iter().map(Vec::as_slice));
        let labels: Vec<_> = merged.row().iter().map(|e| e.label()).collect();
        assert_eq!(labels, ["Cities", "500", "200", "100"], "{x:?} {y:?}");
    }
}

#[test]
fn time_bubble_size_references_ignore_the_time_format_of_the_y_axis() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Time)
        .unwrap()
        .ytick_format(TickFormat::iso())
        .unwrap()
        .bubbles_time(
            AxisData::Numbers(vec![1.0, 2.0]),
            AxisData::Times(vec![day(0), day(1)]),
            [100.0, 400.0].into(),
            options("Cities"),
        )
        .unwrap();
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    let legend = Legend::new(LegendStyle::Header, &marks, &Limits::of(f.single_axes()));
    let labels: Vec<_> = legend.row().iter().map(|e| e.label()).collect();
    assert_eq!(labels, ["Cities", "200", "100", "50"]);
    assert!(f.to_scene(FontLibrary::global()).is_ok());
}

#[test]
fn time_bubble_color_errors_change_nothing() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Time, ScaleKind::Linear).unwrap();
    let before = f.clone();
    let error = f
        .single_axes_mut()
        .bubbles_time(
            AxisData::Times(vec![day(0), day(1), day(2)]),
            AxisData::Numbers(vec![1.0, 2.0, 3.0]),
            [1.0, 2.0, 3.0].into(),
            BubbleOptions {
                color: PointColor::By(ColorBy::new(vec![0.5])),
                ..BubbleOptions::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, Error::Data { .. }));
    assert_eq!(error.parameter(), Some("color"));
    assert_eq!(f, before);
}
