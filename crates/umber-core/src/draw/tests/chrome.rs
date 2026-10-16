//! The axes chrome: gridlines, the baseline, label placement and colors.

use super::{chrome_figure, strokes, texts};
use crate::geometry::{PathVerb, Point};
use crate::model::{AxisData, Figure, LineOptions};
use crate::scene::{LineCap, Scene, TextItem};
use crate::{FontLibrary, GridLines, ScaleKind, Theme};

#[test]
fn figure_without_axes_is_background_only() {
    let figure = Figure::new(2.0, 1.0, Theme::midnight()).unwrap();
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    assert!(scene.items.is_empty());
    assert_eq!((scene.width, scene.height), (144.0, 72.0));
    assert_eq!(scene.background, Theme::midnight().background());
}

#[test]
fn grid_dashes_follow_the_theme() {
    let cases = [
        (Theme::pigment(), vec![0.0, 3.2], LineCap::Round),
        (Theme::graphite(), vec![], LineCap::Butt),
        (Theme::midnight(), vec![2.0, 4.0], LineCap::Butt),
    ];
    for (theme, dash, cap) in cases {
        let scene = chrome_figure(theme.clone())
            .to_scene(FontLibrary::global())
            .unwrap();
        let all = strokes(&scene);
        let (grid, baseline) = all.split_at(all.len() - 1);
        assert!(!grid.is_empty());
        for (path, style) in grid {
            assert_eq!((&style.dash, style.cap), (&dash, cap));
            assert_eq!((style.color, style.width), (theme.grid_color(), 0.75));
            // Horizontal lines only.
            let ys: Vec<f32> = path
                .verbs()
                .iter()
                .filter_map(|v| match v {
                    PathVerb::MoveTo(p) | PathVerb::LineTo(p) => Some(p.y),
                    _ => None,
                })
                .collect();
            assert!(ys.windows(2).all(|w| w[0] == w[1]));
        }
        let (_, style) = baseline[0];
        assert_eq!(style.color, theme.baseline_color());
        assert_eq!(style.width, theme.baseline_width());
        assert!(style.dash.is_empty());
    }
}

#[test]
fn baseline_is_at_zero_or_bottom() {
    let baseline_y = |lo: f64, hi: f64| {
        let mut f = chrome_figure(Theme::graphite());
        f.single_axes_mut().ylim(lo, hi).unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let all = strokes(&scene);
        let (path, _) = all.last().copied().unwrap();
        let Some(PathVerb::MoveTo(p)) = path.verbs().first() else {
            panic!("baseline must start with a move")
        };
        let gridlines: Vec<f32> = all[..all.len() - 1]
            .iter()
            .filter_map(|(path, _)| match path.verbs().first() {
                Some(PathVerb::MoveTo(p)) => Some(p.y),
                _ => None,
            })
            .collect();
        (p.y, gridlines)
    };
    let (zero, grid) = baseline_y(-50.0, 50.0);
    assert!(grid.iter().any(|&y| y > zero) && grid.iter().any(|&y| y < zero));
    assert!(grid.iter().all(|&y| (y - zero).abs() > 0.01));
    let (bottom, grid) = baseline_y(10.0, 50.0);
    assert!(grid.iter().all(|&y| y < bottom));
}

#[test]
fn vertical_grid_and_category_axes() {
    let theme = Theme::graphite().with_grid_lines(GridLines::Both).unwrap();
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 10.0)
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let vertical = strokes(&scene)
        .iter()
        .filter(|(path, _)| match path.verbs() {
            [PathVerb::MoveTo(a), PathVerb::LineTo(b)] => a.x == b.x,
            _ => false,
        })
        .count();
    assert!(vertical >= 3);

    // A category x axis: one label per category, centered in its band, no vertical grid.
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Categories(vec!["Q1".into(), "Q2".into(), "Q3".into()]),
            &[1.0, 2.0, 3.0],
            LineOptions::default(),
        )
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let vertical = strokes(&scene)
        .iter()
        .filter(|(path, _)| match path.verbs() {
            [PathVerb::MoveTo(a), PathVerb::LineTo(b)] => a.x == b.x,
            _ => false,
        })
        .count();
    assert_eq!(vertical, 0);
    let all = texts(&scene);
    let q: Vec<&&TextItem> = all.iter().filter(|t| t.text.starts_with('Q')).collect();
    assert_eq!(q.len(), 3);
    let centers: Vec<f32> = q.iter().map(|t| t.origin.x + t.width / 2.0).collect();
    let (gap1, gap2) = (centers[1] - centers[0], centers[2] - centers[1]);
    assert!((gap1 - gap2).abs() < 1e-3, "{centers:?}");
}

