//! Line marks: clipping far-out data, NaN gaps, downsampling, dashes and pixels.

use super::super::line::downsample_columns;
use super::super::mapping::point;
use super::{Pixels, line_figure, line_strokes, near, plot_of, subpaths};
use crate::model::LineOptions;
use crate::resolve::NormalizedPos;
use crate::scene::{LineCap, LineJoin};
use crate::test_support::scene::assert_scene_bounded;
use crate::{Dash, FontLibrary, Theme};

#[test]
fn lines_far_outside_limits_are_clipped() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = line_figure(
            theme.clone(),
            &[-1e9, 0.5, 1e9],
            &[-1e9, 0.5, 1e9],
            LineOptions::default(),
        );
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let plot = plot_of(&f);
        // The line is y = x. In plot coordinates it is (x + v w, bottom - v h); inside the
        // plot area expanded by m it runs for v in -m / max(w, h) ..= 1 + m / max(w, h).
        let m = f64::from(theme.line_width()).max(2.0);
        let big = plot.width.max(plot.height);
        let at = |v: f64| (plot.x + v * plot.width, plot.bottom() - v * plot.height);
        let expected = [at(-m / big), at(0.5), at(1.0 + m / big)];
        let strokes = line_strokes(&scene);
        let [(path, _)] = strokes.as_slice() else {
            panic!("expected one stroke, got {strokes:?}")
        };
        let subpaths = subpaths(path);
        assert_eq!(subpaths.len(), 1);
        let got = &subpaths[0];
        assert_eq!(got.len(), 3);
        for (p, (ex, ey)) in got.iter().zip(expected) {
            assert!(
                (f64::from(p.x) - ex).abs() < 1e-3 && (f64::from(p.y) - ey).abs() < 1e-3,
                "{p:?} vs ({ex}, {ey})"
            );
        }
    }
}

#[test]
fn lines_leaving_and_reentering_start_new_subpaths() {
    // Up through the top edge and back down: two visible pieces.
    let mut f = line_figure(
        Theme::graphite(),
        &[0.0, 0.5, 1.0],
        &[0.5, 1e6, 0.5],
        LineOptions::default(),
    );
    f.axes_mut()
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    let scene = f.to_scene(FontLibrary::global());
    assert_scene_bounded(&scene);
    let plot = plot_of(&f);
    let top = plot.y - f64::from(Theme::graphite().line_width()).max(2.0);
    let strokes = line_strokes(&scene);
    let parts = subpaths(strokes[0].0);
    assert_eq!(parts.len(), 2);
    for part in &parts {
        assert_eq!(part.len(), 2);
        assert!(part.iter().any(|p| (f64::from(p.y) - top).abs() < 1e-3));
    }

    // Extreme data and limits: mapped coordinates near f64's range stay finite.
    let mut f = line_figure(
        Theme::midnight(),
        &[-1e300, 0.0, 1e300],
        &[1e300, 0.0, -1e300],
        LineOptions::default(),
    );
    f.axes_mut()
        .unwrap()
        .xlim(0.0, 1e-290)
        .unwrap()
        .ylim(-1e-290, 1e-290)
        .unwrap();
    assert_scene_bounded(&f.to_scene(FontLibrary::global()));
}

#[test]
fn nan_breaks_lines() {
    let f = line_figure(
        Theme::graphite(),
        &[0.0, 1.0, 2.0, 3.0, f64::NAN, 5.0, 6.0, 7.0],
        &[1.0, 2.0, f64::NAN, 3.0, 4.0, 2.0, 1.0, 3.0],
        LineOptions::default(),
    );
    let scene = f.to_scene(FontLibrary::global());
    assert_scene_bounded(&scene);
    let strokes = line_strokes(&scene);
    let parts = subpaths(strokes[0].0);
    // Runs: [0, 1], [3] (an isolated point), [5, 6, 7].
    let lens: Vec<usize> = parts.iter().map(Vec::len).collect();
    assert_eq!(lens, [2, 2, 3]);
    // The isolated point is a zero-length subpath, drawn as a dot by the round cap.
    assert_eq!(parts[1][0], parts[1][1]);
    assert_eq!(strokes[0].1.cap, LineCap::Round);
}

#[test]
fn downsampling_kicks_in_above_threshold() {
    let n = 1_000_000;
    let gaps = [123_457_usize, 500_001, 876_543];
    let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
    // Deterministic noise, so every column keeps four distinct points.
    let mut ys: Vec<f64> = (0..n)
        .map(|i| {
            let h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
            (i as f64 / 5e4).sin() + h as f64 / f64::from(1_u32 << 24)
        })
        .collect();
    for &g in &gaps {
        ys[g] = f64::NAN;
    }
    let f = line_figure(Theme::graphite(), &xs, &ys, LineOptions::default());
    let scene = f.to_scene(FontLibrary::global());
    assert_scene_bounded(&scene);
    let columns = downsample_columns(plot_of(&f).width);
    let strokes = line_strokes(&scene);
    let parts = subpaths(strokes[0].0);
    assert_eq!(parts.len(), gaps.len() + 1);
    let points: usize = parts.iter().map(Vec::len).sum();
    // A gap splits at most one column into two buckets of up to 4 points each.
    assert!(
        points <= 4 * columns + 4 * gaps.len(),
        "{points} points for {columns} columns"
    );
    assert!(
        points > 2 * columns,
        "{points} points for {columns} columns"
    );

    // At or below the threshold, every point is kept.
    let m = 4 * columns;
    let f = line_figure(
        Theme::graphite(),
        &xs[..m],
        &ys[..m].iter().map(|y| y.max(0.0)).collect::<Vec<_>>(),
        LineOptions::default(),
    );
    let scene = f.to_scene(FontLibrary::global());
    let points: usize = subpaths(line_strokes(&scene)[0].0)
        .iter()
        .map(Vec::len)
        .sum();
    assert_eq!(points, m);
}

