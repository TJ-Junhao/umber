//! Violin geometry, domain, clipping, degenerate groups and grouped-bar spacing.

use super::{plot_of, subpaths};
use crate::draw::bar::group_slots;
use crate::draw::fill::AREA_ALPHA;
use crate::legend::{Legend, Swatch};
use crate::model::{Distribution, Figure, ViolinOptions};
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
fn violins_include_all_samples_without_forcing_zero() {
    for sign in [-1.0, 1.0] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        f.single_axes_mut()
            .violin(
                vec!["A".into()],
                vec![
                    [10.0, 11.0, 12.0, 13.0, 14.0, 20.0]
                        .map(|v| v * sign)
                        .to_vec(),
                ],
                ViolinOptions::default(),
            )
            .unwrap();
        let (resolved, _) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let (lo, hi) = resolved.y.continuous_scale().domain();
        assert!((lo..=hi).contains(&(10.0 * sign)) && (lo..=hi).contains(&(20.0 * sign)));
        assert!(!(lo..=hi).contains(&0.0), "{lo}..{hi}");
    }
}

#[test]
fn grouped_violins_share_bar_gaps() {
    for n in [1, 2, 5, 100] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        f.single_axes_mut()
            .grouped_violin(
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
        let total = plot.width * f64::from(f.theme().violin_width());
        let (slot, gap) = group_slots(total, n, 2.0);
        near(
            f64::from(fills[0].0.x),
            plot.x + plot.width * (1.0 - f64::from(f.theme().violin_width())) / 2.0,
        );
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
fn log_violins_compute_statistics_before_transforming_and_clip_extremes() {
    for y in [ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN }] {
        let mut f = figure(Theme::midnight(), y);
        f.single_axes_mut()
            .violin(
                vec!["A".into()],
                vec![vec![1.0, 2.0, 4.0, 8.0]],
                ViolinOptions::default(),
            )
            .unwrap();
        let marks = resolve_marks(f.single_axes().marks(), f.theme());
        let [ResolvedMark::Violin { distributions, .. }] = marks.as_slice() else {
            panic!("box expected")
        };
        let stats = distributions[0].groups[0].as_ref().unwrap();
        assert_eq!((stats.q1, stats.median, stats.q3), (1.75, 3.0, 5.0));
        let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let drawn = items(&f);
        let Item::Stroke { path, .. } = &drawn[4] else {
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
            .violin(vec!["A".into()], vec![samples], ViolinOptions::default())
            .unwrap();
        let _ = items(&extreme);
    }
}

#[test]
fn violins_follow_axis_category_order_without_reordering_samples() {
    let mut f = figure(Theme::pigment(), ScaleKind::Linear);
    f.single_axes_mut()
        .ylim(0.0, 10.0)
        .unwrap()
        .violin(
            vec!["B".into(), "A".into()],
            vec![vec![], vec![]],
            ViolinOptions::default(),
        )
        .unwrap()
        .grouped_violin(
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
fn violin_labels_use_lens_swatches_and_direct_falls_back_to_rows() {
    let mut f = figure(Theme::graphite(), ScaleKind::Linear);
    f.single_axes_mut()
        .grouped_violin(
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
                Swatch::Violin {
                    width: f.theme().drawn_line_width(SizeSpec::Auto)
                }
            );
        }
    }
}

#[test]
fn violins_have_equal_max_width() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = figure(theme.clone(), ScaleKind::Linear);
        f.single_axes_mut()
            .violin(
                vec!["Narrow".into(), "Wide".into(), "Skewed".into()],
                vec![
                    vec![1.0, 1.1, 1.2],
                    vec![-100.0, 0.0, 100.0],
                    vec![0.0, 0.0, 0.0, 20.0],
                ],
                ViolinOptions::default(),
            )
            .unwrap();
        let plot = plot_of(&f);
        let drawn = items(&f);
        assert_eq!(drawn.len(), 15);
        for group in drawn.chunks_exact(5) {
            let [
                Item::Fill { path, paint, .. },
                Item::Stroke { style, .. },
                Item::Stroke {
                    style: q1_style, ..
                },
                Item::Stroke {
                    style: q3_style, ..
                },
                Item::Stroke {
                    style: median_style,
                    ..
                },
            ] = group
            else {
                panic!("violin")
            };
            let (a, b) = path.bounds().unwrap();
            near(
                f64::from(b.x - a.x),
                plot.width / 3.0 * f64::from(theme.violin_width()),
            );
            assert_eq!(style.width, theme.drawn_line_width(SizeSpec::Auto) / 2.0);
            assert_eq!(q1_style.width, style.width);
            assert_eq!(q3_style.width, style.width);
            assert_eq!(median_style.width, style.width * 2.0);
            assert_eq!(median_style.cap, LineCap::Butt);
            assert_eq!(*paint, Paint::Solid(style.color.scale_alpha(AREA_ALPHA)));
        }
    }
}

