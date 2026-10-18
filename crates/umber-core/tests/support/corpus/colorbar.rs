//! Colorbar positions, legends, formats and theme metrics.

use super::super::{Output, axes, emit_scene, figure, linear, numbers, themes};
use std::fmt::Write as _;
use umber_core::{
    ColorBy, ColorbarOptions, ColorbarPosition, FontLibrary, LegendPosition, LegendStyle,
    LineOptions, PointColor, ScatterOptions, TickFormat,
};

/// Emits both positions, with and without legends, in each theme.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut metrics = String::new();
    for (name, theme) in themes() {
        let _ = writeln!(
            metrics,
            "{name}: position={:?} thickness={} gap={} tick_length={}",
            theme.colorbar_position(),
            theme.colorbar_thickness(),
            theme.colorbar_gap(),
            theme.colorbar_tick_length()
        );
        for (position_name, position, legend_position) in [
            ("right", ColorbarPosition::Right, LegendPosition::Right),
            ("bottom", ColorbarPosition::Bottom, LegendPosition::Bottom),
        ] {
            for legend in [false, true] {
                let mut f = figure(6.4, 4.4, theme.clone(), linear(), linear());
                let mut ax = axes(&mut f);
                ax.title("Temperature by distance")
                    .xlabel("Distance, km")
                    .ylabel("Depth, m");
                ax.scatter(
                    numbers(vec![1.0, 2.0, 3.0, 4.0]),
                    &[4.0, 2.0, 3.0, 1.0],
                    ScatterOptions {
                        color: PointColor::By(ColorBy {
                            center: Some(0.0),
                            ..ColorBy::new(vec![-2.0, 0.0, 3.0, 8.0])
                        }),
                        ..ScatterOptions::default()
                    },
                )
                .unwrap();
                if legend {
                    ax.line(
                        numbers(vec![1.0, 4.0]),
                        &[2.0, 2.0],
                        LineOptions {
                            label: "Reference".into(),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
                    ax.legend_at(
                        LegendStyle::Header,
                        legend_position,
                        umber_core::SizeSpec::Auto,
                    )
                    .unwrap();
                }
                ax.colorbar(ColorbarOptions {
                    label: "Temperature".into(),
                    position: Some(position),
                    format: Some(TickFormat::fixed(0).unwrap().affix("", " °C").unwrap()),
                    ..ColorbarOptions::default()
                })
                .unwrap();
                let suffix = if legend { "-legend" } else { "" };
                emit_scene(
                    &format!("colorbar-{position_name}{suffix}-{name}"),
                    &f.to_scene(lib).unwrap(),
                    sink,
                );
            }
        }
    }
    sink(Output {
        kind: "api",
        file: "themes-colorbar.txt".into(),
        bytes: metrics.into_bytes(),
    });
}
