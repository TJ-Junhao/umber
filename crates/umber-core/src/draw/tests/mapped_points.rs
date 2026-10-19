//! Color-mapped point outlines: Oklab contrast, batching and unchanged uniform marks.

use super::super::scatter::{MAPPED_MARKER_OUTLINE, MappedOutlines};
use super::{Pixels, mark_group, near, nums};
use crate::cmap::{from_oklab, to_oklab};
use crate::{
    BubbleOptions, ColorBy, ColorSpec, Figure, FontLibrary, Item, LegendStyle, Marker, Paint,
    PointColor, Rgba, ScaleKind, ScatterOptions, SizeSpec, Theme,
};

fn mapped_outline_color(theme: &Theme, fill: Rgba) -> Rgba {
    MappedOutlines::new(theme).color(fill)
}

#[test]
fn mapped_outline_colors_at_centers_ends_and_mid_values() {
    for (theme, center, middle) in [
        (
            Theme::pigment(),
            Rgba::opaque(172, 166, 158),
            Rgba::opaque(183, 185, 179),
        ),
        (
            Theme::graphite(),
            Rgba::opaque(175, 175, 175),
            Rgba::opaque(199, 202, 209),
        ),
        (
            Theme::midnight(),
            Rgba::opaque(65, 69, 78),
            Rgba::opaque(36, 40, 48),
        ),
    ] {
        let cmap = theme.diverging_cmap();
        for (t, expected) in [
            (0.0, theme.background()),
            (0.4, middle),
            (0.5, center),
            (1.0, theme.background()),
        ] {
            let fill = cmap.at(t);
            assert_eq!(mapped_outline_color(&theme, fill), expected, "t={t}");
            let d = to_oklab(fill)
                .iter()
                .zip(to_oklab(theme.background()))
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            let w = (1.0 - d / 0.25).clamp(0.0, 1.0);
            match t {
                0.0 | 1.0 => assert_eq!(w, 0.0),
                0.5 => {
                    // Midnight's darker midpoint is farther from its background.
                    let minimum = if theme.background() == Theme::midnight().background() {
                        0.5
                    } else {
                        0.85
                    };
                    assert!(w > minimum && w < 1.0, "w={w}");
                }
                _ => assert!(w > 0.0 && w < 0.6, "w={w}"),
            }
        }
    }
}

#[test]
fn mapped_outline_moves_lightness_away_from_background_and_ignores_fill_alpha() {
    for bg in [
        Rgba::BLACK,
        Rgba::opaque(97, 97, 97),
        Rgba::opaque(101, 101, 101),
        Rgba::WHITE,
        Rgba::opaque(250, 40, 30),
    ] {
        let theme = Theme::pigment().with_background(bg).unwrap();
        let [lightness, a, b] = to_oklab(bg);
        let delta = if lightness >= 0.5 { -0.25 } else { 0.25 };
        // Equal fill and background give w = 1, including a chromatic, gamut-clipped ring.
        let ring = mapped_outline_color(&theme, bg);
        assert_eq!(ring, from_oklab([lightness + delta, a, b]));
        assert!((to_oklab(ring)[0] - lightness) * delta > 0.0);
        assert_eq!(mapped_outline_color(&theme, Rgba { a: 0, ..bg }), ring);
        let translucent = theme.with_background(Rgba { a: 0, ..bg }).unwrap();
        assert_eq!(mapped_outline_color(&translucent, bg), ring);
        let far = if lightness >= 0.5 {
            Rgba::BLACK
        } else {
            Rgba::WHITE
        };
        assert_eq!(
            mapped_outline_color(&translucent, far),
            translucent.background()
        );
    }
}

