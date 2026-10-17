//! Box geometry, domain, clipping, degenerate groups and grouped-bar spacing.

use super::{plot_of, subpaths};
use crate::draw::bar::group_slots;
use crate::draw::fill::AREA_ALPHA;
use crate::legend::{Legend, Swatch};
use crate::model::{BoxOptions, Distribution, Figure};
use crate::resolve::{Limits, ResolvedMark, resolve_marks};
use crate::scene::{Item, LineCap, Paint};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, LegendStyle, LogBase, ScaleKind, SizeSpec, Theme};

fn figure(theme: Theme, y: ScaleKind) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Category, y).unwrap();
    f
}

fn items(f: &Figure) -> Vec<Item> {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    super::mark_items(&scene).to_vec()
}

#[track_caller]
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-3, "{a} vs {b}");
}

#[test]
fn box_geometry_matches_the_theme_ratios() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = figure(theme.clone(), ScaleKind::Linear);
        f.single_axes_mut()
            .ylim(-2.0, 14.0)
            .unwrap()
            .box_plot(
                vec!["A".into()],
                vec![vec![0.0, 1.0, 2.0, 3.0, 4.0, 12.0]],
                BoxOptions::default(),
            )
            .unwrap();
        let plot = plot_of(&f);
        let drawn = items(&f);
        let [
            Item::Fill {
                path: fill, paint, ..
            },
            Item::Stroke {
                path: outline,
                style,
            },
            Item::Stroke {
                path: median,
                style: median_style,
            },
            Item::Markers {
                shape, positions, ..
            },
        ] = drawn.as_slice()
        else {
            panic!("fill, outline, median and outliers expected: {drawn:?}")
        };
        let (a, b) = fill.bounds().unwrap();
        let width = plot.width * 0.5;
        near(f64::from(b.x - a.x), width);
        near(f64::from(a.x), plot.x + plot.width * 0.25);
        let y = |value: f64| plot.bottom() - (value + 2.0) / 16.0 * plot.height;
        near(f64::from(a.y), y(3.75));
        near(f64::from(b.y), y(1.25));
        assert_eq!(style.width, theme.drawn_line_width(SizeSpec::Auto) * 0.5);
        assert_eq!(median_style.width, theme.drawn_line_width(SizeSpec::Auto));
        assert_eq!(median_style.cap, LineCap::Butt);
        assert_eq!(*paint, Paint::Solid(style.color.scale_alpha(AREA_ALPHA)));
        let segments = subpaths(outline);
        assert_eq!(segments.len(), 5);
        for i in [1, 3] {
            near(f64::from(segments[i][1].x - segments[i][0].x), width * 0.5);
        }
        near(f64::from(segments[1][0].y), y(0.0));
        near(f64::from(segments[3][0].y), y(4.0));
        near(f64::from(subpaths(median)[0][0].y), y(2.5));
        near(
            f64::from(subpaths(median)[0][1].x - subpaths(median)[0][0].x),
            width,
        );
        assert_eq!(positions.len(), 1);
        near(f64::from(positions[0].y), y(12.0));
        let (a, b) = shape.bounds().unwrap();
        near(
            f64::from(b.x - a.x),
            f64::from(theme.drawn_marker_size(SizeSpec::Auto) * 0.8),
        );
    }
}

