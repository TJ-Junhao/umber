//! Values written in a column beside the plot in every theme: a forest plot of two series with
//! their error bars and a titled column right of the plot, a row of fold values above a
//! matched-fold chart, a column outside a colorbar, and a row under the y axis labels of a
//! twin-axes panel.

use std::sync::Arc;

use umber_core::{
    AxisData, ColorBy, ErrorRange, FontLibrary, Marker, Place, PointColor, PointLabels, ScaleKind,
    ScatterOptions, SizeSpec, TickFormat,
};

use super::super::{Output, axes, emit_scene, figure, linear, themes};

fn names(values: &[&str]) -> AxisData {
    AxisData::Categories(values.iter().map(|v| (*v).to_owned()).collect())
}

fn in_column(label: &str, marker: Marker) -> ScatterOptions {
    ScatterOptions {
        label: label.into(),
        marker,
        labels: PointLabels {
            annotate: true,
            format: Some(TickFormat::fixed(1).unwrap()),
            place: Place::Column,
            ..PointLabels::default()
        },
        ..ScatterOptions::default()
    }
}

/// Emits the forest, fold row, colorbar and twin-axes figures.
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
    let folds = ["Fold 1", "Fold 2", "Fold 3", "Fold 4", "Fold 5"];
    for (name, theme) in themes() {
        let mut f = figure(6.4, 3.6, theme.clone(), linear(), ScaleKind::Category);
        let mut ax = axes(&mut f);
        ax.title("Five-fold accuracy by model")
            .xlabel("Accuracy (%)")
            .value_column("Mean (%)", SizeSpec::Auto);
        for (label, marker, means, sds) in &series {
            let low: Vec<f64> = means.iter().zip(sds).map(|(m, s)| m - s).collect();
            let high: Vec<f64> = means.iter().zip(sds).map(|(m, s)| m + s).collect();
            ax.scatter_shared(
                AxisData::Numbers(means.to_vec()),
                names(&models),
                ScatterOptions {
                    x_error: Some(ErrorRange::Numbers {
                        low: Arc::from(low),
                        high: Arc::from(high),
                    }),
                    ..in_column(label, *marker)
                },
            )
            .unwrap();
        }
        emit_scene(
            &format!("value-column-forest-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.4, 3.8, theme.clone(), ScaleKind::Category, linear());
        let mut ax = axes(&mut f);
        ax.title("Matched folds")
            .ylabel("Accuracy (%)")
            .value_column("Per fold", SizeSpec::Auto);
        ax.scatter_shared(
            names(&folds),
            AxisData::Numbers(vec![66.8, 68.1, 65.4, 69.0, 67.3]),
            ScatterOptions {
                connect: true,
                ..in_column("80 epochs", Marker::Square)
            },
        )
        .unwrap();
        emit_scene(
            &format!("value-column-fold-row-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        // A column right of the colorbar, whose label it shares the band above the plot with.
        let mut f = figure(6.4, 3.6, theme.clone(), linear(), ScaleKind::Category);
        let mut ax = axes(&mut f);
        ax.title("Accuracy by model")
            .value_column("Mean (%)", SizeSpec::Auto);
        ax.scatter_shared(
            AxisData::Numbers(vec![68.08, 70.7, 74.0, 75.4]),
            names(&models),
            ScatterOptions {
                color: PointColor::By(ColorBy::new(vec![1.2, 2.5, 3.1, 4.8])),
                ..in_column("", Marker::Circle)
            },
        )
        .unwrap();
        emit_scene(
            &format!("value-column-colorbar-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        // A row above a twin-axes panel: both y axis labels sit above it.
        let mut f = figure(6.4, 3.8, theme, ScaleKind::Category, linear());
        let mut ax = axes(&mut f);
        ax.ylabel("Accuracy (%)")
            .value_column("Per fold", SizeSpec::Auto)
            .scatter_shared(
                names(&folds),
                AxisData::Numbers(vec![66.8, 68.1, 65.4, 69.0, 67.3]),
                in_column("Accuracy", Marker::Circle),
            )
            .unwrap();
        ax.twin_y(linear())
            .unwrap()
            .ylabel("Wall time (h)")
            .scatter_shared(
                names(&folds),
                AxisData::Numbers(vec![1.2, 1.5, 1.4, 1.8, 1.6]),
                ScatterOptions {
                    label: "Wall time".into(),
                    marker: Marker::Diamond,
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("value-column-twin-row-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
}
