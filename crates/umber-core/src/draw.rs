//! Drawing: turns a [`Layout`] into scene items.
//!
//! M5a Task 3 draws the axes chrome: gridlines, the baseline, tick labels, the header and the
//! axis labels, all in one unclipped group. Marks (Task 4) get their own group, clipped to the
//! plot area.

use crate::geometry::{Path, Point};
use crate::layout::{Label, Layout};
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle, TextItem};
use crate::{Dash, GridLines, Rgba, Theme};

/// The widest stroke drawn, whatever the theme says.
const MAX_STROKE_WIDTH: f32 = 1.0e4;

/// A scene point from layout coordinates. Layout keeps every value finite and far below the
/// renderers' `MAX_COORD`, so narrowing to `f32` is safe.
fn point(x: f64, y: f64) -> Point {
    Point {
        x: x as f32,
        y: y as f32,
    }
}

/// A stroke width that is finite and within `0..=MAX_STROKE_WIDTH`.
fn stroke_width(width: f32) -> f32 {
    if width.is_finite() && width > 0.0 {
        width.min(MAX_STROKE_WIDTH)
    } else {
        0.0
    }
}

fn segment(from: Point, to: Point) -> Path {
    let mut path = Path::new();
    path.move_to(from);
    path.line_to(to);
    path
}

/// The gridline stroke: `Dotted` is round dots every 3.2 pt, `Dashed` 2 on / 4 off.
fn grid_style(theme: &Theme) -> StrokeStyle {
    let (dash, cap) = match theme.grid_dash() {
        Dash::Solid => (Vec::new(), LineCap::Butt),
        Dash::Dashed => (vec![2.0, 4.0], LineCap::Butt),
        Dash::Dotted => (vec![0.0, 3.2], LineCap::Round),
    };
    StrokeStyle {
        color: theme.grid_color(),
        width: stroke_width(theme.grid_width()),
        dash,
        cap,
        join: LineJoin::Miter,
    }
}

fn text(label: &Label, color: Rgba) -> Item {
    let origin = point(label.x, label.y);
    Item::Text(TextItem {
        text: label.text.clone(),
        style: label.style,
        origin,
        color,
        outline: label.layout.outline(origin),
        width: label.layout.width(),
    })
}