#[test]
fn box_coverage_counts_fills_whiskers_medians_and_outliers() {
    for grouped in [false, true] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        let mut axes = f.single_axes_mut();
        axes.legend_at(LegendStyle::Header, crate::LegendPosition::Best)
            .unwrap()
            .ylim(-2.0, 14.0)
            .unwrap();
        let samples = vec![0.0, 1.0, 2.0, 3.0, 4.0, 12.0];
        if grouped {
            axes.grouped_box_plot(
                vec!["A".into()],
                ["North", "South"]
                    .into_iter()
                    .map(|label| Distribution {
                        label: label.into(),
                        groups: vec![samples.clone()],
                        ..Default::default()
                    })
                    .collect(),
            )
            .unwrap();
        } else {
            axes.box_plot(
                vec!["A".into()],
                vec![samples],
                BoxOptions {
                    label: "North".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let marks = resolve_marks(f.single_axes().marks(), f.theme());
        let item = crate::draw::marks(f.theme(), &resolved, &layout, &marks).unwrap();
        let prepared = marks[0].for_coverage(Some(crate::coverage::Geometry::of(&item)));
        let score = |x, y| {
            let candidate = crate::layout::Frame {
                x,
                y,
                width: 2.0,
                height: 2.0,
            };
            assert_eq!(marks[0].coverage(&layout.plot, &candidate), 0.0);
            prepared.coverage(&layout.plot, &candidate)
        };
        let Item::Group { items, .. } = item else {
            panic!("box group expected")
        };
        assert_eq!(items.len(), if grouped { 8 } else { 4 });
        for group in items.chunks_exact(4) {
            let [
                Item::Fill { path: fill, .. },
                Item::Stroke { path: outline, .. },
                Item::Stroke { path: median, .. },
                Item::Markers { positions, .. },
            ] = group
            else {
                panic!("fill, outline, median and outlier expected")
            };
            let (a, _) = fill.bounds().unwrap();
            assert_eq!(score(f64::from(a.x) + 2.0, f64::from(a.y) + 2.0), 4.0);
            let segments = subpaths(outline);
            let whisker = &segments[0];
            assert_eq!(
                score(
                    f64::from(whisker[0].x) - 1.0,
                    (f64::from(whisker[0].y) + f64::from(whisker[1].y)) / 2.0 - 1.0,
                ),
                1.0
            );
            let cap = &segments[1];
            assert_eq!(
                score(
                    f64::from(cap[0].x) * 0.75 + f64::from(cap[1].x) * 0.25 - 1.0,
                    f64::from(cap[0].y) - 1.0,
                ),
                1.0
            );
            let (a, b) = median.bounds().unwrap();
            assert_eq!(
                score(
                    (f64::from(a.x) + f64::from(b.x)) / 2.0 - 1.0,
                    f64::from(a.y) - 1.0,
                ),
                5.0
            );
            assert_eq!(
                score(
                    f64::from(positions[0].x) - 1.0,
                    f64::from(positions[0].y) - 1.0
                ),
                1.0
            );
        }
        assert_eq!(score(layout.plot.right() + 1.0, layout.plot.y), 0.0);
    }
}

#[test]
fn boxes_with_inliers_inside_the_box_keep_their_geometry() {
    for (samples, q1, median, q3, outlier) in [
        ([0.0, 100.0, 100.0, 100.0], 75.0, 100.0, 100.0, 0.0),
        ([0.0, 0.0, 0.0, 100.0], 0.0, 0.0, 25.0, 100.0),
        ([0.0, 90.0, 100.0, 100.0], 67.5, 95.0, 100.0, 0.0),
        ([0.0, 0.0, 10.0, 100.0], 0.0, 5.0, 32.5, 100.0),
    ] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        f.single_axes_mut()
            .ylim(-10.0, 110.0)
            .unwrap()
            .box_plot(
                vec!["A".into()],
                vec![samples.to_vec()],
                BoxOptions::default(),
            )
            .unwrap();
        let plot = plot_of(&f);
        let y = |value: f64| plot.bottom() - (value + 10.0) / 120.0 * plot.height;
        let drawn = items(&f);
        let [
            Item::Fill { path: fill, .. },
            Item::Stroke { path: outline, .. },
            Item::Stroke { path: line, .. },
            Item::Markers { positions, .. },
        ] = drawn.as_slice()
        else {
            panic!("fill, outline, median and outlier expected: {drawn:?}")
        };
        let (a, b) = fill.bounds().unwrap();
        near(f64::from(a.y), y(q3));
        near(f64::from(b.y), y(q1));
        near(f64::from(b.x - a.x), plot.width * 0.5);
        let segments = subpaths(outline);
        assert_eq!(segments.len(), 5);
        assert_eq!(segments[4], subpaths(fill)[0]);
        // Both stems and their caps stay on the box edges, never in its interior.
        for (segment, quartile) in segments[..4].iter().zip([q1, q1, q3, q3]) {
            for point in segment {
                near(f64::from(point.y), y(quartile));
            }
        }
        let median_line = subpaths(line);
        assert_eq!(median_line.len(), 1);
        for point in &median_line[0] {
            near(f64::from(point.y), y(median));
        }
        near(
            f64::from(median_line[0][1].x - median_line[0][0].x),
            plot.width * 0.5,
        );
        assert_eq!(positions.len(), 1);
        near(f64::from(positions[0].x), plot.x + plot.width * 0.5);
        near(f64::from(positions[0].y), y(outlier));
    }
}

#[test]
fn boxes_include_whiskers_and_outliers_without_forcing_zero() {
    for sign in [-1.0, 1.0] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        f.single_axes_mut()
            .box_plot(
                vec!["A".into()],
                vec![
                    [10.0, 11.0, 12.0, 13.0, 14.0, 20.0]
                        .map(|v| v * sign)
                        .to_vec(),
                ],
                BoxOptions::default(),
            )
            .unwrap();
        let (resolved, _) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let (lo, hi) = resolved.y.continuous_scale().domain();
        assert!((lo..=hi).contains(&(10.0 * sign)) && (lo..=hi).contains(&(20.0 * sign)));
        assert!(!(lo..=hi).contains(&0.0), "{lo}..{hi}");
    }
}

