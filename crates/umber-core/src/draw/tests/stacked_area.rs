//! Stacked area geometry, shared gaps, flat fills, direct labels and visible downsampling.

use super::super::area::AREA_ALPHA;
use super::super::line::downsample_columns;
use super::{mark_group, plot_of, subpaths};
use crate::model::{Figure, Series};
use crate::resolve::{AnchorX, EndKind, Limits, ResolvedMark, SeriesKind, resolve_marks};
use crate::scene::{Item, Paint};
use crate::test_support::scene::assert_scene_bounded;
use crate::{Dash, FillStyle, FontLibrary, LogBase, ScaleKind, Theme};

const LIN: ScaleKind = ScaleKind::Linear;
const NAN: f64 = f64::NAN;

fn figure(theme: Theme, x: &[f64], values: &[&[f64]]) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(LIN, LIN)
        .unwrap()
        .stacked_area(x, series(values))
        .unwrap();
    f
}

fn series(values: &[&[f64]]) -> Vec<Series> {
    values
        .iter()
        .map(|v| Series {
            values: v.to_vec(),
            ..Series::default()
        })
        .collect()
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

fn chains(f: &Figure) -> Vec<Vec<(f64, f64)>> {
    items(f)
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { path, .. } => Some(
                subpaths(path)
                    .into_iter()
                    .map(|part| {
                        part.into_iter()
                            .map(|p| (f64::from(p.x), f64::from(p.y)))
                            .collect()
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn layers_stack_in_order() {
    let mut f = figure(
        Theme::pigment(),
        &[0.0, 1.0, 2.0],
        &[&[1.0, 2.0, 3.0], &[3.0, 1.0, 2.0]],
    );
    f.single_axes_mut()
        .xlim(0.0, 2.0)
        .unwrap()
        .ylim(0.0, 6.0)
        .unwrap();
    let plot = plot_of(&f);
    let at = |x: f64, y: f64| {
        (
            plot.x + x / 2.0 * plot.width,
            plot.bottom() - y / 6.0 * plot.height,
        )
    };
    let items = items(&f);
    assert_eq!(items.len(), 4);
    for (k, (upper, lower)) in [
        ([1.0, 2.0, 3.0], [0.0; 3]),
        ([4.0, 3.0, 5.0], [1.0, 2.0, 3.0]),
    ]
    .iter()
    .enumerate()
    {
        let Item::Fill { path, .. } = &items[2 * k] else {
            panic!()
        };
        let polygon = &subpaths(path)[0];
        let expected = [
            at(0.0, upper[0]),
            at(1.0, upper[1]),
            at(2.0, upper[2]),
            at(2.0, lower[2]),
            at(1.0, lower[1]),
            at(0.0, lower[0]),
        ];
        assert_eq!(polygon.len(), expected.len());
        for (got, want) in polygon.iter().zip(expected) {
            assert!((f64::from(got.x) - want.0).abs() < 1e-3);
            assert!((f64::from(got.y) - want.1).abs() < 1e-3);
        }
        let Item::Stroke { path, .. } = &items[2 * k + 1] else {
            panic!()
        };
        assert_eq!(subpaths(path)[0], polygon[..3]);
    }
}

#[test]
fn nan_breaks_every_layer() {
    let x = [0.0, 1.0, 2.0, 3.0, 4.0];
    let a = [1.0; 5];
    let b = [2.0; 5];
    let cases = [
        ([0.0, 1.0, NAN, 3.0, 4.0], a, b),
        (x, [1.0, 1.0, NAN, 1.0, 1.0], b),
        (x, a, [2.0, 2.0, NAN, 2.0, 2.0]),
    ];
    let mut previous = None;
    for (x, a, b) in cases {
        let f = figure(Theme::pigment(), &x, &[&a, &b]);
        let items = items(&f);
        assert_eq!(items.len(), 4);
        for item in &items {
            let path = match item {
                Item::Fill { path, .. } | Item::Stroke { path, .. } => path,
                _ => panic!(),
            };
            assert_eq!(subpaths(path).len(), 2);
        }
        if let Some(before) = previous {
            assert_eq!(items, before);
        }
        previous = Some(items);
    }
}

#[test]
fn flat_fill_and_upper_line_ignore_fill_style_in_every_theme() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let width = theme.drawn_line_width(crate::SizeSpec::Auto);
        for style in [FillStyle::Solid, FillStyle::Gradient] {
            let f = figure(
                theme.clone().with_fill_style(style).unwrap(),
                &[0.0, 1.0],
                &[&[1.0, 2.0], &[2.0, 1.0]],
            );
            for (k, pair) in items(&f).chunks_exact(2).enumerate() {
                let [Item::Fill { paint, .. }, Item::Stroke { style, .. }] = pair else {
                    panic!()
                };
                assert_eq!(
                    *paint,
                    Paint::Solid(theme.palette()[k].scale_alpha(AREA_ALPHA))
                );
                assert_eq!(style.color, theme.palette()[k]);
                assert_eq!(style.width, width);
                assert!(style.dash.is_empty());
            }
        }
    }
}

#[test]
fn empty_all_nan_and_zero_layers_have_no_fill() {
    for (x, values) in [
        (vec![], vec![]),
        (vec![0.0, 1.0], vec![NAN; 2]),
        (vec![0.0, 1.0], vec![0.0; 2]),
    ] {
        let f = figure(Theme::pigment(), &x, &[&values]);
        assert!(
            items(&f)
                .iter()
                .all(|item| !matches!(item, Item::Fill { .. }))
        );
    }
}

#[test]
fn log_axes_use_cumulative_values_and_the_resolved_baseline() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let log = ScaleKind::Log { base: LogBase::TEN };
    f.add_axes(log, log)
        .unwrap()
        .stacked_area(&[1.0, 10.0, 100.0], series(&[&[10.0; 3], &[90.0; 3]]))
        .unwrap()
        .xlim(1.0, 100.0)
        .unwrap()
        .ylim(1.0, 1000.0)
        .unwrap();
    let plot = plot_of(&f);
    let curves = chains(&f);
    assert_eq!(curves.len(), 2);
    for (k, curve) in curves.iter().enumerate() {
        for (i, &(x, y)) in curve.iter().enumerate() {
            assert!((x - (plot.x + i as f64 * plot.width / 2.0)).abs() < 1e-3);
            assert!((y - (plot.bottom() - (k + 1) as f64 * plot.height / 3.0)).abs() < 1e-3);
        }
    }
    let Item::Fill { path, .. } = &items(&f)[0] else {
        panic!()
    };
    assert!(
        subpaths(path)[0]
            .iter()
            .any(|p| (f64::from(p.y) - plot.bottom()).abs() < 1e-3)
    );
}

#[test]
fn resolved_layers_have_area_keys_and_direct_anchors_at_each_upper_end() {
    let mut f = figure(
        Theme::graphite(),
        &[0.0, 1.0, 2.0],
        &[&[1.0, 2.0, 3.0], &[3.0, 4.0, NAN]],
    );
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    let [mark @ ResolvedMark::StackedArea { .. }] = marks.as_slice() else {
        panic!()
    };
    assert!(mark.grows_from_baseline() && mark.has_exact_x_extent() && !mark.is_reference());
    assert_eq!(mark.x_numbers().collect::<Vec<_>>(), [0.0, 1.0, 2.0]);
    assert_eq!(mark.x_categories().count(), 0);
    let values = mark.y_values().collect::<Vec<_>>();
    assert_eq!(&values[..2], &[1.0, 2.0]);
    assert_eq!(&values[3..5], &[4.0, 6.0]);
    assert!(values[2].is_nan() && values[5].is_nan());
    for (k, key) in mark.keys(&Limits::of(f.single_axes())).iter().enumerate() {
        assert_eq!(key.color, f.theme().palette()[k]);
        assert!(matches!(
            key.kind,
            SeriesKind::Line {
                dash: Dash::Solid,
                ..
            }
        ));
        assert!(key.kind.takes_direct_label());
        let anchor = key.anchor.unwrap();
        assert_eq!(
            (anchor.x, anchor.y, anchor.end),
            (AnchorX::Number(1.0), [2.0, 6.0][k], EndKind::Point)
        );
    }
    f.single_axes_mut().xlim(0.0, 0.5).unwrap();
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    for (k, key) in marks[0]
        .keys(&Limits::of(f.single_axes()))
        .iter()
        .enumerate()
    {
        let anchor = key.anchor.unwrap();
        assert_eq!(
            (anchor.x, anchor.y, anchor.end),
            (AnchorX::Number(0.5), [1.5, 5.0][k], EndKind::Edge)
        );
    }
}

/// The vertical range of a sorted polyline inside a column, including interpolated edges.
fn over(chain: &[(f64, f64)], a: f64, b: f64) -> (f64, f64) {
    let at = |x: f64| {
        let k = chain.partition_point(|p| p.0 < x);
        if k == 0 {
            chain[0].1
        } else if k == chain.len() {
            chain[k - 1].1
        } else {
            let (p, q) = (chain[k - 1], chain[k]);
            p.1 + (q.1 - p.1) * (x - p.0) / (q.0 - p.0)
        }
    };
    let from = chain.partition_point(|p| p.0 <= a);
    let to = chain.partition_point(|p| p.0 < b);
    chain[from..to.max(from)]
        .iter()
        .map(|p| p.1)
        .chain([at(a), at(b)])
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), y| {
            (lo.min(y), hi.max(y))
        })
}

