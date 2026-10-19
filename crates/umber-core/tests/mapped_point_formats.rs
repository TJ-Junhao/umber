//! Mapped point strokes are scene colors, shared by PNG, both SVG text modes and PDF.

use umber_core::{
    AxisData, BubbleOptions, ColorBy, Figure, FontLibrary, Item, Marker, PointColor, ScaleKind,
    ScatterOptions, SizeSpec, SvgText, Theme,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;

#[test]
fn mapped_point_outlines_agree_in_png_svg_and_pdf() {
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
            let png = f.to_png(144).unwrap();
            assert_eq!(png, umber_core::render::render_png(&scene, 144).unwrap());
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
            if pdf_check::have("pdftoppm") {
                let dir = pdf_check::scratch(&format!("mapped-points-{name}-{bubbles}"));
                let path = pdf_check::write(&dir, "points.pdf", &pdf);
                pdf_check::assert_matches_png(&path, &png, 144);
            }
        }
    }
}
