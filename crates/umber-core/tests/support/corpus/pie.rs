//! Pie and donut labels and panel legends in all themes.

use super::super::{Output, emit_scene, themes};
use umber_core::{Figure, FontLibrary, LegendStyle, PieOptions, SizeSpec};

pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (kind, hole, annotate, style) in [
            ("basic", 0.0, false, LegendStyle::Direct),
            ("donut", 0.5, false, LegendStyle::Direct),
            ("annotated", 0.0, true, LegendStyle::Direct),
            ("legend", 0.5, false, LegendStyle::Header),
            ("chips", 0.0, true, LegendStyle::Chips),
            ("outside", 0.0, true, LegendStyle::Direct),
        ] {
            let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
            let mut axes = f.add_pie_axes().unwrap();
            let values = if kind == "outside" {
                [80.0, 15.0, 3.0, 2.0]
            } else {
                [40.0, 30.0, 20.0, 10.0]
            };
            axes.title("Orders by channel")
                .pie(
                    ["Web", "App", "Shop", "Other"].map(String::from).to_vec(),
                    &values,
                    PieOptions {
                        hole,
                        annotate,
                        ..Default::default()
                    },
                )
                .unwrap()
                .legend(style, None, SizeSpec::Auto)
                .unwrap();
            emit_scene(
                &format!("pie-{kind}-{name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
