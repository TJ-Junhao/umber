//! Stems, their shared scatter markers, clipping, domains and legend entries.

use super::{mark_group, nums, plot_of, subpaths};
use crate::legend::{Legend, Swatch};
use crate::model::{AxisData, Figure, LineOptions, ScatterOptions, StemOptions};
use crate::resolve::{Limits, resolve_marks};
use crate::scene::{Item, LineCap};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, LegendStyle, LogBase, Marker, ScaleKind, SizeSpec, Theme};

const LIN: ScaleKind = ScaleKind::Linear;
const LOG: ScaleKind = ScaleKind::Log { base: LogBase::TEN };

fn figure(theme: Theme, x: ScaleKind, y: ScaleKind) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(x, y).unwrap();
    f
}

fn limited(theme: Theme) -> Figure {
    let mut f = figure(theme, LIN, LIN);
    f.single_axes_mut()
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(-10.0, 10.0)
        .unwrap();
    f
}

fn items(f: &Figure) -> Vec<Item> {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    if scene.items.len() == 2 {
        Vec::new()
    } else {
        mark_group(&scene).1.to_vec()
    }
}

#[track_caller]
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-3, "{a} vs {b}");
}

#[test]
fn linear_stem_domain_includes_zero() {
    for values in [[2.0, 5.0], [-2.0, -5.0], [-2.0, 5.0], [0.0, 0.0]] {
        let mut f = figure(Theme::pigment(), LIN, LIN);
        f.single_axes_mut()
            .stem(nums(&[1.0, 2.0]), &values, StemOptions::default())
            .unwrap();
        let (resolved, _) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let (lo, hi) = resolved.y.continuous_scale().domain();
        assert!(lo <= 0.0 && hi >= 0.0, "{lo}..{hi}");
        for y in values {
            assert!((lo..=hi).contains(&y));
        }
        near(
            resolved.baseline.pos.get(),
            resolved.y.continuous_scale().normalize(0.0),
        );
    }
}

#[test]
fn log_stem_starts_at_baseline() {
    for explicit in [false, true] {
        let mut f = figure(Theme::midnight(), LIN, LOG);
        if explicit {
            f.single_axes_mut().ylim(1.0, 1000.0).unwrap();
        }
        f.single_axes_mut()
            .stem(nums(&[1.0, 2.0]), &[10.0, 100.0], StemOptions::default())
            .unwrap();
        let (_, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let drawn = items(&f);
        let Item::Stroke { path, .. } = &drawn[0] else {
            panic!("stem stroke expected")
        };
        for part in subpaths(path) {
            near(f64::from(part[0].y), layout.baseline_y());
        }
        if explicit {
            let parts = subpaths(path);
            near(
                f64::from(parts[0][1].y),
                layout.plot.bottom() - layout.plot.height / 3.0,
            );
        }
    }
}

#[test]
fn stem_nan_skips_point() {
    let mut f = limited(Theme::pigment());
    f.single_axes_mut()
        .stem(
            nums(&[1.0, f64::NAN, 3.0, 4.0]),
            &[2.0, 3.0, f64::NAN, -5.0],
            StemOptions::default(),
        )
        .unwrap();
    let drawn = items(&f);
    let [Item::Stroke { path, .. }, Item::Markers { positions, .. }] = drawn.as_slice() else {
        panic!("stroke then markers expected")
    };
    assert_eq!(subpaths(path).len(), 2);
    assert_eq!(positions.len(), 2);
    let mut empty = limited(Theme::pigment());
    empty
        .single_axes_mut()
        .stem(
            nums(&[f64::NAN, 1.0]),
            &[1.0, f64::NAN],
            StemOptions::default(),
        )
        .unwrap();
    assert!(items(&empty).is_empty());
}

#[test]
fn stem_markers_are_exactly_scatter_markers_in_every_theme_shape_and_size() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for marker in [
            Marker::Circle,
            Marker::Square,
            Marker::Triangle,
            Marker::Diamond,
            Marker::Cross,
        ] {
            for size in [
                SizeSpec::Auto,
                SizeSpec::pt(9.0).unwrap(),
                SizeSpec::times(2.0).unwrap(),
            ] {
                let mut stems = limited(theme.clone());
                let mut scatter = stems.clone();
                let (x, y) = ([0.0, 3.0, 10.0, 20.0], [-10.0, 5.0, 10.0, 5.0]);
                stems
                    .single_axes_mut()
                    .stem(
                        nums(&x),
                        &y,
                        StemOptions {
                            marker,
                            size,
                            ..StemOptions::default()
                        },
                    )
                    .unwrap();
                scatter
                    .single_axes_mut()
                    .scatter(
                        nums(&x),
                        &y,
                        ScatterOptions {
                            marker,
                            size,
                            ..ScatterOptions::default()
                        },
                    )
                    .unwrap();
                let (stems, scatter) = (items(&stems), items(&scatter));
                assert_eq!(stems.last(), scatter.last());
                let Item::Stroke { style, .. } = &stems[0] else {
                    panic!("stem stroke expected")
                };
                assert_eq!(style.width, 1.5);
                assert_eq!(style.cap, LineCap::Butt);
                assert!(style.dash.is_empty());
            }
        }
    }
}

