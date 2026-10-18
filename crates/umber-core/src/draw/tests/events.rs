//! Event row positions, tick lengths and bounded geometry.

use super::{mark_items, subpaths};
use crate::scene::Item;
use crate::test_support::scene::assert_scene_bounded;
use crate::{AxisData, EventsOptions, Figure, FontLibrary, Instant, LogBase, ScaleKind, Theme};

#[test]
fn events_one_row_per_category() {
    for length in [0.1, 0.8, 1.0] {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Category)
            .unwrap()
            .events(
                ["North", "Empty", "South"].map(String::from).to_vec(),
                vec![
                    AxisData::Numbers(vec![1.0, f64::NAN, 2.0]),
                    AxisData::Numbers(vec![]),
                    AxisData::Numbers(vec![3.0]),
                ],
                EventsOptions {
                    length,
                    ..Default::default()
                },
            )
            .unwrap();
        let (_, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        assert_scene_bounded(&scene);
        let [Item::Stroke { path, .. }] = mark_items(&scene) else {
            panic!("event strokes expected")
        };
        let parts = subpaths(path);
        assert_eq!(parts.len(), 3);
        for (part, row) in parts.iter().zip([0.0, 0.0, 2.0]) {
            assert_eq!(part.len(), 2);
            assert_eq!(part[0].x, part[1].x);
            let middle = f64::from(part[0].y + part[1].y) / 2.0;
            assert!((middle - layout.plot.y - (row + 0.5) * layout.plot.height / 3.0).abs() < 1e-4);
            assert!(
                (f64::from(part[1].y - part[0].y).abs() - length * layout.plot.height / 3.0).abs()
                    < 1e-4
            );
        }
    }
}

#[test]
fn events_on_time_x() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Time, ScaleKind::Category)
        .unwrap()
        .events(
            vec!["A".into()],
            vec![AxisData::Times(vec![
                Some(Instant::from_micros(0).unwrap()),
                None,
                Some(Instant::from_micros(86_400_000_000).unwrap()),
            ])],
            EventsOptions::default(),
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let [Item::Stroke { path, .. }] = mark_items(&scene) else {
        panic!("event strokes expected")
    };
    let parts = subpaths(path);
    assert_eq!(parts.len(), 2);
    assert!(parts[0][0].x < parts[1][0].x);
}

#[test]
fn events_clip_before_mapping_extreme_values() {
    for x in [ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN }] {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(x, ScaleKind::Category)
            .unwrap()
            .xlim(1.0, 10.0)
            .unwrap()
            .events(
                vec!["A".into()],
                vec![AxisData::Numbers(vec![1e-10, 5.0, 1e300])],
                EventsOptions::default(),
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        assert_scene_bounded(&scene);
        let [Item::Stroke { path, .. }] = mark_items(&scene) else {
            panic!("event strokes expected")
        };
        assert_eq!(subpaths(path).len(), 1);
    }
}
