//! A color-mapped contour line close to the background is drawn in its moved color alike in
//! PNG, both SVG text modes and PDF.

use umber_core::{
    ContourLevels, ContourOptions, Figure, FontLibrary, Item, PathVerb, ScaleKind, SvgText, Theme,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;
use pdf_check::image;

#[test]
fn contrasting_contour_lines_agree_in_png_svg_and_pdf() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!(
            "skipped contour contrast PDF/PNG raster comparison: pdftoppm is not installed; \
             PNG pixels, SVG strokes and PDF operators are still checked"
        );
    }
    let x: Vec<f64> = (0..25).map(|i| f64::from(i) / 4.0 - 3.0).collect();
    let y: Vec<f64> = (0..17).map(|j| f64::from(j) / 4.0 - 2.0).collect();
    let z: Vec<f64> = y
        .iter()
        .flat_map(|&y| x.iter().map(move |&x| (x * 1.6).sin() * (y * 2.1).cos()))
        .collect();
    for (name, theme) in [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = Figure::new(4.0, 3.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .contour(
                &x,
                &y,
                &z,
                ContourOptions {
                    levels: ContourLevels::Values(vec![-0.95, 0.0, 0.95]),
                    center: Some(0.0),
                    ..ContourOptions::default()
                },
            )
            .unwrap();
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
                Item::Stroke { path, style } => Some((path, style)),
                _ => None,
            })
            .collect();
        // One line per level; the center level's is moved away from the colormap's midpoint.
        assert_eq!(strokes.len(), 3, "{name}");
        let (center_path, center) = strokes[1];
        assert_eq!(center.width, 1.25);
        assert_eq!(center.color.a, 255);
        assert_ne!(center.color, theme.diverging_cmap().at(0.5), "{name}");

        // SVG: each line's color and width, in both text modes.
        for text in [SvgText::Editable, SvgText::Outlines] {
            let svg = f.to_svg(text).unwrap();
            for (_, style) in &strokes {
                let attributes =
                    format!(r#"stroke="{}" stroke-width="1.25""#, style.color.to_hex());
                assert!(svg.contains(&attributes), "{name}: {attributes}");
            }
        }

        // PNG: the middle of a long straight piece of the center line is its color.
        let (start, end) = center_path
            .verbs()
            .windows(2)
            .find_map(|pair| match pair {
                [
                    PathVerb::MoveTo(a) | PathVerb::LineTo(a),
                    PathVerb::LineTo(b),
                ] if (b.x - a.x).hypot(b.y - a.y) > 5.0 => Some((*a, *b)),
                _ => None,
            })
            .unwrap();
        let at = ((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
        let dpi = 360;
        let png = f.to_png(dpi).unwrap();
        let pixels = image::decode_rgba8(&png).unwrap();
        let scale = dpi as f32 / 72.0;
        let actual = pixels.at((at.0 * scale) as u32, (at.1 * scale) as u32);
        let expected = [center.color.r, center.color.g, center.color.b, 255];
        assert!(
            actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
            "{name}: line pixel {actual:?}, expected {expected:?}"
        );

        // PDF: the same scene; each line stroked in its color at its width.
        let pdf = f.to_pdf().unwrap();
        assert_eq!(pdf, umber_core::render::render_pdf(&scene).unwrap());
        let content = pdf_check::page_content(&pdf);
        let lines: Vec<_> = content.lines().collect();
        let pdf_colors: Vec<Vec<f32>> = lines
            .windows(2)
            .filter(|pair| pair[0].ends_with(" RG") && pair[1] == "1.25 w")
            .map(|pair| {
                pair[0]
                    .split_whitespace()
                    .take(3)
                    .map(|v| v.parse().unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(pdf_colors.len(), 3, "{name}: {pdf_colors:?}");
        for ((_, style), pdf_color) in strokes.iter().zip(&pdf_colors) {
            // PDF normalizes color channels to three decimal places.
            let rgb = [style.color.r, style.color.g, style.color.b]
                .map(|v| (f32::from(v) / 255.0 * 1000.0).round() / 1000.0);
            assert!(
                pdf_color.iter().zip(rgb).all(|(a, b)| (a - b).abs() < 1e-4),
                "{name}: {pdf_color:?}, expected {rgb:?}"
            );
        }
        if poppler {
            let dir = pdf_check::scratch(&format!("contour-contrast-{name}"));
            let path = pdf_check::write(&dir, "contour.pdf", &pdf);
            pdf_check::assert_matches_png(&path, &png, dpi as u32);
        }
    }
}
