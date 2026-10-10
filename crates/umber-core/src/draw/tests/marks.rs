//! The marks group: its clip, mark order and resolved colors, determinism.

use super::{line_figure, mark_group, nums, plot_of};
use crate::model::{Figure, LineOptions, ScatterOptions};
use crate::scene::{Item, Rect};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, Rgba, ScaleKind, Theme};

#[test]
fn marks_group_is_clipped_to_the_plot_area() {
    let f = line_figure(
        Theme::graphite(),
        &[0.0, 1.0],
        &[0.0, 1.0],
        LineOptions::default(),
    );
    let scene = f.to_scene(FontLibrary::global());
    let plot = plot_of(&f);
    let (clip, items) = mark_group(&scene);
    assert_eq!(
        clip,
        Rect {
            x: plot.x as f32,
            y: plot.y as f32,
            width: plot.width as f32,
            height: plot.height as f32,
        }
    );
    assert_eq!(items.len(), 1);
    assert_scene_bounded(&scene);
    // No marks, or none visible: no marks group.
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(nums(&[f64::NAN]), &[f64::NAN], LineOptions::default())
        .unwrap();
    assert_eq!(f.to_scene(FontLibrary::global()).items.len(), 1);
}

#[test]
fn marks_follow_insertion_order_and_resolved_colors() {
    let theme = Theme::pigment();
    let fixed = Rgba::opaque(1, 2, 3);
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(nums(&[0.0, 1.0]), &[0.0, 1.0], LineOptions::default())
        .unwrap()
        .scatter(nums(&[0.5]), &[0.5], ScatterOptions::default())
        .unwrap()
        .line(
            nums(&[0.0, 1.0]),
            &[1.0, 0.0],
            LineOptions {
                color: crate::ColorSpec::Fixed(fixed),
                ..LineOptions::default()
            },
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global());
    let (_, items) = mark_group(&scene);
    let palette = theme.palette();
    match items {
        [
            Item::Stroke { style: a, .. },
            Item::Markers { fill: Some(b), .. },
            Item::Stroke { style: c, .. },
        ] => {
            assert_eq!(a.color, palette[0]);
            assert_eq!((b.r, b.g, b.b), (palette[1].r, palette[1].g, palette[1].b));
            assert_eq!(c.color, fixed);
        }
        other => panic!("unexpected marks {other:?}"),
    }
}

#[test]
fn mark_scenes_are_deterministic() {
    let build = || {
        let mut f = line_figure(
            Theme::midnight(),
            &[0.0, 1.0, 2.0],
            &[3.0, f64::NAN, 4.0],
            LineOptions::default(),
        );
        f.axes_mut()
            .unwrap()
            .scatter(nums(&[0.5, 1.5]), &[3.5, 3.7], ScatterOptions::default())
            .unwrap();
        f
    };
    assert_eq!(
        build().to_scene(FontLibrary::global()),
        build().to_scene(FontLibrary::global())
    );
    assert_eq!(build().to_png(72).unwrap(), build().to_png(72).unwrap());
}
