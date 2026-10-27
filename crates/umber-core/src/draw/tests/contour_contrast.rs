//! Color-mapped contour lines whose colors come close to the background, drawn darker (lighter
//! on a dark background).

use crate::cmap::{from_oklab, to_oklab};
use crate::pipeline::plan_figure;
use crate::scene::StrokeStyle;
use crate::{
    ColorSpec, ContourLevels, ContourOptions, ContourStyle, Figure, FontLibrary, Item, Rgba,
    ScaleKind, Theme,
};

use super::mark_items;

/// The three themes.
fn themes() -> [Theme; 3] {
    [Theme::pigment(), Theme::graphite(), Theme::midnight()]
}

/// A wave on a 25 × 17 grid, from -1 to 1, symmetric about zero.
fn wave() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..25).map(|i| f64::from(i) / 4.0 - 3.0).collect();
    let y: Vec<f64> = (0..17).map(|j| f64::from(j) / 4.0 - 2.0).collect();
    let z = y
        .iter()
        .flat_map(|&y| x.iter().map(move |&x| (x * 1.6).sin() * (y * 2.1).cos()))
        .collect();
    (x, y, z)
}

/// The levels of the centered contours: the middle of the colormap and two near its ends.
const LEVELS: [f64; 3] = [-0.95, 0.0, 0.95];

/// Centered contour options of `style`, with [`LEVELS`].
fn centered(style: ContourStyle) -> ContourOptions {
    ContourOptions {
        levels: ContourLevels::Values(LEVELS.to_vec()),
        style,
        center: Some(0.0),
        ..ContourOptions::default()
    }
}

/// A figure in `theme` with one contour of [`wave`].
fn figure(theme: &Theme, options: ContourOptions) -> Figure {
    let (x, y, z) = wave();
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .contour(&x, &y, &z, options)
        .unwrap();
    f
}

/// The stroke styles of the marks group of `f`, in drawing order.
fn strokes(f: &Figure) -> Vec<StrokeStyle> {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    mark_items(&scene)
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { style, .. } => Some(style.clone()),
            _ => None,
        })
        .collect()
}

/// The weight `w` of `color` against `background`, computed here from the rule itself.
fn weight(background: Rgba, color: Rgba) -> f64 {
    let bg = to_oklab(background);
    let lab = to_oklab(color);
    let d = (0..3).map(|i| (lab[i] - bg[i]).powi(2)).sum::<f64>().sqrt();
    (1.0 - d / 0.25).clamp(0.0, 1.0)
}

/// The color the rule draws a line colored by level in `color` on `background`: its Oklab
/// lightness moved `w × 0.25` away from the background's and kept within `0..=1`, opaque; the
/// color itself, opaque, at `w = 0`.
pub(super) fn expected_line_color(background: Rgba, color: Rgba) -> Rgba {
    let w = weight(background, color);
    if w == 0.0 {
        return Rgba { a: 255, ..color };
    }
    let [lightness, a, b] = to_oklab(color);
    let shift = if to_oklab(background)[0] >= 0.5 {
        -0.25
    } else {
        0.25
    };
    from_oklab([(lightness + w * shift).clamp(0.0, 1.0), a, b])
}

/// The color of the line along `level` on the color scale of `f`, before any contrast.
fn level_color(f: &Figure, level: f64) -> Rgba {
    let (resolved, _) = plan_figure(f, (0, 0), FontLibrary::global());
    resolved.color.unwrap().color(level).unwrap()
}

