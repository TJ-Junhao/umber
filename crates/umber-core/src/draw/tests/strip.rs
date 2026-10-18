//! Category points: jitter bounds, non-overlap and call order.

use super::mark_items;
use crate::model::{BoxOptions, StripOptions, SwarmOptions};
use crate::scene::Item;
use crate::{Figure, FontLibrary, ScaleKind, SizeSpec, Theme};

fn figure(horizontal: bool) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let (x, y) = if horizontal {
        (ScaleKind::Linear, ScaleKind::Category)
    } else {
        (ScaleKind::Category, ScaleKind::Linear)
    };
    figure.add_axes(x, y).unwrap();
    figure
}

#[test]
fn strip_jitter_within_band() {
    for horizontal in [false, true] {
        for jitter in [0.0, 0.1, 0.4, 0.5] {
            let mut f = figure(horizontal);
            f.single_axes_mut()
                .strip(
                    vec!["A".into()],
                    vec![vec![1.0; 100]],
                    StripOptions {
                        jitter,
                        ..Default::default()
                    },
                )
                .unwrap();
            let (_, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let [Item::Markers { positions, .. }] = mark_items(&scene) else {
                panic!("markers expected")
            };
            assert_eq!(positions.len(), 100);
            let (start, extent) = if horizontal {
                (layout.plot.y, layout.plot.height)
            } else {
                (layout.plot.x, layout.plot.width)
            };
            for p in positions {
                let along = if horizontal { p.y } else { p.x };
                assert!((f64::from(along) - start - extent / 2.0).abs() <= jitter * extent + 1e-4);
            }
        }
    }
}

#[test]
fn swarm_scene_points_never_overlap() {
    let mut seed = 0xbb67_ae85_84ca_a73b_u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 11) as f64 / (1_u64 << 53) as f64
    };
    for horizontal in [false, true] {
        for _ in 0..32 {
            let mut f = figure(horizontal);
            let count = 10 + (random() * 151.0) as usize;
            let values = (0..count).map(|_| random() * 30.0).collect();
            f.single_axes_mut()
                .swarm(
                    vec!["A".into()],
                    vec![values],
                    SwarmOptions {
                        size: SizeSpec::pt(6.0).unwrap(),
                        ..Default::default()
                    },
                )
                .unwrap();
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let [Item::Markers { positions, .. }] = mark_items(&scene) else {
                panic!("markers expected")
            };
            assert_eq!(positions.len(), count);
            for (i, a) in positions.iter().enumerate() {
                for b in &positions[i + 1..] {
                    assert!(
                        (a.x - b.x).hypot(a.y - b.y) >= 6.0 - 1e-4,
                        "{a:?} and {b:?}"
                    );
                }
            }
        }
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing bound requires release mode")]
fn large_tied_swarm_overflow_is_fast() {
    let mut f = figure(false);
    f.single_axes_mut()
        .swarm(
            vec!["A".into()],
            vec![vec![1.0; 30_000]],
            SwarmOptions {
                size: SizeSpec::pt(6.0).unwrap(),
                ..Default::default()
            },
        )
        .unwrap();
    let fonts = FontLibrary::global();
    let start = std::time::Instant::now();
    let error = f.to_scene(fonts).unwrap_err();
    let elapsed = start.elapsed();
    println!("30000 tied swarm points rejected in {elapsed:?}");
    assert!(matches!(error, crate::Error::Style { .. }));
    assert_eq!(
        error.hint(),
        Some("use strip() or violin() for many points.")
    );
    assert!(elapsed < std::time::Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn unrelated_marks_do_not_change_strip_jitter() {
    for horizontal in [false, true] {
        let mut offsets = Vec::new();
        for before in [false, true] {
            let mut f = figure(horizontal);
            let mut ax = f.single_axes_mut();
            let samples = vec![vec![2.0; 40]];
            if !before {
                ax.strip(vec!["A".into()], samples.clone(), StripOptions::default())
                    .unwrap();
            }
            ax.box_plot(
                vec!["B".into(), "A".into()],
                vec![vec![], vec![]],
                BoxOptions::default(),
            )
            .unwrap();
            if before {
                ax.strip(vec!["A".into()], samples, StripOptions::default())
                    .unwrap();
            }
            let (_, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let [Item::Markers { positions, .. }] = mark_items(&scene) else {
                panic!("markers expected")
            };
            let (start, extent) = if horizontal {
                (layout.plot.y, layout.plot.height)
            } else {
                (layout.plot.x, layout.plot.width)
            };
            let category = usize::from(before);
            offsets.push(
                positions
                    .iter()
                    .map(|p| {
                        let coordinate = if horizontal { p.y } else { p.x };
                        (f64::from(coordinate) - start) / (extent / 2.0) - category as f64 - 0.5
                    })
                    .collect::<Vec<_>>(),
            );
        }
        for (a, b) in offsets[0].iter().zip(&offsets[1]) {
            assert!((a - b).abs() < 1e-6, "{a} != {b}");
        }
    }
}

#[test]
fn strips_on_the_same_category_have_independent_jitter() {
    for horizontal in [false, true] {
        let mut f = figure(horizontal);
        for _ in 0..2 {
            f.single_axes_mut()
                .strip(
                    vec!["A".into()],
                    vec![vec![1.0; 40]],
                    StripOptions::default(),
                )
                .unwrap();
        }
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let [
            Item::Markers { positions: a, .. },
            Item::Markers { positions: b, .. },
        ] = mark_items(&scene)
        else {
            panic!("two marker batches expected")
        };
        assert_ne!(a, b);
        assert_eq!(scene, f.to_scene(FontLibrary::global()).unwrap());
    }
}

#[test]
fn strip_over_box_draws_in_call_order() {
    for first in [false, true] {
        let mut f = figure(false);
        let mut ax = f.single_axes_mut();
        if first {
            ax.strip(vec!["A".into()], vec![vec![2.0]], StripOptions::default())
                .unwrap();
        }
        ax.box_plot(
            vec!["A".into()],
            vec![vec![1.0, 2.0, 3.0]],
            BoxOptions::default(),
        )
        .unwrap();
        if !first {
            ax.strip(vec!["A".into()], vec![vec![2.0]], StripOptions::default())
                .unwrap();
        }
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let items = mark_items(&scene);
        assert_eq!(matches!(items.first(), Some(Item::Markers { .. })), first);
        assert_eq!(matches!(items.last(), Some(Item::Markers { .. })), !first);
    }
}