/// The axes chrome as one unclipped group: gridlines, baseline, tick labels, header and axis
/// labels, in that order.
pub(crate) fn chrome(theme: &Theme, layout: &Layout) -> Item {
    let plot = layout.plot;
    let mut items = Vec::new();

    let grid = grid_style(theme);
    let lines = theme.grid_lines();
    if matches!(lines, GridLines::Horizontal | GridLines::Both) {
        for &y in &layout.y_grid {
            items.push(Item::Stroke {
                path: segment(point(plot.x, y), point(plot.right(), y)),
                style: grid.clone(),
            });
        }
    }
    if matches!(lines, GridLines::Vertical | GridLines::Both) {
        for &x in &layout.x_grid {
            items.push(Item::Stroke {
                path: segment(point(x, plot.y), point(x, plot.bottom())),
                style: grid.clone(),
            });
        }
    }

    items.push(Item::Stroke {
        path: segment(
            point(plot.x, layout.baseline_y),
            point(plot.right(), layout.baseline_y),
        ),
        style: StrokeStyle {
            color: theme.baseline_color(),
            width: stroke_width(theme.baseline_width()),
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        },
    });

    let muted = theme.muted_color();
    for label in layout.y_tick_labels.iter().chain(&layout.x_tick_labels) {
        items.push(text(label, muted));
    }
    if let Some(label) = &layout.title {
        items.push(text(label, theme.text_color()));
    }
    if let Some(label) = &layout.subtitle {
        items.push(text(label, muted));
    }
    for label in layout.ylabel.iter().chain(&layout.xlabel) {
        items.push(text(label, theme.text_color()));
    }

    Item::Group { clip: None, items }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geometry::PathVerb;
    use crate::render::{SvgText, render_svg};
    use crate::scene::Scene;
    use crate::{Figure, FontLibrary, ScaleKind};

    /// The renderers' coordinate limit, restated so the tests do not depend on `normalize`.
    const MAX_COORD: f32 = 1.0e6;

    fn check(p: Point) {
        assert!(
            p.x.is_finite() && p.y.is_finite() && p.x.abs() <= MAX_COORD && p.y.abs() <= MAX_COORD,
            "{p:?}"
        );
    }

    fn check_path(path: &Path) {
        for verb in path.verbs() {
            match *verb {
                PathVerb::MoveTo(p) | PathVerb::LineTo(p) => check(p),
                PathVerb::QuadTo(c, p) => {
                    check(c);
                    check(p);
                }
                PathVerb::CubicTo(c1, c2, p) => {
                    check(c1);
                    check(c2);
                    check(p);
                }
                PathVerb::Close => {}
            }
        }
    }

    fn check_items(items: &[Item]) {
        for item in items {
            match item {
                Item::Fill { path, .. } => check_path(path),
                Item::Stroke { path, style } => {
                    check_path(path);
                    assert!(style.width.is_finite());
                }
                Item::Markers {
                    shape, positions, ..
                } => {
                    check_path(shape);
                    positions.iter().copied().for_each(check);
                }
                Item::Text(t) => {
                    check(t.origin);
                    check_path(&t.outline);
                    assert!(t.width.is_finite());
                }
                Item::Group { clip, items } => {
                    if let Some(r) = clip {
                        check(Point { x: r.x, y: r.y });
                        check(Point {
                            x: r.x + r.width,
                            y: r.y + r.height,
                        });
                    }
                    check_items(items);
                }
            }
        }
    }

    /// Asserts every coordinate of `scene` is finite and within `MAX_COORD`.
    pub(crate) fn assert_scene_bounded(scene: &Scene) {
        assert!(scene.width.is_finite() && scene.height.is_finite());
        check_items(&scene.items);
    }

    fn chrome_items(scene: &Scene) -> &[Item] {
        match scene.items.as_slice() {
            [Item::Group { clip: None, items }] => items,
            other => panic!("expected one unclipped group, got {other:?}"),
        }
    }

    fn strokes(scene: &Scene) -> Vec<(&Path, &StrokeStyle)> {
        chrome_items(scene)
            .iter()
            .filter_map(|item| match item {
                Item::Stroke { path, style } => Some((path, style)),
                _ => None,
            })
            .collect()
    }

    fn texts(scene: &Scene) -> Vec<&TextItem> {
        chrome_items(scene)
            .iter()
            .filter_map(|item| match item {
                Item::Text(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    fn chrome_figure(theme: Theme) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
        figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .title("Station output")
            .subtitle("Daily mean, MW")
            .xlabel("Day")
            .ylabel("MW")
            .xlim(0.0, 100.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        figure
    }

    #[test]
    fn chrome_snapshot() {
        let scene = chrome_figure(Theme::pigment()).to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        insta::assert_snapshot!(render_svg(&scene, SvgText::Editable));
    }

    #[test]
    fn figure_without_axes_is_background_only() {
        let figure = Figure::new(2.0, 1.0, Theme::midnight()).unwrap();
        let scene = figure.to_scene(FontLibrary::global());
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
            let scene = chrome_figure(theme.clone()).to_scene(FontLibrary::global());
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
            f.axes_mut().unwrap().ylim(lo, hi).unwrap();
            let scene = f.to_scene(FontLibrary::global());
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
        let scene = f.to_scene(FontLibrary::global());
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
                crate::AxisData::Categories(vec!["Q1".into(), "Q2".into(), "Q3".into()]),
                &[1.0, 2.0, 3.0],
                crate::LineOptions::default(),
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
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
        let scene = chrome_figure(theme.clone()).to_scene(FontLibrary::global());
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

    #[test]
    fn scenes_are_deterministic() {
        let a = chrome_figure(Theme::pigment()).to_scene(FontLibrary::global());
        let b = chrome_figure(Theme::pigment()).to_scene(FontLibrary::global());
        assert_eq!(a, b);
    }
}
