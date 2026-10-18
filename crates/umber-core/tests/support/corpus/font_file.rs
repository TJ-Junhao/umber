//! File-backed fonts in the three themes, using OFL files without system lookup.

use umber_core::fonts::font_bytes;
use umber_core::{AxisData, BundledFont, Font, FontLibrary, LegendStyle, LineOptions, Weight};

use super::super::{Output, axes, emit_scene, figure, labelled, linear, themes};

pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let font = Font::from_files(
        font_bytes(BundledFont::Inter, Weight::Regular).to_vec(),
        Some(font_bytes(BundledFont::Inter, Weight::SemiBold).to_vec()),
        "Inter".into(),
    )
    .unwrap();
    font_cases("font-file", font, lib, sink);
    for (name, bytes) in [
        (
            "font-file-cff",
            include_bytes!("../../../../../tests/fonts/SourceSerif4-Regular.otf").as_slice(),
        ),
        (
            "font-file-variable",
            include_bytes!("../../../../../tests/fonts/SourceSerif4Variable-Roman.ttf").as_slice(),
        ),
    ] {
        let font = Font::from_files(
            bytes.to_vec(),
            Some(bytes.to_vec()),
            "Source Serif 4".into(),
        )
        .unwrap();
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
            .legend(LegendStyle::Direct)
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
