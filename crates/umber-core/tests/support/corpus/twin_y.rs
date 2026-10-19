//! Two y axes in one panel: category bars and a line, numeric grids, and a logarithmic right axis.

use umber_core::{
    AxisData, BarOptions, ColorBy, ColorbarOptions, ColorbarPosition, Figure, FontLibrary, Grid,
    GridIndex, LegendStyle, LineOptions, Placement, PointColor, ScaleKind, ScatterOptions,
    SizeSpec, ValueLabels,
};

use super::super::{Output, axes, emit_scene, figure, linear, log, themes};

/// Emits figures with right-hand axes in each theme.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        let months: Vec<String> = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let mut f = figure(8.0, 5.0, theme.clone(), ScaleKind::Category, linear());
        let mut left = axes(&mut f);
        left.title("A year of weather")
            .ylabel("Rainfall (mm)")
            .bar(
                months.clone(),
                &[52., 48., 55., 61., 72., 84., 91., 88., 70., 64., 58., 54.],
                BarOptions {
                    label: "Rainfall".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        left.twin_y(linear())
            .unwrap()
            .ylabel("Temperature (°C)")
            .line(
                umber_core::AxisData::Categories(months),
                &[4., 5., 9., 13., 18., 22., 25., 24., 20., 14., 9., 5.],
                LineOptions {
                    label: "Temperature".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("twin-y-weather-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(8.0, 5.0, theme.clone(), ScaleKind::Category, linear());
        let categories: Vec<String> = ["A", "B", "C"].into_iter().map(str::to_owned).collect();
        let mut left = axes(&mut f);
        left.title("Counts and magnitude")
            .ylabel("Count")
            .bar(
                categories.clone(),
                &[2., 4., 3.],
                BarOptions {
                    label: "Count".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        left.twin_y(log(10.0))
            .unwrap()
            .ylabel_sized("Magnitude", SizeSpec::pt(14.).unwrap())
            .bar(
                categories,
                &[10., 100., 1000.],
                BarOptions {
                    label: "Magnitude".into(),
                    annotation: ValueLabels {
                        annotate: true,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("twin-y-log-bars-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(10.0, 6.0, theme.clone(), linear(), linear());
        axes(&mut f).twin_y(linear()).unwrap();
        for (twin, label, values) in [
            (false, "Left line", [1., 2.]),
            (true, "Right line", [200., 100.]),
        ] {
            let mut ax = if twin {
                f.twin_axes_at_mut(GridIndex::Auto, GridIndex::Auto)
                    .unwrap()
            } else {
                axes(&mut f)
            };
            ax.line(
                AxisData::Numbers(vec![0., 1.]),
                &values,
                LineOptions {
                    label: label.into(),
                    ..Default::default()
                },
            )
            .unwrap();
            ax.scatter(
                AxisData::Numbers(vec![0.2, 0.8]),
                &values,
                ScatterOptions {
                    color: PointColor::By(ColorBy::new(vec![0., 10.])),
                    ..Default::default()
                },
            )
            .unwrap();
            ax.colorbar(ColorbarOptions {
                label: if twin { "Right colors" } else { "Left colors" }.into(),
                position: Some(ColorbarPosition::Right),
                ..Default::default()
            })
            .unwrap();
            ax.legend(LegendStyle::Direct, SizeSpec::Auto).unwrap();
        }
        emit_scene(
            &format!("twin-y-direct-colorbars-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = Figure::with_grid(10.0, 7.0, theme, Grid::new(2, 2).unwrap()).unwrap();
        for row in 0..2 {
            for col in 0..2 {
                let mut left = f
                    .add_axes_at(
                        Placement {
                            row: GridIndex::At(row),
                            col: GridIndex::At(col),
                            ..Default::default()
                        },
                        linear(),
                        linear(),
                    )
                    .unwrap();
                left.line(
                    umber_core::AxisData::Numbers(vec![0., 1., 2.]),
                    &[1., 2., 1.5],
                    LineOptions {
                        label: "Rate".into(),
                        ..Default::default()
                    },
                )
                .unwrap();
                if col == 0 {
                    left.twin_y(linear())
                        .unwrap()
                        .ylabel("Total")
                        .line(
                            umber_core::AxisData::Numbers(vec![0., 1., 2.]),
                            &[10000., 80000., 50000.],
                            LineOptions {
                                label: "Total".into(),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
            }
        }
        emit_scene(
            &format!("twin-y-grid-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
}
