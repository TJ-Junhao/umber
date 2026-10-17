//! Output cases for heatmaps: an annotated correlation matrix centered at zero in each theme, a
//! week of counts with an empty cell, drawn as vector cells and as one image, counts on a log
//! color scale, and their errors.

use std::fmt::Write as _;

use umber_core::{
    Cmap, CmapChoice, ColorbarOptions, ColorbarPosition, Figure, FontLibrary, HeatmapOptions,
    LogBase, Render, ScaleKind, Theme, TickFormat, Transform,
};

use super::super::{Output, axes, emit_scene, figure, record, themes};

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

fn categories() -> ScaleKind {
    ScaleKind::Category
}

const METRICS: [&str; 7] = [
    "Revenue", "Users", "Orders", "Churn", "Latency", "Support", "Margin",
];

/// Pearson correlations of the seven metrics, row by row.
const CORRELATIONS: [f64; 49] = [
    1.00, 0.62, 0.81, -0.47, -0.12, 0.35, 0.58, //
    0.62, 1.00, 0.74, -0.31, 0.08, 0.66, 0.21, //
    0.81, 0.74, 1.00, -0.42, -0.05, 0.52, 0.44, //
    -0.47, -0.31, -0.42, 1.00, 0.38, 0.17, -0.55, //
    -0.12, 0.08, -0.05, 0.38, 1.00, 0.29, -0.23, //
    0.35, 0.66, 0.52, 0.17, 0.29, 1.00, 0.02, //
    0.58, 0.21, 0.44, -0.55, -0.23, 0.02, 1.00,
];

/// The correlation matrix, centered at zero with the theme's diverging colormap, each cell
/// annotated with two decimals.
fn correlations(theme: Theme) -> Figure {
    let mut f = figure(6.4, 5.2, theme, categories(), categories());
    axes(&mut f)
        .title("Metric correlations")
        .subtitle("Pearson r over 400 days, centered at 0")
        .heatmap(
            &names(&METRICS),
            &names(&METRICS),
            &CORRELATIONS,
            HeatmapOptions {
                center: Some(0.0),
                annotate: true,
                format: Some(TickFormat::fixed(2).unwrap()),
                ..HeatmapOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "Correlation".into(),
            ..ColorbarOptions::default()
        })
        .unwrap();
    f
}

/// Visits by day and period with the theme's sequential colormap, one cell empty, and the
/// colorbar below.
fn week() -> Figure {
    week_drawn(Render::Auto)
}

/// [`week`] with its cells drawn as `render` says.
fn week_drawn(render: Render) -> Figure {
    let mut f = figure(4.8, 3.6, Theme::graphite(), categories(), categories());
    axes(&mut f)
        .title("Visits by day")
        .heatmap(
            &names(&["Mon", "Tue", "Wed", "Thu", "Fri"]),
            &names(&["Morning", "Noon", "Evening", "Night"]),
            &[
                42.0,
                51.0,
                47.0,
                55.0,
                61.0, //
                88.0,
                93.0,
                90.0,
                97.0,
                104.0, //
                65.0,
                f64::NAN,
                70.0,
                74.0,
                81.0, //
                12.0,
                15.0,
                11.0,
                18.0,
                26.0,
            ],
            HeatmapOptions {
                render,
                ..HeatmapOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "Visits".into(),
            position: Some(ColorbarPosition::Bottom),
            ..ColorbarOptions::default()
        })
        .unwrap();
    f
}

/// Counts over three orders of magnitude on a log color scale with a fixed colormap, annotated
/// with the automatic labels.
fn log_counts() -> Figure {
    let mut f = figure(4.8, 3.2, Theme::midnight(), categories(), categories());
    axes(&mut f)
        .title("Requests by region")
        .heatmap(
            &names(&["2023", "2024", "2025"]),
            &names(&["North", "South"]),
            &[3.0, 40.0, 520.0, 12.0, 260.0, 2400.0],
            HeatmapOptions {
                cmap: CmapChoice::Fixed(Cmap::magma()),
                scale: Transform::Log(LogBase::TEN),
                annotate: true,
                ..HeatmapOptions::default()
            },
        )
        .unwrap();
    f
}

/// Records `result` of a call on `f`, which must leave `f` unchanged when it fails.
fn record_call(
    out: &mut String,
    label: &str,
    f: &mut Figure,
    call: impl FnOnce(&mut Figure) -> umber_core::Result<()>,
) {
    let before = f.clone();
    match call(f) {
        Ok(()) => {
            let _ = writeln!(out, "{label}: ok");
        }
        Err(err) => {
            assert_eq!(*f, before, "{label}");
            let _ = writeln!(
                out,
                "{label}: {}\nparameter={:?} hint={:?}",
                record::error(&err),
                err.parameter(),
                err.hint()
            );
        }
    }
}