#[test]
fn category_stems_use_band_centres_and_allow_repeated_names() {
    let mut f = figure(Theme::pigment(), ScaleKind::Category, LIN);
    f.single_axes_mut()
        .stem(
            AxisData::Categories(vec!["b".into(), "a".into(), "b".into()]),
            &[1.0, 2.0, 3.0],
            StemOptions::default(),
        )
        .unwrap();
    let plot = plot_of(&f);
    let drawn = items(&f);
    let [Item::Stroke { path, .. }, Item::Markers { positions, .. }] = drawn.as_slice() else {
        panic!("stems expected")
    };
    for ((part, marker), fraction) in subpaths(path).iter().zip(positions).zip([0.25, 0.75, 0.25]) {
        near(f64::from(part[0].x), plot.x + fraction * plot.width);
        assert_eq!(part[1], *marker);
    }
}

#[test]
fn explicit_limits_cut_stems_keep_boundary_markers_and_drop_far_markers() {
    for y_kind in [LIN, LOG] {
        let mut f = figure(Theme::pigment(), LIN, y_kind);
        f.single_axes_mut()
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(1.0, 100.0)
            .unwrap()
            .stem(
                nums(&[0.0, 10.0, 5.0, 6.0, -1e300, 1e300]),
                &[1.0, 100.0, 1e300, 1e-300, 50.0, 50.0],
                StemOptions::default(),
            )
            .unwrap();
        let plot = plot_of(&f);
        let drawn = items(&f);
        let [Item::Stroke { path, .. }, Item::Markers { positions, .. }] = drawn.as_slice() else {
            panic!("stems expected")
        };
        let parts = subpaths(path);
        assert_eq!(parts.len(), 2);
        for part in parts {
            near(f64::from(part[0].y), plot.bottom());
            near(f64::from(part[1].y), plot.y);
        }
        // On a linear y axis the value just below 1 still has a marker whose edge is visible.
        assert_eq!(positions.len(), if y_kind == LIN { 3 } else { 2 });
        near(f64::from(positions[0].x), plot.x);
        near(f64::from(positions[0].y), plot.bottom());
        near(f64::from(positions[1].x), plot.right());
        near(f64::from(positions[1].y), plot.y);
    }
}

#[test]
fn negative_stems_end_at_the_negative_value_from_zero() {
    let mut f = limited(Theme::graphite());
    f.single_axes_mut()
        .stem(nums(&[5.0]), &[-10.0], StemOptions::default())
        .unwrap();
    let plot = plot_of(&f);
    let drawn = items(&f);
    let Item::Stroke { path, .. } = &drawn[0] else {
        panic!("stroke expected")
    };
    let part = &subpaths(path)[0];
    near(f64::from(part[0].y), plot.y + plot.height / 2.0);
    near(f64::from(part[1].y), plot.bottom());
}

#[test]
fn empty_axis_contributions_do_not_change_padding_ticks_or_baseline() {
    for (x, y) in [(vec![], vec![]), (vec![f64::NAN], vec![f64::NAN])] {
        let mut f = figure(Theme::pigment(), LIN, LIN);
        f.single_axes_mut()
            .line(nums(&[1.1, 2.3]), &[10.0, 12.0], LineOptions::default())
            .unwrap();
        let before = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).0;
        f.single_axes_mut()
            .stem(nums(&x), &y, StemOptions::default())
            .unwrap();
        let after = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).0;
        assert_eq!(before, after);
    }
    // With no finite y data, an empty stem must not make explicit y ticks include zero.
    let mut f = figure(Theme::pigment(), LIN, LIN);
    f.single_axes_mut().yticks(&[10.0, 12.0]).unwrap();
    let before = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).0;
    f.single_axes_mut()
        .stem(nums(&[]), &[], StemOptions::default())
        .unwrap();
    assert_eq!(
        before,
        crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).0
    );
}

#[test]
fn stems_use_scatter_padding_and_row_legend_entries() {
    let mut f = figure(Theme::graphite(), LIN, LIN);
    f.single_axes_mut()
        .stem(
            nums(&[1.1, 2.3]),
            &[3.0, 4.0],
            StemOptions {
                label: "Response".into(),
                marker: Marker::Diamond,
                size: SizeSpec::pt(9.0).unwrap(),
                ..StemOptions::default()
            },
        )
        .unwrap();
    let mut scatter = figure(Theme::graphite(), LIN, LIN);
    scatter
        .single_axes_mut()
        .scatter(nums(&[1.1, 2.3]), &[3.0, 4.0], ScatterOptions::default())
        .unwrap();
    let resolve = |f: &Figure| crate::pipeline::plan_figure(f, (0, 0), FontLibrary::global()).0;
    assert_eq!(resolve(&f).x, resolve(&scatter).x);
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    for style in [
        LegendStyle::Header,
        LegendStyle::Chips,
        LegendStyle::Direct,
        LegendStyle::None,
    ] {
        let legend = Legend::new(style, &marks, &Limits::of(f.single_axes()));
        assert!(legend.direct().is_empty());
        if style == LegendStyle::None {
            assert!(legend.row().is_empty());
        } else {
            assert_eq!(legend.row().len(), 1);
            assert_eq!(legend.row()[0].label(), "Response");
            assert_eq!(legend.row()[0].swatch(), Swatch::Dot { diameter: 9.0 });
        }
    }
}

#[test]
fn dense_stems_keep_every_point_without_downsampling() {
    let mut f = limited(Theme::pigment());
    let x: Vec<_> = (0..20_000).map(|i| f64::from(i) / 2000.0).collect();
    f.single_axes_mut()
        .stem(nums(&x), &vec![5.0; x.len()], StemOptions::default())
        .unwrap();
    let drawn = items(&f);
    let [Item::Stroke { path, .. }, Item::Markers { positions, .. }] = drawn.as_slice() else {
        panic!("stems expected")
    };
    assert_eq!(subpaths(path).len(), x.len());
    assert_eq!(positions.len(), x.len());
}
