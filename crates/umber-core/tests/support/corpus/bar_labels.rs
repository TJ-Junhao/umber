//! Values written on bars in every theme: plain bars with a negative value, horizontal bars,
//! grouped and stacked bars, a log value axis, and explicit limits with room for the labels.

use umber_core::{
    BarOptions, ColorSpec, FontLibrary, ScaleKind, Series, SizeSpec, TickFormat, ValueLabels,
};

use super::super::{Output, axes, emit_scene, figure, linear, log, themes};

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

fn series(label: &str, values: &[f64]) -> Series {
    Series {
        label: label.into(),
        values: values.to_vec(),
        color: ColorSpec::AUTO,
    }
}

fn shown() -> ValueLabels {
    ValueLabels {
        annotate: true,
        ..ValueLabels::default()
    }
}

fn bars() -> BarOptions {
    BarOptions {
        annotation: shown(),
        ..BarOptions::default()
    }
}

/// Emits annotated bars of every arrangement and orientation.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let six = names(&["A", "B", "C", "D", "E", "F"]);
    for (name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        axes(&mut f)
            .title("Net change")
            .ylabel("Orders")
            .bar(six.clone(), &[28.4, 42.0, -16.5, 36.2, 52.8, 31.5], bars())
            .unwrap();
        emit_scene(
            &format!("bar-labels-vertical-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), ScaleKind::Category);
        axes(&mut f)
            .title("Orders by team")
            .xlabel("Orders")
            .bar(six.clone(), &[28.4, 42.0, 16.5, 36.2, 52.8, 31.5], bars())
            .unwrap();
        emit_scene(
            &format!("bar-labels-horizontal-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let quarters = names(&["Q1", "Q2", "Q3", "Q4"]);
        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        axes(&mut f)
            .title("Orders by quarter")
            .grouped_bar_annotated(
                quarters.clone(),
                vec![
                    series("Online", &[24.5, 31.2, 42.0, 38.6]),
                    series("Retail", &[18.2, 26.3, 30.8, 34.1]),
                ],
                shown(),
            )
            .unwrap();
        emit_scene(
            &format!("bar-labels-grouped-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        axes(&mut f)
            .title("Components of each total")
            .ylabel("Orders")
            .stacked_bar_annotated(
                quarters.clone(),
                vec![
                    series("Core", &[32.0, 38.0, 29.0, 41.0]),
                    series("Service", &[21.0, 24.0, 26.0, 20.0]),
                    series("Other", &[18.0, 22.0, 19.0, 25.0]),
                ],
                shown(),
            )
            .unwrap();
        emit_scene(
            &format!("bar-labels-stacked-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), ScaleKind::Category);
        axes(&mut f)
            .title("Stacked rows")
            .stacked_bar_annotated(
                quarters.clone(),
                vec![
                    series("Core", &[32.0, 38.0, 29.0, 41.0]),
                    series("Returns", &[-12.0, -9.0, -14.0, -10.0]),
                ],
                ValueLabels {
                    annotate: true,
                    format: Some(TickFormat::fixed(1).unwrap()),
                    size: SizeSpec::pt(8.0).unwrap(),
                },
            )
            .unwrap();
        emit_scene(
            &format!("bar-labels-stacked-horizontal-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, log(10.0));
        axes(&mut f)
            .title("Log value axis")
            .bar(names(&["A", "B", "C"]), &[3.0, 420.0, 9_000.0], bars())
            .unwrap();
        emit_scene(
            &format!("bar-labels-log-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(4.7, 2.55, theme, ScaleKind::Category, linear());
        axes(&mut f)
            .title("Explicit limits")
            .ylim(-40.0, 130.0)
            .unwrap()
            .bar(
                six.clone(),
                &[50.0, 60.0, 80.0, -10.0, 65.0, 100.0],
                BarOptions {
                    annotation: ValueLabels {
                        annotate: true,
                        format: Some(TickFormat::fixed(0).unwrap().affix("", " k").unwrap()),
                        size: SizeSpec::Auto,
                    },
                    ..BarOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("bar-labels-limits-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
}
