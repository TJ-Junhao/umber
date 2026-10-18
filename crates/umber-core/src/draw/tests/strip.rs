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
fn swarm_points_never_overlap() {
    for horizontal in [false, true] {
        for seed in 0..32 {
            let mut f = figure(horizontal);
            let values = (0..40)
                .map(|i| f64::from((i * 17 + seed * 13) % 31) / 10.0)
                .collect();
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
            assert_eq!(positions.len(), 40);
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
