//! File-backed fonts in the three themes, using OFL files without system lookup.

use umber_core::fonts::font_bytes;
use umber_core::{AxisData, BundledFont, Font, FontLibrary, LegendStyle, LineOptions, Weight};

use super::super::{Output, axes, emit_scene, figure, labelled, linear, themes};

#[path = "../../fixtures/font_file.rs"]
mod font_fixture;

pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let font = Font::from_files(
        font_bytes(BundledFont::Inter, Weight::Regular).to_vec(),
        Some(font_bytes(BundledFont::Inter, Weight::SemiBold).to_vec()),
        "Inter".into(),
    )
    .unwrap();
    font_cases("font-file", font, lib, sink);
    let fallback = Font::from_files(
        font_bytes(BundledFont::Inter, Weight::Regular).to_vec(),
        None,
        "Inter".into(),
    )
    .unwrap()
    .with_bold_fallback(true)
    .unwrap();
    font_cases("font-file-bold-fallback", fallback, lib, sink);
    for (name, file) in [
        ("font-file-cff", "SourceSerif4-Regular.otf"),
        ("font-file-variable", "SourceSerif4Variable-Roman.ttf"),
    ] {
        let Some(bytes) = font_fixture::read(file) else {
            continue;
        };
        let font = Font::from_files(bytes.clone(), Some(bytes), "Source Serif 4".into()).unwrap();
        font_cases(name, font, lib, sink);
    }
}

fn font_cases(prefix: &str, font: Font, lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        let theme = theme
            .with_title_font(font.clone())
            .unwrap()
            .with_body_font(font.clone())
            .unwrap();
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Revenue and costs");
        f.title("Annual report");
        axes(&mut f)
            .legend(LegendStyle::Direct, umber_core::SizeSpec::Auto)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                &[-1.0, 2.0, 4.0],
                LineOptions {
                    label: "Revenue".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                &[0.0, 1.0, 2.0],
                LineOptions {
                    label: "Costs".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        emit_scene(&format!("{prefix}-{name}"), &f.to_scene(lib).unwrap(), sink);
    }
}
