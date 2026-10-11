//! The marks group: its clip, mark order and resolved colors, determinism.

use super::{line_figure, mark_group, nums, plot_of};
use crate::model::{Figure, LineOptions, ScatterOptions};
use crate::scene::{Item, Rect};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, Rgba, ScaleKind, SizeSpec, Theme};

#[test]
fn marks_group_is_clipped_to_the_plot_area_widened_for_line_ends() {
    // Graphite lines are 1.8 pt wide: the clip reaches 0.9 pt past the left and right edges,
    // and its top and bottom are the plot area's.
    let f = line_figure(
        Theme::graphite(),
        &[0.0, 1.0],
        &[0.0, 1.0],
        LineOptions::default(),
    );
    let scene = f.to_scene(FontLibrary::global());
    let plot = plot_of(&f);
    let (clip, items) = mark_group(&scene);
    let half = f64::from(Theme::graphite().line_width()) / 2.0;
    assert!((half - 0.9).abs() < 1e-6);
    assert_eq!(
        clip,
        Rect {
            x: (plot.x - half) as f32,
            y: plot.y as f32,
            width: (plot.width + 2.0 * half) as f32,
            height: plot.height as f32,
        }
    );
    assert_eq!(items.len(), 1);
    assert_scene_bounded(&scene);

    // The widest line sets the widening, whatever other marks the axes hold.
    let theme = Theme::graphite().with_line_width(6.0).unwrap();
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(nums(&[0.0, 1.0]), &[0.0, 1.0], ScatterOptions::default())
        .unwrap()
        .line(nums(&[0.0, 1.0]), &[1.0, 0.0], LineOptions::default())
        .unwrap();
    let scene = f.to_scene(FontLibrary::global());
    let plot = plot_of(&f);
    let (clip, _) = mark_group(&scene);
    assert_eq!(
        clip,
        Rect {
            x: (plot.x - 3.0) as f32,
            y: plot.y as f32,
            width: (plot.width + 6.0) as f32,
            height: plot.height as f32,
        }
    );

    // Without line marks the clip is the plot area.
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(nums(&[0.0, 1.0]), &[0.0, 1.0], ScatterOptions::default())
        .unwrap();
    let scene = f.to_scene(FontLibrary::global());
    let plot = plot_of(&f);
    let (clip, _) = mark_group(&scene);
    assert_eq!(
        clip,
        Rect {
            x: plot.x as f32,
            y: plot.y as f32,
            width: plot.width as f32,
            height: plot.height as f32,
        }
    );
    // No marks, or none visible: no marks group.
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(nums(&[f64::NAN]), &[f64::NAN], LineOptions::default())
        .unwrap();
    assert_eq!(f.to_scene(FontLibrary::global()).items.len(), 2);
}

#[test]
fn clip_widening_follows_the_widest_drawn_line_of_any_width() {
    // Graphite lines are 1.8 pt wide; the area asks for 9 pt and is the widest line. The large
    // scatter does not count: markers are culled by their own extent.
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    let line = |width: SizeSpec| LineOptions {
        width,
        ..LineOptions::default()
    };
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(nums(&[0.0, 1.0]), &[0.0, 1.0], line(SizeSpec::Auto))
        .unwrap()
        .area(
            nums(&[0.0, 1.0]),
            &[0.5, 0.5],
            line(SizeSpec::pt(9.0).unwrap()),
        )
        .unwrap()
        .line(
            nums(&[0.0, 1.0]),
            &[1.0, 0.0],
            line(SizeSpec::times(0.5).unwrap()),
        )
        .unwrap()
        .scatter(
            nums(&[0.5]),
            &[0.5],
            ScatterOptions {
                size: SizeSpec::pt(40.0).unwrap(),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global());
    let plot = plot_of(&f);
    let (clip, _) = mark_group(&scene);
    assert_eq!(
        clip,
        Rect {
            x: (plot.x - 4.5) as f32,
            y: plot.y as f32,
            width: (plot.width + 9.0) as f32,
            height: plot.height as f32,
        }
    );

    // A thin line alone widens the clip by half its own width, not the theme's.
    let f = line_figure(
        Theme::graphite(),
        &[0.0, 1.0],
        &[0.0, 1.0],
        line(SizeSpec::pt(0.5).unwrap()),
    );
    let scene = f.to_scene(FontLibrary::global());
    let plot = plot_of(&f);
    assert_eq!(mark_group(&scene).0.x, (plot.x - 0.25) as f32);
}

#[test]
fn clip_widening_stops_at_the_y_labels_and_the_canvas() {
    for width in [20.0, 30.0] {
        let theme = Theme::graphite().with_line_width(width).unwrap();
        let mut f = Figure::new(6.0, 3.0, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[0.5, 0.5], LineOptions::default())
            .unwrap();
        let layout = crate::pipeline::plan_figure(&f, FontLibrary::global()).1;
        let scene = f.to_scene(FontLibrary::global());
        let (clip, _) = mark_group(&scene);
        let (left, right) = (f64::from(clip.x), f64::from(clip.x + clip.width));
        let half = width / 2.0;
        // Half the stroke is wider than the 6 pt gap to the y tick labels: the clip stops there.
        assert!(half > layout.room_left(), "{width}");
        assert!(
            (left - (layout.plot.x - layout.room_left())).abs() < 1e-3,
            "{width}"
        );
        // On the right it reaches past the plot by half the stroke, but never past the canvas.
        let expected_right = layout.plot.right() + half.min(layout.room_right());
        assert!((right - expected_right).abs() < 1e-3, "{width}: {right}");
        if width == 30.0 {
            assert!(half > layout.room_right());
        }
        // No y tick label lies under the clip, and the clip stays on the canvas.
        assert!(!layout.y_tick_labels.is_empty());
        for label in &layout.y_tick_labels {
            assert!(
                label.x + label.width() <= left + 1e-3,
                "{width}: {}",
                label.text
            );
        }
        assert!(left >= 0.0, "{width}: {left}");
        assert!(right <= 6.0 * 72.0 + 1e-3, "{width}: {right}");
    }
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