#[test]
fn near_background_lines_move_away_from_it_and_far_lines_keep_their_color() {
    for theme in themes() {
        let background = theme.background();
        let dark = to_oklab(background)[0] < 0.5;
        let f = figure(&theme, centered(ContourStyle::Lines));
        let drawn = strokes(&f);
        // One stroke per level, at the line width: nothing drawn under or beside the lines.
        assert_eq!(drawn.len(), LEVELS.len());
        for (style, level) in drawn.iter().zip(LEVELS) {
            let color = level_color(&f, level);
            assert_eq!(style.width, 1.25);
            assert_eq!(style.color, expected_line_color(background, color));
            assert_eq!(style.color.a, 255);
            let w = weight(background, color);
            if level == 0.0 {
                assert_eq!(color, theme.diverging_cmap().at(0.5));
                assert!(w > 0.5 && w < 1.0, "w={w}");
                let moved = to_oklab(style.color)[0] - to_oklab(color)[0];
                // Darker on the light themes, lighter on Midnight, by about w × 0.25.
                assert_eq!(moved > 0.0, dark, "{moved}");
                assert!((moved.abs() - w * 0.25).abs() < 0.01, "{moved} vs {w}");
            } else {
                assert_eq!(w, 0.0, "level {level}");
                assert_eq!(style.color, color, "level {level}");
            }
        }
    }
}

#[test]
fn labels_keep_the_text_color_and_annotated_lines_the_moved_color() {
    for theme in themes() {
        let plain = strokes(&figure(&theme, centered(ContourStyle::Lines)));
        let f = figure(
            &theme,
            ContourOptions {
                annotate: true,
                ..centered(ContourStyle::Lines)
            },
        );
        let annotated = strokes(&f);
        let colors = |styles: &[StrokeStyle]| -> Vec<(Rgba, f32)> {
            styles
                .iter()
                .map(|style| (style.color, style.width))
                .collect()
        };
        assert_eq!(colors(&annotated), colors(&plain));
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let labels: Vec<Rgba> = mark_items(&scene)
            .iter()
            .filter_map(|item| match item {
                Item::RotatedText { text, .. } => Some(text.color),
                _ => None,
            })
            .collect();
        assert!(!labels.is_empty());
        assert!(labels.iter().all(|&color| color == theme.text_color()));
    }
}

#[test]
fn filled_both_and_one_color_contours_keep_their_colors() {
    for theme in themes() {
        let near = theme.diverging_cmap().at(0.5);
        assert!(weight(theme.background(), near) > 0.0);
        // Bands only: no strokes at all.
        assert!(strokes(&figure(&theme, centered(ContourStyle::Filled))).is_empty());
        // Both: one thin line per level in the text color.
        let both = strokes(&figure(&theme, centered(ContourStyle::Both)));
        assert_eq!(both.len(), LEVELS.len());
        let text = theme.text_color().scale_alpha(0.7);
        assert!(
            both.iter()
                .all(|style| (style.color, style.width) == (text, 0.6))
        );
        // One color, even one close to the background: every line in it.
        for annotate in [false, true] {
            let one = strokes(&figure(
                &theme,
                ContourOptions {
                    levels: ContourLevels::Values(LEVELS.to_vec()),
                    color: ColorSpec::Fixed(near),
                    annotate,
                    ..ContourOptions::default()
                },
            ));
            assert_eq!(one.len(), LEVELS.len());
            assert!(
                one.iter()
                    .all(|style| (style.color, style.width) == (near, 1.25))
            );
        }
    }
}

#[test]
fn translucent_mapped_lines_keep_their_opacity() {
    for theme in themes() {
        let alpha = crate::Alpha::new(0.5).unwrap();
        let f = figure(
            &theme,
            ContourOptions {
                color: ColorSpec::Auto(alpha),
                ..centered(ContourStyle::Lines)
            },
        );
        let drawn = strokes(&f);
        assert_eq!(drawn.len(), LEVELS.len());
        for (style, level) in drawn.iter().zip(LEVELS) {
            let color = level_color(&f, level);
            assert_eq!(
                style.color,
                expected_line_color(theme.background(), color).with_alpha(alpha)
            );
        }
        assert_ne!(drawn[1].color, level_color(&f, 0.0).with_alpha(alpha));
    }
}