/// Emits the scenes and the errors of invalid heatmaps.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        emit_scene(
            &format!("heatmap-correlation-{name}"),
            &correlations(theme).to_scene(lib).unwrap(),
            sink,
        );
    }
    emit_scene("heatmap-week", &week().to_scene(lib).unwrap(), sink);
    emit_scene("heatmap-log", &log_counts().to_scene(lib).unwrap(), sink);
    emit_scene(
        "heatmap-raster",
        &week_drawn(Render::Raster).to_scene(lib).unwrap(),
        sink,
    );

    let mut out = String::new();
    let add = |x: &'static [&'static str],
               y: &'static [&'static str],
               values: Vec<f64>,
               options: HeatmapOptions| {
        move |f: &mut Figure| {
            axes(f)
                .heatmap(&names(x), &names(y), &values, options)
                .map(drop)
        }
    };
    let mut numeric = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Linear, categories());
    record_call(
        &mut out,
        "linear x axis",
        &mut numeric,
        add(&["a"], &["b"], vec![1.0], HeatmapOptions::default()),
    );
    let mut f = figure(6.0, 4.0, Theme::pigment(), categories(), categories());
    record_call(
        &mut out,
        "wrong length",
        &mut f,
        add(
            &["a", "b", "c"],
            &["p", "q", "r", "s"],
            vec![0.0; 11],
            HeatmapOptions::default(),
        ),
    );
    record_call(
        &mut out,
        "repeated x category",
        &mut f,
        add(&["a", "a"], &["p"], vec![0.0; 2], HeatmapOptions::default()),
    );
    record_call(
        &mut out,
        "repeated y category",
        &mut f,
        add(
            &["a"],
            &["p", "q", "p"],
            vec![0.0; 3],
            HeatmapOptions::default(),
        ),
    );
    record_call(
        &mut out,
        "infinite value",
        &mut f,
        add(
            &["a", "b"],
            &["p", "q"],
            vec![0.0, 1.0, f64::NEG_INFINITY, 2.0],
            HeatmapOptions::default(),
        ),
    );
    record_call(
        &mut out,
        "zero on a log scale",
        &mut f,
        add(
            &["a", "b"],
            &["p"],
            vec![1.0, 0.0],
            HeatmapOptions {
                scale: Transform::Log(LogBase::TEN),
                ..HeatmapOptions::default()
            },
        ),
    );
    record_call(
        &mut out,
        "center with a log scale",
        &mut f,
        add(
            &["a"],
            &["p"],
            vec![1.0],
            HeatmapOptions {
                scale: Transform::Log(LogBase::TEN),
                center: Some(1.0),
                ..HeatmapOptions::default()
            },
        ),
    );
    record_call(
        &mut out,
        "format without annotate",
        &mut f,
        add(
            &["a"],
            &["p"],
            vec![1.0],
            HeatmapOptions {
                format: Some(TickFormat::percent(0).unwrap()),
                ..HeatmapOptions::default()
            },
        ),
    );
    record_call(
        &mut out,
        "iso format",
        &mut f,
        add(
            &["a"],
            &["p"],
            vec![1.0],
            HeatmapOptions {
                annotate: true,
                format: Some(TickFormat::iso()),
                ..HeatmapOptions::default()
            },
        ),
    );
    let viridis = HeatmapOptions {
        cmap: CmapChoice::Fixed(Cmap::viridis()),
        ..HeatmapOptions::default()
    };
    record_call(
        &mut out,
        "first heatmap",
        &mut f,
        add(&["a"], &["p"], vec![1.0], viridis.clone()),
    );
    record_call(
        &mut out,
        "other cmap",
        &mut f,
        add(
            &["b"],
            &["p"],
            vec![2.0],
            HeatmapOptions {
                cmap: CmapChoice::Fixed(Cmap::cividis()),
                ..viridis.clone()
            },
        ),
    );
    record_call(
        &mut out,
        "other scale",
        &mut f,
        add(
            &["b"],
            &["p"],
            vec![2.0],
            HeatmapOptions {
                scale: Transform::Log(LogBase::TEN),
                ..viridis.clone()
            },
        ),
    );
    record_call(
        &mut out,
        "other center",
        &mut f,
        add(
            &["b"],
            &["p"],
            vec![2.0],
            HeatmapOptions {
                center: Some(1.5),
                ..viridis.clone()
            },
        ),
    );
    record_call(
        &mut out,
        "second heatmap",
        &mut f,
        add(&["b"], &["q"], vec![2.0], viridis),
    );
    let mut limited = figure(6.0, 4.0, Theme::pigment(), categories(), categories());
    axes(&mut limited).clim(0.0, 10.0).unwrap();
    record_call(
        &mut out,
        "log after clim",
        &mut limited,
        add(
            &["a"],
            &["p"],
            vec![1.0],
            HeatmapOptions {
                scale: Transform::Log(LogBase::TEN),
                ..HeatmapOptions::default()
            },
        ),
    );
    record_call(
        &mut out,
        "center after clim",
        &mut limited,
        add(
            &["a"],
            &["p"],
            vec![1.0],
            HeatmapOptions {
                center: Some(12.0),
                ..HeatmapOptions::default()
            },
        ),
    );
    sink(Output {
        kind: "errors",
        file: "heatmap.txt".into(),
        bytes: out.into_bytes(),
    });
}