#[test]
fn text_colors_follow_roles() {
    let theme = Theme::midnight();
    let scene = chrome_figure(theme.clone())
        .to_scene(FontLibrary::global())
        .unwrap();
    for t in texts(&scene) {
        let expected = match t.text.as_str() {
            "Station output" | "Day" | "MW" => theme.text_color(),
            _ => theme.muted_color(),
        };
        assert_eq!(t.color, expected, "{}", t.text);
        assert_eq!(
            t.style.tabular_figures,
            t.text.chars().all(|c| !c.is_alphabetic())
        );
    }
}

/// Asserts every glyph-outline point of every text item lies on the canvas.
#[track_caller]
fn assert_text_on_canvas(scene: &Scene) {
    let texts = texts(scene);
    assert!(!texts.is_empty());
    for t in texts {
        for verb in t.outline.verbs() {
            let points: Vec<Point> = match *verb {
                PathVerb::MoveTo(p) | PathVerb::LineTo(p) => vec![p],
                PathVerb::QuadTo(c, p) => vec![c, p],
                PathVerb::CubicTo(c1, c2, p) => vec![c1, c2, p],
                PathVerb::Close => vec![],
            };
            for p in points {
                assert!(
                    (0.0..=scene.width).contains(&p.x) && (0.0..=scene.height).contains(&p.y),
                    "{:?} at {p:?} leaves the {}x{} canvas",
                    t.text,
                    scene.width,
                    scene.height
                );
            }
        }
    }
}

#[test]
fn x_end_labels_stay_on_the_canvas() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for (lo, hi) in [(0.0, 100_000.0), (-100_000.0, 100_000.0), (-900_000.0, 0.0)] {
            let mut f = chrome_figure(theme.clone());
            f.single_axes_mut()
                .xlim(lo, hi)
                .unwrap()
                .ylim(-100_000.0, 100_000.0)
                .unwrap();
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            assert_text_on_canvas(&scene);
        }

        // Category labels wider than the padding at both ends.
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .title("Revenue by quarter")
            .line(
                AxisData::Categories(vec![
                    "First quarter of the year".into(),
                    "Q2".into(),
                    "Last quarter of the year".into(),
                ]),
                &[1.0, 2.0, 3.0],
                LineOptions::default(),
            )
            .unwrap();
        assert_text_on_canvas(&f.to_scene(FontLibrary::global()).unwrap());
    }
}