#[test]
fn dash_patterns_scale_with_width() {
    for w in [1.0, 3.0] {
        let theme = Theme::graphite().with_line_width(w).unwrap();
        let w = w as f32;
        for (dash, pattern, cap) in [
            (Dash::Solid, vec![], LineCap::Round),
            (Dash::Dashed, vec![4.0 * w, 3.0 * w], LineCap::Butt),
            (Dash::Dotted, vec![0.0, 2.5 * w], LineCap::Round),
        ] {
            let f = line_figure(
                theme.clone(),
                &[0.0, 1.0],
                &[0.0, 1.0],
                LineOptions {
                    dash,
                    ..LineOptions::default()
                },
            );
            let scene = f.to_scene(FontLibrary::global());
            let strokes = line_strokes(&scene);
            let style = strokes[0].1;
            assert_eq!(style.dash, pattern, "{dash:?}");
            assert_eq!(
                (style.cap, style.join, style.width),
                (cap, LineJoin::Round, w)
            );
        }
    }
}

#[test]
fn line_color_appears_in_plot() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let theme = theme.with_line_width(4.0).unwrap();
        let mut f = line_figure(
            theme.clone(),
            &[0.0, 1.0],
            &[0.7, 0.7],
            LineOptions::default(),
        );
        f.axes_mut().unwrap().ylim(0.0, 1.0).unwrap();
        let plot = plot_of(&f);
        let dpi = 144.0;
        let pixels = Pixels::decode(&f.to_png(dpi as i64).unwrap());
        let color = theme.palette()[0];
        let on_line = (
            plot.x_at(NormalizedPos::new(0.5)),
            plot.y_at(NormalizedPos::new(0.7)),
        );
        assert!(
            near(pixels.at_point(on_line, dpi), color, 2),
            "{:?} vs {color:?}",
            pixels.at_point(on_line, dpi)
        );
        // Above the line there is no line color.
        let above = (
            plot.x_at(NormalizedPos::new(0.5)),
            plot.y_at(NormalizedPos::new(0.7)) - 6.0,
        );
        assert!(!near(pixels.at_point(above, dpi), color, 40));
    }
}

#[test]
fn huge_horizontal_lines_keep_their_visible_width() {
    for x in [1e20, 1e107, 1e300] {
        let mut f = line_figure(
            Theme::graphite(),
            &[-x, x * 0.7],
            &[0.5, 0.5],
            LineOptions::default(),
        );
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let plot = plot_of(&f);
        let m = f64::from(Theme::graphite().line_width()).max(2.0);
        let parts = subpaths(line_strokes(&scene)[0].0);
        assert_eq!(parts.len(), 1);
        let [a, b] = parts[0].as_slice() else {
            panic!("expected one segment, got {parts:?}")
        };
        assert!((f64::from(a.x) - (plot.x - m)).abs() < 1e-3, "{a:?}");
        assert!((f64::from(b.x) - (plot.right() + m)).abs() < 1e-3, "{b:?}");
        for p in [a, b] {
            assert!(
                (f64::from(p.y) - plot.y_at(NormalizedPos::new(0.5))).abs() < 1e-3,
                "{p:?}"
            );
        }
    }
}

#[test]
fn huge_to_inside_segments_keep_the_inside_end() {
    for v in [1e12, 1e20, 1e60, 1e150, 1e300] {
        let mut f = line_figure(
            Theme::midnight(),
            &[-v, 0.5],
            &[-v, 0.5],
            LineOptions::default(),
        );
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let plot = plot_of(&f);
        let m = f64::from(Theme::midnight().line_width()).max(2.0);
        let big = plot.width.max(plot.height);
        let at = |v: f64| (plot.x + v * plot.width, plot.bottom() - v * plot.height);
        let parts = subpaths(line_strokes(&scene)[0].0);
        let [a, b] = parts[0].as_slice() else {
            panic!("expected one segment, got {parts:?}")
        };
        for (p, (ex, ey)) in [(a, at(-m / big)), (b, at(0.5))] {
            assert!(
                (f64::from(p.x) - ex).abs() < 1e-3 && (f64::from(p.y) - ey).abs() < 1e-3,
                "{v}: {p:?} vs ({ex}, {ey})"
            );
        }
        // The inside end is the mapped data point itself.
        assert_eq!(*b, point(at(0.5).0, at(0.5).1));
    }
}