#[test]
fn mapped_outline_alpha_follows_weight_on_transparent_backgrounds() {
    for bg in [Rgba::WHITE, Rgba::opaque(0, 0, 0)] {
        let opaque = Theme::pigment().with_background(bg).unwrap();
        let [lightness, a, b] = to_oklab(bg);
        let near = from_oklab([lightness + if lightness >= 0.5 { -0.1 } else { 0.1 }, a, b]);
        let far = if lightness >= 0.5 {
            Rgba::BLACK
        } else {
            Rgba::WHITE
        };
        for alpha in [0, 128] {
            let theme = opaque.with_background(Rgba { a: alpha, ..bg }).unwrap();
            let mut outlines = MappedOutlines::new(&theme);
            assert_eq!(outlines.color(far), theme.background());
            assert_eq!(outlines.color(bg), mapped_outline_color(&opaque, bg));
            assert_eq!(outlines.color(bg).a, 255);
            let distance = to_oklab(near)
                .iter()
                .zip(to_oklab(bg))
                .map(|(fill, background)| (fill - background).powi(2))
                .sum::<f64>()
                .sqrt();
            let weight = (1.0 - distance / 0.25).clamp(0.0, 1.0);
            assert!(weight > 0.0 && weight < 1.0);
            let expected = Rgba {
                a: ((1.0 - weight) * f64::from(alpha) + weight * 255.0).round() as u8,
                ..mapped_outline_color(&opaque, near)
            };
            assert!(expected.a > alpha && expected.a < 255);
            assert_eq!(outlines.color(near), expected);
            assert_eq!(outlines.color(far), theme.background());
            assert_eq!(outlines.color(near), expected);
        }
    }
}

