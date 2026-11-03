//! Values written next to scatter points in every theme: a horizontal dot-and-whisker chart and
//! a matched-fold chart with points joined by lines, both placed automatically, and labels on a
//! fixed side that grow the automatic limits.

use std::sync::Arc;

use umber_core::{
    Alpha, AxisData, ColorSpec, ErrorBarOptions, FontLibrary, LineOptions, Marker, Place,
    PointColor, PointLabels, ScaleKind, ScatterOptions, SizeSpec, TickFormat,
};

use super::super::{Output, axes, emit_scene, figure, linear, themes};

fn names(values: &[&str]) -> AxisData {
    AxisData::Categories(values.iter().map(|v| (*v).to_owned()).collect())
}

fn palette(index: usize) -> ColorSpec {
    ColorSpec::Palette {
        index,
        alpha: Alpha::OPAQUE,
    }
}

fn labelled(label: &str, index: usize, marker: Marker, place: Place) -> ScatterOptions {
    ScatterOptions {
        label: label.into(),
        color: PointColor::Uniform(palette(index)),
        marker,
        labels: PointLabels {
            annotate: true,
            format: Some(TickFormat::fixed(1).unwrap()),
            place,
            ..PointLabels::default()
        },
        ..ScatterOptions::default()
    }
}

/// Emits the dot-and-whisker, matched-fold and fixed-side figures.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let models = ["Baseline", "Dropout", "Gate", "Gate + aug"];
    // Five-fold means and population standard deviations.
    let series = [
        (
            "80 epochs",
            Marker::Square,
            [66.5, 69.14, 71.64, 73.06],
            [0.66, 0.65, 0.63, 0.63],
        ),
        (
            "Long training",
            Marker::Circle,
            [68.08, 70.7, 74.0, 75.4],
            [0.54, 0.62, 0.65, 0.57],
        ),
    ];
    let folds = ["Fold 1", "Fold 2", "Fold 3", "Fold 4", "Fold 5", "Mean"];
    let matched = [
        (
            "80 epochs",
            Marker::Square,
            [66.8, 68.1, 65.4, 69.0, 67.3, 67.32],
            1.24,
        ),
        (
            "Long training",
            Marker::Circle,
            [78.4, 80.2, 76.9, 81.7, 79.5, 79.34],
            1.6,
        ),
    ];
    for (name, theme) in themes() {
        let mut f = figure(6.4, 3.6, theme.clone(), linear(), ScaleKind::Category);
        let mut ax = axes(&mut f);
        ax.title("Five-fold accuracy by model")
            .xlabel("Accuracy (%)");
        for (i, (label, marker, means, sds)) in series.iter().enumerate() {
            let low: Vec<f64> = means.iter().zip(sds).map(|(m, s)| m - s).collect();
            let high: Vec<f64> = means.iter().zip(sds).map(|(m, s)| m + s).collect();
            ax.x_error_bars_shared(
                names(&models),
                Arc::from(low),
                Arc::from(high),
                ErrorBarOptions {
                    color: palette(i),
                    width: SizeSpec::pt(1.0).unwrap(),
                    ..ErrorBarOptions::default()
                },
            )
            .unwrap()
            .scatter_shared(
                AxisData::Numbers(means.to_vec()),
                names(&models),
                labelled(label, i, *marker, Place::Auto),
            )
            .unwrap();
        }
        emit_scene(
            &format!("point-labels-dot-whisker-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.4, 3.8, theme.clone(), ScaleKind::Category, linear());
        let mut ax = axes(&mut f);
        ax.title("Matched folds")
            .ylabel("Accuracy (%)")
            .ylim(62.0, 85.0)
            .unwrap();
        for (i, (label, marker, values, sd)) in matched.iter().enumerate() {
            let mean = values[5];
            ax.line(
                names(&folds),
                values,
                LineOptions {
                    color: palette(i),
                    width: SizeSpec::pt(0.8).unwrap(),
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .y_error_bars(
                names(&["Mean"]),
                &[mean - sd],
                &[mean + sd],
                ErrorBarOptions {
                    color: palette(i),
                    width: SizeSpec::pt(1.0).unwrap(),
                    ..ErrorBarOptions::default()
                },
            )
            .unwrap()
            .scatter_shared(
                names(&folds),
                AxisData::Numbers(values.to_vec()),
                labelled(label, i, *marker, Place::Auto),
            )
            .unwrap();
        }
        emit_scene(
            &format!("point-labels-matched-folds-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        // Labels on fixed sides at the edges of the data: automatic limits grow to hold them.
        let mut f = figure(4.0, 3.0, theme, linear(), linear());
        let mut ax = axes(&mut f);
        ax.title("Fixed sides");
        let (xs, ys) = (vec![0.0, 4.0, 10.0], vec![0.0, 6.5, 10.0]);
        for (i, place) in [Place::Above, Place::Right].into_iter().enumerate() {
            let shift = 3.0 * i as f64;
            ax.scatter_shared(
                AxisData::Numbers(xs.clone()),
                AxisData::Numbers(ys.iter().map(|y| y + shift).collect()),
                labelled("", i, Marker::Diamond, place),
            )
            .unwrap();
        }
        emit_scene(
            &format!("point-labels-fixed-sides-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
}