#[test]
fn end_label_inset_grows_only_for_the_first_x_label() {
    // The y tick labels ("0" to "1") are narrow, so a wide enough first x label decides the
    // left inset, and a wide last x label leaves it alone.
    let long = "A category name far wider than its band and the y tick labels together";
    let plot_left = |first: &str, last: &str| {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap()
            .line(
                AxisData::Categories(vec![first.into(), "b".into(), last.into()]),
                &[0.2, 0.5, 0.8],
                LineOptions::default(),
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let all = strokes(&scene);
        let (path, _) = all.last().copied().unwrap();
        match path.verbs().first() {
            Some(PathVerb::MoveTo(p)) => p.x,
            _ => panic!("baseline must start with a move"),
        }
    };
    let narrow = plot_left("a", "c");
    assert!(narrow > 12.0, "the y tick labels inset the plot");
    assert!(plot_left(long, "c") > narrow);
    assert_eq!(plot_left("a", long), narrow);
}

#[test]
fn scenes_are_deterministic() {
    let a = chrome_figure(Theme::pigment())
        .to_scene(FontLibrary::global())
        .unwrap();
    let b = chrome_figure(Theme::pigment())
        .to_scene(FontLibrary::global())
        .unwrap();
    assert_eq!(a, b);
}

#[test]
fn baseline_is_drawn_last_over_the_marks() {
    use crate::draw::mapping::point;
    use crate::scene::Item;
    let theme = Theme::pigment();
    let mut f = chrome_figure(theme.clone());
    f.single_axes_mut()
        .line(
            AxisData::Numbers(vec![0.0, 100.0]),
            &[0.0, 1.0],
            LineOptions::default(),
        )
        .unwrap();
    let layout = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global()).1;
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let is_baseline = |item: &Item| match item {
        Item::Stroke { path, style } => {
            style.color == theme.baseline_color()
                && style.width == theme.baseline_width()
                && path.verbs()
                    == [
                        PathVerb::MoveTo(point(layout.plot.x, layout.baseline_y())),
                        PathVerb::LineTo(point(layout.plot.right(), layout.baseline_y())),
                    ]
        }
        _ => false,
    };
    // Chrome, then the clipped marks, then the baseline alone and unclipped.
    let [
        Item::Group {
            clip: None,
            items: chrome,
        },
        Item::Group {
            clip: Some(_),
            items: marks,
        },
        Item::Group {
            clip: None,
            items: baseline,
        },
    ] = scene.items.as_slice()
    else {
        panic!("unexpected scene order: {:?}", scene.items);
    };
    assert!(!chrome.iter().any(is_baseline));
    assert!(!marks.is_empty() && !marks.iter().any(is_baseline));
    assert!(matches!(baseline.as_slice(), [only] if is_baseline(only)));

    // Without visible marks the baseline still follows the chrome.
    let scene = chrome_figure(theme.clone())
        .to_scene(FontLibrary::global())
        .unwrap();
    let [
        Item::Group { clip: None, .. },
        Item::Group {
            clip: None,
            items: baseline,
        },
    ] = scene.items.as_slice()
    else {
        panic!("unexpected scene order: {:?}", scene.items);
    };
    assert!(matches!(baseline.as_slice(), [only] if is_baseline(only)));
}

#[test]
fn a_baseline_on_the_x_axis_runs_from_top_to_bottom() {
    use crate::draw::mapping::point;
    use crate::resolve::ValueAxis;
    use crate::scene::Item;
    // A category y axis beside a linear x axis: the values, and the baseline, lie along x. It
    // sits at x = 0 inside the explicit limits, in place of the gridline there.
    let theme = Theme::graphite();
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .xlim(-2.0, 6.0)
        .unwrap();
    let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(resolved.baseline.axis, ValueAxis::X);
    assert_eq!(resolved.baseline.pos.get(), 0.25);
    assert_eq!(layout.baseline.axis, ValueAxis::X);
    let x = layout.plot.x + 0.25 * layout.plot.width;
    assert_eq!(layout.baseline.at, x);
    assert!(layout.x_grid.iter().all(|&grid| (grid - x).abs() > 1e-6));
    assert_eq!(layout.x_grid.len() + 1, resolved.x.ticks.len());
    // The category y axis has no categories, so no gridlines and no tick labels.
    assert!(layout.y_grid.is_empty() && layout.y_tick_labels.is_empty());
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let Some(Item::Group {
        clip: None,
        items: baseline,
    }) = scene.items.last()
    else {
        panic!("unexpected scene order: {:?}", scene.items);
    };
    let [Item::Stroke { path, style }] = baseline.as_slice() else {
        panic!("one baseline stroke expected, got {baseline:?}");
    };
    assert_eq!(style.color, theme.baseline_color());
    assert_eq!(
        path.verbs(),
        [
            PathVerb::MoveTo(point(x, layout.plot.y)),
            PathVerb::LineTo(point(x, layout.plot.bottom())),
        ]
    );
}

#[test]
fn two_category_axes_have_their_baseline_at_the_bottom() {
    use crate::resolve::ValueAxis;
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap();
    let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(resolved.baseline.axis, ValueAxis::Y);
    assert_eq!(layout.baseline_y(), layout.plot.bottom());
    assert!(layout.x_grid.is_empty() && layout.y_grid.is_empty());
}