#[test]
fn every_layer_keeps_its_envelope_with_exact_explicit_limits() {
    let n = 200_000;
    let x: Vec<_> = (0..n).map(|i| i as f64).collect();
    let noise = |i: usize| {
        ((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f64 / f64::from(1_u32 << 24)
    };
    let mut a: Vec<_> = (0..n).map(|i| 1.0 + noise(i)).collect();
    let mut b: Vec<_> = (0..n).map(|i| 2.0 + noise(i + n)).collect();
    a[59_999] = 90.0;
    b[140_001] = 90.0;
    let cumulative: Vec<_> = a.iter().zip(&b).map(|(a, b)| a + b).collect();
    for (lo, hi) in [
        (0.0, n as f64 - 1.0),
        (60_000.0, 140_000.0),
        (60_000.25, 139_999.75),
    ] {
        let mut f = figure(Theme::pigment(), &x, &[&a, &b]);
        f.single_axes_mut()
            .xlim(lo, hi)
            .unwrap()
            .ylim(0.0, 100.0)
            .unwrap();
        let plot = plot_of(&f);
        let curves = chains(&f);
        assert_eq!(curves.len(), 2);
        let columns = downsample_columns(plot.width);
        assert!(curves[0].len() < n / 4);
        for (got, raw) in curves.iter().zip([&a, &cumulative]) {
            let raw: Vec<_> = x
                .iter()
                .zip(raw)
                .map(|(&x, &y)| {
                    (
                        plot.x + (x - lo) / (hi - lo) * plot.width,
                        plot.bottom() - y / 100.0 * plot.height,
                    )
                })
                .collect();
            for c in 0..columns {
                let (left, right) = (
                    plot.x + c as f64 / columns as f64 * plot.width,
                    plot.x + (c + 1) as f64 / columns as f64 * plot.width,
                );
                let (g, w) = (over(got, left, right), over(&raw, left, right));
                assert!(
                    (g.0 - w.0).abs() < 0.05 && (g.1 - w.1).abs() < 0.05,
                    "column {c}: {g:?} vs {w:?}"
                );
            }
        }
    }
}