#[test]
fn box_degenerate_groups() {
    let mut f = figure(Theme::pigment(), ScaleKind::Linear);
    f.single_axes_mut()
        .box_plot(
            ["Empty", "NaN", "One", "Equal"].map(String::from).to_vec(),
            vec![vec![], vec![f64::NAN], vec![2.0], vec![4.0, 4.0]],
            BoxOptions::default(),
        )
        .unwrap();
    let drawn = items(&f);
    assert_eq!(drawn.len(), 2);
    for item in drawn {
        let Item::Stroke { path, style } = item else {
            panic!("median only")
        };
        assert_eq!(subpaths(&path).len(), 1);
        assert_eq!(style.width, f.theme().drawn_line_width(SizeSpec::Auto));
    }
    let mut empty = figure(Theme::pigment(), ScaleKind::Linear);
    empty
        .single_axes_mut()
        .box_plot(
            vec!["A".into()],
            vec![vec![f64::NAN]],
            BoxOptions::default(),
        )
        .unwrap();
    assert!(items(&empty).is_empty());
    let start = std::time::Instant::now();
    let samples = (0..1_000_000).rev().map(f64::from).collect();
    let mut large = figure(Theme::pigment(), ScaleKind::Linear);
    large
        .single_axes_mut()
        .box_plot(vec!["Million".into()], vec![samples], BoxOptions::default())
        .unwrap();
    assert_eq!(items(&large).len(), 3);
    println!(
        "box_degenerate_groups: 1000000 samples rendered in {:?}",
        start.elapsed()
    );
}

#[test]
fn grouped_boxes_share_bar_gaps() {
    for n in [1, 2, 5, 100] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        f.single_axes_mut()
            .grouped_box_plot(
                vec!["A".into()],
                (0..n)
                    .map(|_| Distribution {
                        groups: vec![vec![1.0, 2.0, 3.0]],
                        ..Default::default()
                    })
                    .collect(),
            )
            .unwrap();
        let plot = plot_of(&f);
        let fills: Vec<_> = items(&f)
            .iter()
            .filter_map(|item| match item {
                Item::Fill { path, .. } => path.bounds(),
                _ => None,
            })
            .collect();
        assert_eq!(fills.len(), n);
        let total = plot.width * 0.5;
        let (slot, gap) = group_slots(total, n, 2.0);
        near(f64::from(fills[0].0.x), plot.x + plot.width * 0.25);
        near(f64::from(fills[n - 1].1.x - fills[0].0.x), total);
        for (i, (a, b)) in fills.iter().enumerate() {
            near(f64::from(b.x - a.x), slot);
            if i > 0 {
                near(f64::from(a.x - fills[i - 1].1.x), gap);
            }
        }
        assert_eq!(gap, if n == 100 { 0.0 } else { 2.0 });
    }
}

