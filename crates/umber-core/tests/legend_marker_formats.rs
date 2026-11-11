//! Legend swatches of point marks are scene paths, shared by PNG, both SVG text modes and PDF.

use umber_core::scene::{Item, Paint};
use umber_core::{
    AxisData, Figure, FontLibrary, Marker, ScaleKind, ScatterOptions, SizeSpec, SvgText, Theme,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;
use pdf_check::image;

#[test]
fn marker_swatches_agree_in_png_svg_and_pdf() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!(
            "skipped marker-swatch PDF/PNG raster comparison: pdftoppm is not installed; \
             the scene, PNG pixels and SVG and PDF outputs are still checked"
        );
    }
    for marker in [
        Marker::Circle,
        Marker::Square,
        Marker::Triangle,
        Marker::Diamond,
        Marker::Cross,
    ] {
        let mut f = Figure::new(4.0, 3.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                AxisData::Numbers(vec![1.0, 2.0]),
                &[1.0, 2.0],
                ScatterOptions {
                    label: "Points".to_owned(),
                    marker,
                    size: SizeSpec::pt(12.0).unwrap(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        // The legend is an unclipped group; with no bars or areas in the figure, its first fill
        // is the marker swatch.
        let swatch = scene
            .items
            .iter()
            .find_map(|item| match item {
                Item::Group {
                    clip: None, items, ..
                } => items.iter().find(|i| matches!(i, Item::Fill { .. })),
                _ => None,
            })
            .expect("a legend group");
        let Item::Fill {
            path,
            paint: Paint::Solid(_),
            ..
        } = swatch
        else {
            panic!("{marker:?}: the swatch is a solid fill, got {swatch:?}")
        };
        let (lo, hi) = path.bounds().unwrap();
        let (cx, cy) = ((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);

        let dpi = 360;
        let png = f.to_png(dpi).unwrap();
        let pixels = image::decode_rgba8(&png).unwrap();
        let scale = dpi as f32 / 72.0;
        let actual = pixels.at((cx * scale) as u32, (cy * scale) as u32);
        // The swatch center is painted: it differs from the plain background beside the figure.
        let background = pixels.at(2, 2);
        assert_ne!(actual, background, "{marker:?}: swatch pixel is background");

        for text in [SvgText::Editable, SvgText::Outlines] {
            assert_eq!(
                f.to_svg(text, 300).unwrap(),
                umber_core::render::render_svg(&scene, text, 300).unwrap()
            );
        }
        let pdf = f.to_pdf(300).unwrap();
        assert_eq!(pdf, umber_core::render::render_pdf(&scene, 300).unwrap());
        if poppler {
            let dir = pdf_check::scratch(&format!("legend-marker-{marker:?}"));
            let path = pdf_check::write(&dir, "legend.pdf", &pdf);
            pdf_check::assert_matches_png(&path, &png, dpi as u32);
        }
    }
}