#[test]
fn violin_quantiles_follow_local_density_width() {
    let mut f = figure(Theme::pigment(), ScaleKind::Linear);
    f.single_axes_mut()
        .ylim(-2.0, 14.0)
        .unwrap()
        .violin(
            vec!["A".into()],
            vec![vec![0.0, 1.0, 2.0, 3.0, 4.0, 12.0]],
            ViolinOptions::default(),
        )
        .unwrap();
    let plot = plot_of(&f);
    let drawn = items(&f);
    let density = crate::stats::kde_scott(&[0.0, 1.0, 2.0, 3.0, 4.0, 12.0], 128).unwrap();
    let peak = density.iter().map(|p| p.1).fold(0.0, f64::max);
    for (item, (value, scale)) in drawn[2..]
        .iter()
        .zip([(1.25, 0.5), (3.75, 0.5), (2.5, 1.0)])
    {
        let Item::Stroke { path, .. } = item else {
            panic!("quantile")
        };
        let pair = density
            .windows(2)
            .find(|p| p[0].0 <= value && p[1].0 >= value)
            .unwrap();
        let fraction = (value - pair[0].0) / (pair[1].0 - pair[0].0);
        let density = pair[0].1 * (1.0 - fraction) + pair[1].1 * fraction;
        let (a, b) = path.bounds().unwrap();
        near(
            f64::from(b.x - a.x),
            plot.width * f64::from(f.theme().violin_width()) * scale * density / peak,
        );
        near(
            f64::from(a.y),
            plot.bottom() - (value + 2.0) / 16.0 * plot.height,
        );
    }
}

#[test]
fn violin_degenerate_groups() {
    for grouped in [false, true] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        let categories = ["Empty", "NaN", "One", "Equal"].map(String::from).to_vec();
        let groups = vec![vec![], vec![f64::NAN], vec![2.0], vec![4.0, 4.0]];
        if grouped {
            f.single_axes_mut()
                .grouped_violin(
                    categories,
                    vec![Distribution {
                        groups,
                        ..Default::default()
                    }],
                )
                .unwrap();
        } else {
            f.single_axes_mut()
                .violin(categories, groups, ViolinOptions::default())
                .unwrap();
        }
        let plot = plot_of(&f);
        let drawn = items(&f);
        assert_eq!(drawn.len(), 2);
        for item in drawn {
            let Item::Stroke { path, style } = item else {
                panic!("median only")
            };
            assert_eq!(subpaths(&path).len(), 1);
            assert_eq!(style.width, f.theme().drawn_line_width(SizeSpec::Auto));
            let (a, b) = path.bounds().unwrap();
            near(
                f64::from(b.x - a.x),
                plot.width / 4.0 * f64::from(f.theme().violin_width()),
            );
        }
    }
    for samples in [vec![], vec![f64::NAN]] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        f.single_axes_mut()
            .violin(vec!["A".into()], vec![samples], ViolinOptions::default())
            .unwrap();
        assert!(items(&f).is_empty());
    }
}

#[test]
fn violin_million_points_under_budget() {
    let start = std::time::Instant::now();
    let samples = (0..1_000_000).rev().map(f64::from).collect();
    let mut f = figure(Theme::pigment(), ScaleKind::Linear);
    f.single_axes_mut()
        .violin(
            vec!["Million".into()],
            vec![samples],
            ViolinOptions::default(),
        )
        .unwrap();
    assert_eq!(items(&f).len(), 5);
    println!(
        "violin_million_points_under_budget: 1000000 samples rendered in {:?}",
        start.elapsed()
    );
}

#[test]
fn violin_coverage_counts_density_and_quantile_lines() {
    for grouped in [false, true] {
        let mut f = figure(Theme::pigment(), ScaleKind::Linear);
        f.single_axes_mut()
            .legend_at(
                LegendStyle::Header,
                crate::LegendPosition::Best,
                crate::SizeSpec::Auto,
            )
            .unwrap();
        let groups = vec![vec![0.0, 1.0, 2.0, 3.0, 4.0]];
        if grouped {
            f.single_axes_mut()
                .grouped_violin(
                    vec!["A".into()],
                    vec![Distribution {
                        label: "Samples".into(),
                        groups,
                        ..Default::default()
                    }],
                )
                .unwrap();
        } else {
            f.single_axes_mut()
                .violin(
                    vec!["A".into()],
                    groups,
                    ViolinOptions {
                        label: "Samples".into(),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let marks = resolve_marks(f.single_axes().marks(), f.theme());
        let item = crate::draw::marks(
            f.theme(),
            &crate::test_support::figure_fonts(f.theme()),
            &resolved,
            &layout,
            &marks,
        )
        .unwrap()
        .unwrap();
        let prepared = marks[0].for_coverage(Some(crate::coverage::Geometry::of(&item)));
        let candidate = crate::layout::Frame {
            x: layout.plot.x + layout.plot.width / 2.0 - 1.0,
            y: layout.plot.bottom()
                - resolved.y.continuous_scale().normalize(2.0) * layout.plot.height
                - 1.0,
            width: 2.0,
            height: 2.0,
        };
        assert_eq!(marks[0].coverage(&layout.plot, &candidate), 0.0);
        near(prepared.coverage(&layout.plot, &candidate), 5.0);
        let outside = crate::layout::Frame {
            x: layout.plot.right() + 1.0,
            ..candidate
        };
        assert_eq!(prepared.coverage(&layout.plot, &outside), 0.0);
        assert!(!f.to_svg(crate::SvgText::Editable).unwrap().is_empty());
    }
}
