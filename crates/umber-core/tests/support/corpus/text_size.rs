//! Per-call text sizes, independent tick labels, legends and colorbar titles in every theme.

use umber_core::{
    ColorBy, ColorbarOptions, ColorbarPosition, Figure, FontLibrary, Grid, GridIndex, LegendStyle,
    LineOptions, Placement, PointColor, ScatterOptions, Share, SizeSpec,
};

use super::super::{Output, axes, emit_scene, figure, linear, numbers, themes};

/// Passes the text size scenes to `sink` in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (suffix, size, style) in [
            ("points", SizeSpec::pt(12.0).unwrap(), LegendStyle::Header),
            ("times", SizeSpec::times(1.5).unwrap(), LegendStyle::Direct),
            ("chips", SizeSpec::pt(16.0).unwrap(), LegendStyle::Chips),
        ] {
            let mut f = figure(
                9.0,
                7.0,
                theme.clone().with_font_scale(1.4).unwrap(),
                linear(),
                linear(),
            );
            f.title_sized("Figure title", size)
                .subtitle_sized("Figure subtitle", size);
            axes(&mut f)
                .line(
                    numbers(vec![0.0, 1.0, 2.0]),
                    &[1.0, 3.0, 2.0],
                    LineOptions {
                        label: "Measured series".into(),
                        ..LineOptions::default()
                    },
                )
                .unwrap()
                .title_sized("Panel title", size)
                .subtitle_sized("Panel subtitle", size)
                .xlabel_sized("Horizontal", size)
                .ylabel_sized("Vertical", size)
                .xtick_labels(SizeSpec::pt(10.0).unwrap())
                .unwrap()
                .ytick_labels(SizeSpec::pt(18.0).unwrap())
                .unwrap()
                .legend(style, size)
                .unwrap();
            emit_scene(
                &format!("text-size-{suffix}-{name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
        for (suffix, position) in [
            ("right", ColorbarPosition::Right),
            ("bottom", ColorbarPosition::Bottom),
        ] {
            let mut f = figure(8.0, 6.0, theme.clone(), linear(), linear());
            axes(&mut f)
                .scatter(
                    numbers(vec![0.0, 1.0, 2.0]),
                    &[1.0, 3.0, 2.0],
                    ScatterOptions {
                        color: PointColor::By(ColorBy::new(vec![0.0, 5.0, 10.0])),
                        ..ScatterOptions::default()
                    },
                )
                .unwrap()
                .xtick_labels(SizeSpec::pt(14.0).unwrap())
                .unwrap()
                .ytick_labels(SizeSpec::pt(16.0).unwrap())
                .unwrap()
                .colorbar(ColorbarOptions {
                    label: "Intensity".into(),
                    position: Some(position),
                    size: SizeSpec::pt(20.0).unwrap(),
                    ..ColorbarOptions::default()
                })
                .unwrap();
            emit_scene(
                &format!("text-size-colorbar-{suffix}-{name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
        for (suffix, position) in [
            ("right", ColorbarPosition::Right),
            ("bottom", ColorbarPosition::Bottom),
        ] {
            for (scope, count, size) in [
                ("panel", 1, SizeSpec::pt(72.0).unwrap()),
                ("shared", 2, SizeSpec::pt(30.0).unwrap()),
            ] {
                let grid = Grid::new(count, count)
                    .unwrap()
                    .with_share_color(if count == 1 { Share::None } else { Share::All });
                let mut f = Figure::with_grid(12.0, 8.0, theme.clone(), grid).unwrap();
                for row in 0..count {
                    for col in 0..count {
                        let mut ax = f
                            .add_axes_at(
                                Placement {
                                    row: GridIndex::At(row),
                                    col: GridIndex::At(col),
                                    ..Placement::default()
                                },
                                linear(),
                                linear(),
                            )
                            .unwrap();
                        ax.scatter(
                            numbers(vec![0.0, 1.0, 2.0]),
                            &[1.0, 3.0, 2.0],
                            ScatterOptions {
                                color: PointColor::By(ColorBy::new(vec![0.0, 5.0, 10.0])),
                                ..ScatterOptions::default()
                            },
                        )
                        .unwrap()
                        .ylabel_sized("gypsy", size);
                        if count == 1 {
                            ax.colorbar(ColorbarOptions {
                                label: "gypsy".into(),
                                position: Some(position),
                                size,
                                ..ColorbarOptions::default()
                            })
                            .unwrap();
                        }
                    }
                }
                if count > 1 {
                    f.colorbar(ColorbarOptions {
                        label: "gypsy".into(),
                        position: Some(position),
                        size,
                        ..ColorbarOptions::default()
                    })
                    .unwrap();
                }
                emit_scene(
                    &format!("text-size-descenders-{scope}-{suffix}-{name}"),
                    &f.to_scene(lib).unwrap(),
                    sink,
                );
            }
        }
        let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
        let mut f = Figure::with_grid(12.0, 5.0, theme, grid).unwrap();
        f.title_sized("Shared labels", SizeSpec::pt(22.0).unwrap());
        for col in [0, 1] {
            f.add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                linear(),
                linear(),
            )
            .unwrap()
            .line(
                numbers(vec![0.0, 1.0]),
                &[col as f64, 2.0],
                LineOptions {
                    label: format!("Series {col}"),
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .xtick_labels(SizeSpec::pt(14.0).unwrap())
            .unwrap()
            .legend(LegendStyle::Header, SizeSpec::pt(12.0).unwrap())
            .unwrap();
        }
        emit_scene(
            &format!("text-size-grid-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
}
