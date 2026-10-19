//! Mapped point strokes are scene colors, shared by PNG, both SVG text modes and PDF.

use umber_core::{
    AxisData, BubbleOptions, ColorBy, Figure, FontLibrary, Item, Marker, PointColor, ScaleKind,
    ScatterOptions, SizeSpec, SvgText, Theme,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;
use pdf_check::image;

#[test]
fn mapped_point_outlines_agree_in_png_svg_and_pdf() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!(
            "skipped mapped-point PDF/PNG raster comparison: pdftoppm is not installed; \
             PNG pixels, SVG strokes and PDF color operators are still checked"
        );
    }
    for (name, theme) in [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        for bubbles in [false, true] {
            let mut f = Figure::new(4.0, 3.0, theme.clone()).unwrap();
            let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            let x = [1.0, 2.0, 3.0, 4.0, 5.0];
            let y = [1.0, 2.0, 1.0, 2.0, 1.0];
            let color = PointColor::By(ColorBy {
                center: Some(0.0),
                ..ColorBy::new(vec![-1.0, -0.2, 0.0, 0.2, 1.0])
            });
            if bubbles {
                axes.bubbles(
                    &x,
                    &y,
                    &[1.0, 2.0, 3.0, 2.0, 1.0],
                    BubbleOptions {
                        color,
                        ..BubbleOptions::default()
                    },
                )
                .unwrap();
            } else {
                axes.scatter(
                    AxisData::Numbers(x.to_vec()),
                    &y,
                    ScatterOptions {
                        color,
                        marker: Marker::Square,
                        size: SizeSpec::pt(16.0).unwrap(),
                        ..ScatterOptions::default()
                    },
                )
                .unwrap();
            }
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let Item::Group {
                clip: Some(_),
                items,
            } = &scene.items[1]
            else {
                panic!("the clipped marks group")
            };
            let strokes: Vec<_> = items
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
            assert_eq!(strokes.len(), 5);
            assert_ne!(strokes[2].color, theme.background());
            for style in &strokes {
                assert_eq!(style.width, 0.5);
                assert_eq!(style.color.a, 255);
            }
            for text in [SvgText::Editable, SvgText::Outlines] {
                let svg = f.to_svg(text).unwrap();
                for style in &strokes {
                    assert!(svg.contains(&format!(
                        r#"stroke="{}" stroke-width="0.5""#,
                        style.color.to_hex()
                    )));
                }
            }
            let center = items
                .iter()
                .filter(|item| matches!(item, Item::Markers { .. } | Item::Stroke { .. }))
                .nth(2)
                .unwrap();
            let (ring_x, ring_y) = match center {
                Item::Markers {
                    shape, positions, ..
                } => {
                    let (lo, hi) = shape.bounds().unwrap();
                    (positions[0].x + lo.x, positions[0].y + (lo.y + hi.y) / 2.0)
                }
                Item::Stroke { path, .. } => {
                    let (lo, hi) = path.bounds().unwrap();
                    (lo.x, (lo.y + hi.y) / 2.0)
                }
                _ => unreachable!(),
            };
            // At 360 dpi the 0.5 pt ring covers a full pixel at the path's left midpoint.
            let dpi = 360;
            let png = f.to_png(dpi).unwrap();
            let pixels = image::decode_rgba8(&png).unwrap();
            let scale = dpi as f32 / 72.0;
            let actual = pixels.at((ring_x * scale) as u32, (ring_y * scale) as u32);
            let color = strokes[2].color;
            let expected = [color.r, color.g, color.b, color.a];
            assert!(
                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "{name}, bubbles={bubbles}: ring pixel {actual:?}, expected {expected:?}"
            );
            let pdf = f.to_pdf().unwrap();
            assert_eq!(pdf, umber_core::render::render_pdf(&scene).unwrap());
            let content = pdf_check::page_content(&pdf);
            let lines: Vec<_> = content.lines().collect();
            let pdf_colors: Vec<Vec<f32>> = lines
                .windows(2)
                .filter(|pair| pair[0].ends_with(" RG") && pair[1] == "0.5 w")
                .map(|pair| {
                    pair[0]
                        .split_whitespace()
                        .take(3)
                        .map(|v| v.parse().unwrap())
                        .collect()
                })
                .collect();
            for style in &strokes {
                // PDF normalizes color channels to three decimal places.
                let rgb = [style.color.r, style.color.g, style.color.b]
                    .map(|v| (f32::from(v) / 255.0 * 1000.0).round() / 1000.0);
                assert!(
                    pdf_colors
                        .iter()
                        .any(|color| color.iter().zip(rgb).all(|(a, b)| (a - b).abs() < 1e-4)),
                    "missing {rgb:?} in PDF: {pdf_colors:?}"
                );
            }
            if poppler {
                let dir = pdf_check::scratch(&format!("mapped-points-{name}-{bubbles}"));
                let path = pdf_check::write(&dir, "points.pdf", &pdf);
                pdf_check::assert_matches_png(&path, &png, dpi as u32);
            }
        }
    }
}