#[test]
fn mapped_translucent_outlines_reach_scatter_and_bubbles() {
    for alpha in [0, 128] {
        let theme = Theme::pigment()
            .with_background(Rgba {
                a: alpha,
                ..Rgba::WHITE
            })
            .unwrap();
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        let color = PointColor::By(ColorBy {
            center: Some(0.0),
            ..ColorBy::new(vec![-1.0, -0.2, 0.0, -1.0])
        });
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                nums(&[1.0, 2.0, 3.0, 4.0]),
                &[1.0; 4],
                ScatterOptions {
                    color: color.clone(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .bubbles(
                &[1.0, 2.0, 3.0, 4.0],
                &[2.0; 4],
                &[1.0; 4],
                BubbleOptions {
                    color,
                    ..BubbleOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let strokes: Vec<_> = mark_group(&scene)
            .1
            .iter()
            .filter_map(|item| match item {
                Item::Markers {
                    stroke: Some(style),
                    ..
                }
                | Item::Stroke { style, .. } => Some(style),
                _ => None,
            })
            .collect();
        assert_eq!(strokes.len(), 8);
        for batch in strokes.chunks_exact(4) {
            for (style, t) in batch.iter().zip([0.0, 0.4, 0.5, 0.0]) {
                assert_eq!(style.width, 0.5);
                assert_eq!(
                    style.color,
                    mapped_outline_color(&theme, theme.diverging_cmap().at(t))
                );
            }
            assert_eq!(batch[0].color, theme.background());
            assert!(batch[1].color.a > alpha && batch[1].color.a < 255);
            assert!(batch[2].color.a > batch[1].color.a);
            assert_eq!(batch[0].color, batch[3].color);
        }
    }
}

#[test]
fn mapped_scatter_batches_share_fill_and_outline_in_data_order() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for marker in [
            Marker::Circle,
            Marker::Square,
            Marker::Triangle,
            Marker::Diamond,
            Marker::Cross,
        ] {
            let mut f = Figure::new(6.0, 4.0, theme.with_marker_alpha(0.0).unwrap()).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .scatter(
                    nums(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
                    &[1.0; 6],
                    ScatterOptions {
                        marker,
                        color: PointColor::By(ColorBy {
                            center: Some(0.0),
                            ..ColorBy::new(vec![0.0, 0.0, f64::NAN, 1.0, -1.0, 0.0])
                        }),
                        ..ScatterOptions::default()
                    },
                )
                .unwrap();
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let items = mark_group(&scene).1;
            assert_eq!(items.len(), 4);
            for (item, (t, count)) in items.iter().zip([(0.5, 2), (1.0, 1), (0.0, 1), (0.5, 1)]) {
                let Item::Markers {
                    fill: Some(fill),
                    stroke: Some(stroke),
                    positions,
                    ..
                } = item
                else {
                    panic!("a mapped marker batch")
                };
                assert_eq!(*fill, theme.diverging_cmap().at(t));
                assert_eq!(fill.a, 255);
                assert_eq!(positions.len(), count);
                assert_eq!(stroke.width, MAPPED_MARKER_OUTLINE);
                assert_eq!(stroke.width, 0.5);
                assert_eq!(stroke.color, mapped_outline_color(&theme, *fill));
                assert_eq!(stroke.color.a, 255);
            }
        }
    }
}

#[test]
fn mapped_bubbles_use_the_same_outline_and_keep_neutral_swatches() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let theme = theme.with_legend_style(LegendStyle::Header).unwrap();
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .bubbles(
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[100.0, 400.0, 900.0],
                BubbleOptions {
                    label: "Size".into(),
                    color: PointColor::By(ColorBy {
                        center: Some(0.0),
                        ..ColorBy::new(vec![-1.0, 0.0, 1.0])
                    }),
                    ..BubbleOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let marks = mark_group(&scene).1;
        assert_eq!(marks.len(), 6);
        for (pair, t) in marks.chunks_exact(2).zip([0.0, 0.5, 1.0]) {
            let [
                Item::Fill {
                    paint: Paint::Solid(fill),
                    ..
                },
                Item::Stroke { style, .. },
            ] = pair
            else {
                panic!("each bubble's fill then outline")
            };
            assert_eq!(*fill, theme.diverging_cmap().at(t));
            assert_eq!(style.width, 0.5);
            assert_eq!(style.color, mapped_outline_color(&theme, *fill));
        }
        let swatches: Vec<_> = scene
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Group { clip: None, items } => Some(items),
                _ => None,
            })
            .flatten()
            .filter_map(|item| match item {
                Item::Markers {
                    fill,
                    stroke: Some(stroke),
                    ..
                } => Some((fill, stroke)),
                _ => None,
            })
            .collect();
        assert!(!swatches.is_empty());
        for (fill, stroke) in swatches {
            let neutral = theme
                .text_color()
                .scale_alpha(theme.marker_alpha())
                .over(theme.background());
            assert_eq!(*fill, Some(neutral));
            assert_eq!((stroke.color, stroke.width), (theme.background(), 0.8));
        }
    }
}

#[test]
fn uniform_scatter_and_bubbles_keep_background_outlines_even_for_near_fills() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()]
        .into_iter()
        .chain([0, 128].map(|a| {
            Theme::pigment()
                .with_background(Rgba { a, ..Rgba::WHITE })
                .unwrap()
        }))
    {
        let color = theme.diverging_cmap().at(0.5);
        let uniform = PointColor::Uniform(ColorSpec::Fixed(color));
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                nums(&[1.0]),
                &[1.0],
                ScatterOptions {
                    color: uniform.clone(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .bubbles(
                &[2.0],
                &[2.0],
                &[1.0],
                BubbleOptions {
                    color: uniform,
                    ..BubbleOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let [
            Item::Markers {
                fill: Some(scatter_fill),
                stroke: Some(scatter_stroke),
                ..
            },
            Item::Fill {
                paint: Paint::Solid(bubble_fill),
                ..
            },
            Item::Stroke {
                style: bubble_stroke,
                ..
            },
        ] = mark_group(&scene).1
        else {
            panic!("a scatter and a bubble")
        };
        assert_eq!(*scatter_fill, color.scale_alpha(theme.marker_alpha()));
        assert_eq!(scatter_fill, bubble_fill);
        for stroke in [scatter_stroke, bubble_stroke] {
            assert_eq!((stroke.color, stroke.width), (theme.background(), 0.8));
        }
    }
}

#[test]
fn mapped_center_outline_is_visible_in_png() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                nums(&[0.5]),
                &[0.5],
                ScatterOptions {
                    marker: Marker::Square,
                    size: SizeSpec::pt(24.0).unwrap(),
                    color: PointColor::By(ColorBy {
                        center: Some(0.0),
                        ..ColorBy::new(vec![0.0])
                    }),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let [
            Item::Markers {
                shape,
                positions,
                fill: Some(fill),
                stroke: Some(stroke),
            },
        ] = mark_group(&scene).1
        else {
            panic!("one marker")
        };
        let dpi = 720.0;
        let png = Pixels::decode(&f.to_png(dpi as i64).unwrap());
        let center = positions[0];
        let (lo, _) = shape.bounds().unwrap();
        let at = (f64::from(center.x + lo.x), f64::from(center.y));
        assert_ne!(stroke.color, theme.background());
        assert!(near(png.at_point(at, dpi), stroke.color, 2));
        assert!(near(
            png.at_point((f64::from(center.x), at.1), dpi),
            *fill,
            2
        ));
    }
}