#[test]
fn log_boxes_compute_statistics_before_transforming_and_clip_extremes() {
    for y in [ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN }] {
        let mut f = figure(Theme::midnight(), y);
        f.single_axes_mut()
            .box_plot(
                vec!["A".into()],
                vec![vec![1.0, 2.0, 4.0, 8.0]],
                BoxOptions::default(),
            )
            .unwrap();
        let marks = resolve_marks(f.single_axes().marks(), f.theme());
        let [ResolvedMark::Box { distributions, .. }] = marks.as_slice() else {
            panic!("box expected")
        };
        let stats = distributions[0].groups[0].as_ref().unwrap();
        assert_eq!((stats.q1, stats.median, stats.q3), (1.75, 3.0, 5.0));
        let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let drawn = items(&f);
        let Item::Stroke { path, .. } = &drawn[2] else {
            panic!("median expected")
        };
        near(
            f64::from(subpaths(path)[0][0].y),
            layout.plot.bottom()
                - resolved.y.continuous_scale().normalize(3.0) * layout.plot.height,
        );
        let mut extreme = figure(Theme::pigment(), y);
        let samples = if y == ScaleKind::Linear {
            vec![-1e300, -1e299, 1e299, 1e300]
        } else {
            vec![1e-300, 1.0, 1e299, 1e300]
        };
        extreme
            .single_axes_mut()
            .ylim(1.0, 10.0)
            .unwrap()
            .box_plot(vec!["A".into()], vec![samples], BoxOptions::default())
            .unwrap();
        let _ = items(&extreme);
    }
}

#[test]
fn boxes_follow_axis_category_order_without_reordering_samples() {
    let mut f = figure(Theme::pigment(), ScaleKind::Linear);
    f.single_axes_mut()
        .ylim(0.0, 10.0)
        .unwrap()
        .box_plot(
            vec!["B".into(), "A".into()],
            vec![vec![], vec![]],
            BoxOptions::default(),
        )
        .unwrap()
        .grouped_box_plot(
            vec!["A".into(), "B".into()],
            vec![Distribution {
                groups: vec![vec![8.0], vec![2.0]],
                ..Default::default()
            }],
        )
        .unwrap();
    let plot = plot_of(&f);
    let drawn = items(&f);
    assert_eq!(drawn.len(), 2);
    for (item, (x, y)) in drawn.iter().zip([(0.25, 2.0), (0.75, 8.0)]) {
        let Item::Stroke { path, .. } = item else {
            panic!("median")
        };
        let (a, b) = path.bounds().unwrap();
        near(f64::from(a.x + b.x) / 2.0, plot.x + x * plot.width);
        near(f64::from(a.y), plot.bottom() - y / 10.0 * plot.height);
    }
}

#[test]
fn box_labels_use_small_box_swatches_and_direct_falls_back_to_rows() {
    let mut f = figure(Theme::graphite(), ScaleKind::Linear);
    f.single_axes_mut()
        .grouped_box_plot(
            vec!["A".into()],
            vec![Distribution {
                label: "North".into(),
                groups: vec![vec![1.0, 2.0]],
                ..Default::default()
            }],
        )
        .unwrap();
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    for style in [
        LegendStyle::Header,
        LegendStyle::Direct,
        LegendStyle::Chips,
        LegendStyle::None,
    ] {
        let legend = Legend::new(style, &marks, &Limits::of(f.single_axes()));
        assert!(legend.direct().is_empty());
        if style == LegendStyle::None {
            assert!(legend.row().is_empty());
        } else {
            assert_eq!(legend.row()[0].label(), "North");
            assert_eq!(
                legend.row()[0].swatch(),
                Swatch::Box {
                    width: f.theme().drawn_line_width(SizeSpec::Auto)
                }
            );
        }
    }
}
