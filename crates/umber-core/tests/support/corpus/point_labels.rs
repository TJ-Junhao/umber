//! Values written next to scatter points in every theme: a horizontal dot-and-whisker chart and
//! a matched-fold chart with points joined by lines, both placed automatically, with the error
//! bars and lines declared on the scatters so that they take the series colors, the second
//! with automatic limits that grow to hold the label above its highest point, and labels on a
//! fixed side that grow the automatic limits.

use std::sync::Arc;

use umber_core::{
    Alpha, AxisData, ColorSpec, ErrorRange, FontLibrary, Marker, Place, PointColor, PointLabels,
    ScaleKind, ScatterOptions, TickFormat,
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

fn labelled(label: &str, marker: Marker, place: Place) -> ScatterOptions {
    ScatterOptions {
        label: label.into(),
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
                    ..labelled(label, *marker, Place::Auto)
                },
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
        // Automatic limits grow to hold the label above the highest point.
        ax.title("Matched folds").ylabel("Accuracy (%)");
        for (label, marker, values, sd) in &matched {
            // Only the mean has an error bar: NaN skips the bars of the folds.
            let mean = values[5];
            let mut low = [f64::NAN; 6];
            let mut high = [f64::NAN; 6];
            (low[5], high[5]) = (mean - sd, mean + sd);
            ax.scatter_shared(
                names(&folds),
                AxisData::Numbers(values.to_vec()),
                ScatterOptions {
                    y_error: Some(ErrorRange::Numbers {
                        low: Arc::from(low),
                        high: Arc::from(high),
                    }),
                    connect: true,
                    ..labelled(label, *marker, Place::Auto)
                },
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
                ScatterOptions {
                    color: PointColor::Uniform(palette(i)),
                    ..labelled("", Marker::Diamond, place)
                },
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
