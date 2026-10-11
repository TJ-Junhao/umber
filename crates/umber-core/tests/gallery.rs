//! The golden gallery: one image per theme and chart kind, pinned to `tests/golden/gallery_*.png`.
//!
//! Every image is 6 x 4 in at 150 dpi and is built only through the public figure API. Text
//! is shaped with the bundled fonts alone (no system fallback), so the output does not depend
//! on the fonts installed on the machine. Run with `UPDATE_GOLDEN=1` to write the references.
//!
//! The ignored test `write_contact_sheet` collects every image into
//! `target/umber-gallery/index.html` for looking at them side by side.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here.
#![allow(clippy::unwrap_used, clippy::panic)]

#[path = "support/golden_compare.rs"]
mod golden_compare;

use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::PathBuf;

use umber_core::render::render_png;
use umber_core::{
    AreaOptions, AxisData, BarOptions, BarSeries, ColorSpec, Dash, Figure, FillStyle, FontLibrary,
    HistOptions, LegendStyle, LineOptions, LogBase, Marker, ScaleKind, ScatterOptions, Theme,
};

use golden_compare::{assert_golden, golden_dir};

const DPI: u32 = 150;
const TITLE: &str = "Station output";
const SUBTITLE: &str = "Daily mean, MW";
const YLABEL: &str = "Daily mean, MW";

/// `(name, caption)` of every gallery image, in contact-sheet order.
const IMAGES: [(&str, &str); 27] = [
    ("pigment_line", "Pigment: three sine series"),
    ("graphite_line", "Graphite: three sine series"),
    ("midnight_line", "Midnight: three sine series"),
    ("pigment_scatter", "Pigment: three clusters"),
    ("graphite_scatter", "Graphite: three clusters"),
    ("midnight_scatter", "Midnight: three clusters"),
    ("pigment_log_line", "Pigment: log y axis, 1 to 1e5"),
    ("pigment_nan_gaps", "Pigment: line with NaN gaps"),
    ("graphite_category_line", "Graphite: six categories on x"),
    ("midnight_empty", "Midnight: title only"),
    (
        "pigment_grouped_bar",
        "Pigment: grouped bars, header legend",
    ),
    (
        "graphite_grouped_bar",
        "Graphite: grouped bars, direct fallback to a row",
    ),
    (
        "midnight_grouped_bar",
        "Midnight: grouped bars, chips, rounded tops",
    ),
    ("pigment_hist", "Pigment: histogram, no gap"),
    ("graphite_hist", "Graphite: histogram, 1.2 pt gap"),
    (
        "midnight_hist",
        "Midnight: histogram, gradient with top edges",
    ),
    ("pigment_stacked_bar", "Pigment: stacked bars, mixed signs"),
    (
        "graphite_bar_negative",
        "Graphite: bars with negative heights",
    ),
    (
        "midnight_hist_area_off",
        "Midnight: histogram with a solid fill",
    ),
    (
        "pigment_legend_wrap",
        "Pigment: nine labelled lines, wrapping header row",
    ),
    (
        "graphite_direct_crowded",
        "Graphite: six lines ending close together",
    ),
    (
        "graphite_direct_xlim",
        "Graphite: direct labels with line ends cut by xlim",
    ),
    (
        "midnight_chips_long_labels",
        "Midnight: chips with long labels",
    ),
    ("pigment_legend_none", "Pigment: labelled lines, legend off"),
    ("pigment_area", "Pigment: an area and a dashed line"),
    ("graphite_area", "Graphite: an area and a dashed line"),
    ("midnight_area", "Midnight: an area and a dashed line"),
];

/// The four panels per theme: `(image kind, caption)`.
const GRID_KINDS: [(&str, &str); 4] = [
    ("line", "line"),
    ("scatter", "scatter"),
    ("grouped_bar", "grouped bar"),
    ("hist", "histogram"),
];

/// The three theme blocks of the reference image `theme-directions.png`: `(theme, top, height)` in pixels.
const REFERENCE_BLOCKS: [(&str, u32, u32); 3] = [
    ("pigment", 119, 650),
    ("graphite", 894, 646),
    ("midnight", 1669, 643),
];
const REFERENCE_SIZE: (u32, u32) = (940, 2500);
/// The Midnight line panel of the reference image: `(left, top, width, height)` in pixels.
const REFERENCE_MIDNIGHT_LINE_PANEL: (u32, u32, u32, u32) = (25, 1687, 427, 294);

/// Renders with the bundled fonts only: no system fallback, so output is machine independent.
fn render(figure: &Figure) -> Vec<u8> {
    let library = FontLibrary::with_fallbacks(Vec::new());
    render_png(&figure.to_scene(&library), DPI).unwrap()
}

fn check(name: &str, figure: &Figure) {
    assert_golden(&format!("gallery_{name}"), &render(figure));
}

fn theme(name: &str) -> Theme {
    match name {
        "pigment" => Theme::pigment(),
        "graphite" => Theme::graphite(),
        "midnight" => Theme::midnight(),
        other => panic!("unknown theme {other}"),
    }
}

fn numbers(values: Vec<f64>) -> AxisData {
    AxisData::Numbers(values)
}

fn line_figure(theme_name: &str) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, theme(theme_name)).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title(TITLE).subtitle(SUBTITLE).xlabel("Day");
    let x: Vec<f64> = (0..60).map(f64::from).collect();
    for (i, label) in ["North", "Central", "South"].into_iter().enumerate() {
        let k = i as f64;
        let y: Vec<f64> = x
            .iter()
            .map(|t| {
                40.0 + 10.0 * k + (12.0 - 2.0 * k) * (t * 2.0 * PI / (30.0 + 6.0 * k) + k).sin()
            })
            .collect();
        axes.line(
            numbers(x.clone()),
            &y,
            LineOptions {
                label: label.to_owned(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    figure
}

/// A small linear congruential generator, so the clusters are the same on every machine.
struct Lcg(u64);

impl Lcg {
    /// Uniform in [0, 1).
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Approximately normal (sum of four uniforms), mean 0, standard deviation about 1.
    fn normal(&mut self) -> f64 {
        let sum: f64 = (0..4).map(|_| self.next()).sum();
        (sum - 2.0) * 3.0_f64.sqrt()
    }
}

fn scatter_figure(theme_name: &str) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, theme(theme_name)).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title(TITLE)
        .subtitle("Each dot is one sampled plant")
        .xlabel("Temperature, C");
    let mut rng = Lcg(2024);
    let centers = [(20.0, 30.0), (35.0, 55.0), (50.0, 35.0)];
    for (i, (cx, cy)) in centers.into_iter().enumerate() {
        let (x, y): (Vec<f64>, Vec<f64>) = (0..40)
            .map(|_| (cx + 4.0 * rng.normal(), cy + 6.0 * rng.normal()))
            .unzip();
        axes.scatter(
            numbers(x),
            &y,
            ScatterOptions {
                label: format!("Cluster {}", i + 1),
                color: ColorSpec::AUTO,
                marker: Marker::Circle,
            },
        )
        .unwrap();
    }
    figure
}

#[test]
fn pigment_line() {
    check("pigment_line", &line_figure("pigment"));
}

#[test]
fn graphite_line() {
    check("graphite_line", &line_figure("graphite"));
}

#[test]
fn midnight_line() {
    check("midnight_line", &line_figure("midnight"));
}

#[test]
fn pigment_scatter() {
    check("pigment_scatter", &scatter_figure("pigment"));
}

#[test]
fn graphite_scatter() {
    check("graphite_scatter", &scatter_figure("graphite"));
}

#[test]
fn midnight_scatter() {
    check("midnight_scatter", &scatter_figure("midnight"));
}

#[test]
fn pigment_log_line() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN })
        .unwrap();
    axes.title(TITLE).ylabel(YLABEL).xlabel("Day");
    let x: Vec<f64> = (0..=50).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|t| 10f64.powf(t / 10.0)).collect();
    axes.line(numbers(x), &y, LineOptions::default()).unwrap();
    check("pigment_log_line", &figure);
}

#[test]
fn pigment_nan_gaps() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title(TITLE).ylabel(YLABEL).xlabel("Day");
    let x: Vec<f64> = (0..60).map(f64::from).collect();
    let y: Vec<f64> = x
        .iter()
        .map(|&t| {
            if (15.0..22.0).contains(&t) || (40.0..44.0).contains(&t) {
                f64::NAN
            } else {
                50.0 + 15.0 * (t * 2.0 * PI / 30.0).sin()
            }
        })
        .collect();
    axes.line(numbers(x), &y, LineOptions::default()).unwrap();
    check("pigment_nan_gaps", &figure);
}

#[test]
fn graphite_category_line() {
    let mut figure = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap();
    axes.title(TITLE).ylabel(YLABEL).xlabel("Month");
    let months: Vec<String> = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for (i, label) in ["Wind", "Solar"].into_iter().enumerate() {
        let k = i as f64;
        let y: Vec<f64> = (0..6)
            .map(|m| 30.0 + 8.0 * k + 10.0 * (f64::from(m) * 0.9 + 2.0 * k).sin())
            .collect();
        axes.line(
            AxisData::Categories(months.clone()),
            &y,
            LineOptions {
                label: label.to_owned(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    check("graphite_category_line", &figure);
}

#[test]
fn midnight_empty() {
    let mut figure = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title(TITLE);
    check("midnight_empty", &figure);
}

const QUARTERS: [&str; 4] = ["Q1", "Q2", "Q3", "Q4"];

fn quarters() -> Vec<String> {
    QUARTERS.map(str::to_owned).to_vec()
}

fn revenue_series() -> Vec<BarSeries> {
    [
        ("2024", [12.0, 18.0, 15.0, 22.0]),
        ("2025", [9.0, 14.0, 19.0, 17.0]),
        ("2026", [5.0, 8.0, 11.0, 14.0]),
    ]
    .map(|(label, heights)| BarSeries {
        label: label.to_owned(),
        heights: heights.to_vec(),
        color: ColorSpec::AUTO,
    })
    .to_vec()
}

fn grouped_bar_figure(theme_name: &str) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, theme(theme_name)).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap();
    axes.title("Revenue by quarter").subtitle("USD, millions");
    axes.grouped_bar(quarters(), revenue_series()).unwrap();
    figure
}

/// Log-normal values from a seeded generator, so the histogram is the same on every machine.
fn response_times() -> Vec<f64> {
    let mut rng = Lcg(1400);
    (0..1400)
        .map(|_| (3.0 + 0.45 * rng.normal()).exp())
        .collect()
}

fn hist_figure(theme: Theme) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Response time").subtitle("1,400 requests, ms");
    axes.hist(&response_times(), HistOptions::default())
        .unwrap();
    figure
}

/// A 6 x 4 in figure in `theme` with a title and `count` labelled wobbling lines.
fn many_lines(theme: Theme, title: &str, labels: &[String]) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title(title).xlabel("Day");
    let x: Vec<f64> = (0..30).map(f64::from).collect();
    for (i, label) in labels.iter().enumerate() {
        let k = i as f64;
        let y: Vec<f64> = x
            .iter()
            .map(|t| 20.0 + 3.0 * k + 5.0 * (t / 4.0 + k).sin())
            .collect();
        axes.line(
            numbers(x.clone()),
            &y,
            LineOptions {
                label: label.clone(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    figure
}

/// The first two series of the line images: the first as an area, the second as a dashed line.
fn area_figure(theme_name: &str) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, theme(theme_name)).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title(TITLE).subtitle(SUBTITLE).xlabel("Day");
    let x: Vec<f64> = (0..60).map(f64::from).collect();
    let series = |k: f64| -> Vec<f64> {
        x.iter()
            .map(|t| {
                40.0 + 10.0 * k + (12.0 - 2.0 * k) * (t * 2.0 * PI / (30.0 + 6.0 * k) + k).sin()
            })
            .collect()
    };
    axes.area(
        numbers(x.clone()),
        &series(0.0),
        AreaOptions {
            label: "North".to_owned(),
            ..AreaOptions::default()
        },
    )
    .unwrap()
    .line(
        numbers(x.clone()),
        &series(1.0),
        LineOptions {
            label: "Central".to_owned(),
            dash: Dash::Dashed,
            ..LineOptions::default()
        },
    )
    .unwrap();
    figure
}

#[test]
fn pigment_area() {
    check("pigment_area", &area_figure("pigment"));
}

#[test]
fn graphite_area() {
    check("graphite_area", &area_figure("graphite"));
}

#[test]
fn midnight_area() {
    check("midnight_area", &area_figure("midnight"));
}

#[test]
fn pigment_grouped_bar() {
    check("pigment_grouped_bar", &grouped_bar_figure("pigment"));
}

#[test]
fn graphite_grouped_bar() {
    check("graphite_grouped_bar", &grouped_bar_figure("graphite"));
}

#[test]
fn midnight_grouped_bar() {
    check("midnight_grouped_bar", &grouped_bar_figure("midnight"));
}

#[test]
fn pigment_hist() {
    check("pigment_hist", &hist_figure(Theme::pigment()));
}

#[test]
fn graphite_hist() {
    check("graphite_hist", &hist_figure(Theme::graphite()));
}

#[test]
fn midnight_hist() {
    check("midnight_hist", &hist_figure(Theme::midnight()));
}

#[test]
fn pigment_stacked_bar() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap();
    axes.title("Cash flow by quarter").subtitle("USD, millions");
    let series = [
        ("Sales", [14.0, 18.0, 12.0, 20.0]),
        ("Services", [6.0, -4.0, 9.0, 7.0]),
        ("Costs", [-8.0, -10.0, -6.0, -12.0]),
    ]
    .map(|(label, heights)| BarSeries {
        label: label.to_owned(),
        heights: heights.to_vec(),
        color: ColorSpec::AUTO,
    })
    .to_vec();
    axes.stacked_bar(quarters(), series).unwrap();
    check("pigment_stacked_bar", &figure);
}

#[test]
fn graphite_bar_negative() {
    let mut figure = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap();
    axes.title("Net change by quarter")
        .subtitle("USD, millions");
    axes.bar(quarters(), &[8.0, -5.0, 12.0, -9.0], BarOptions::default())
        .unwrap();
    check("graphite_bar_negative", &figure);
}

#[test]
fn midnight_hist_area_off() {
    let theme = Theme::midnight().with_fill_style(FillStyle::Solid).unwrap();
    check("midnight_hist_area_off", &hist_figure(theme));
}

#[test]
fn pigment_legend_wrap() {
    let labels: Vec<String> = (1..=9).map(|i| format!("Station {i}")).collect();
    check(
        "pigment_legend_wrap",
        &many_lines(Theme::pigment(), "Nine stations", &labels),
    );
}

#[test]
fn graphite_direct_crowded() {
    let mut figure = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Six lines, close ends").xlabel("Day");
    let x: Vec<f64> = (0..=20).map(f64::from).collect();
    for k in 0..6 {
        let k = f64::from(k);
        let end = 50.0 + 1.5 * k;
        let start = 20.0 + 12.0 * ((k * 7.0) % 6.0);
        let y: Vec<f64> = x
            .iter()
            .map(|t| start + (end - start) * t / 20.0 + 2.0 * (t / 3.0 + k).sin())
            .collect();
        axes.line(
            numbers(x.clone()),
            &y,
            LineOptions {
                label: format!("Region {}", k + 1.0),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    check("graphite_direct_crowded", &figure);
}

#[test]
fn graphite_direct_xlim() {
    let labels: Vec<String> = ["North", "Central", "South"].map(str::to_owned).to_vec();
    let mut figure = many_lines(Theme::graphite(), "Cut by an explicit limit", &labels);
    figure.axes_mut().unwrap().xlim(0.0, 20.5).unwrap();
    check("graphite_direct_xlim", &figure);
}

#[test]
fn midnight_chips_long_labels() {
    let labels: Vec<String> = [
        "Northern coastal stations, hourly mean",
        "Inland valley stations, hourly mean",
        "Mountain ridge stations, hourly mean",
        "Offshore platforms",
    ]
    .map(str::to_owned)
    .to_vec();
    check(
        "midnight_chips_long_labels",
        &many_lines(Theme::midnight(), "Long labels", &labels),
    );
}

#[test]
fn pigment_legend_none() {
    let labels: Vec<String> = ["North", "Central", "South"].map(str::to_owned).to_vec();
    let mut figure = many_lines(Theme::pigment(), "Legend off", &labels);
    figure.axes_mut().unwrap().legend(LegendStyle::None);
    check("pigment_legend_none", &figure);
}

/// Writes every gallery image and the local design reference images, when present, with an `index.html`, to
/// `target/umber-gallery/`. Run with `--ignored`.
#[test]
#[ignore = "writes the contact sheet; run explicitly"]
fn write_contact_sheet() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("umber-gallery");
    std::fs::create_dir_all(&out).unwrap();

    let mut cards = String::new();
    // The design reference images are local files, in `docs/design` or in the directory named by
    // `UMBER_DESIGN_DIR`; they are skipped where absent (CI).
    let design = std::env::var_os("UMBER_DESIGN_DIR").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("docs")
                .join("design")
        },
        PathBuf::from,
    );
    for (file, caption) in [
        ("theme-directions.png", "Reference: theme directions"),
        ("graphite-navy.png", "Reference: graphite navy"),
    ] {
        let source = design.join(file);
        if source.is_file() {
            std::fs::copy(&source, out.join(file)).unwrap();
            // The reference image itself is shown as crops beside the theme grids below.
            if file != "theme-directions.png" {
                let _ = writeln!(
                    cards,
                    "<figure><img src=\"{file}\"><figcaption>{caption}</figcaption></figure>"
                );
            }
        }
    }
    let caption_of = |name: &str| {
        IMAGES
            .iter()
            .find(|(n, _)| *n == name)
            .map_or("", |(_, c)| *c)
    };
    let copy_image = |name: &str| -> String {
        let file = format!("gallery_{name}.png");
        std::fs::copy(golden_dir().join(&file), out.join(&file)).unwrap();
        file
    };

    // The reference layout: per theme, line, scatter, grouped bar and histogram in a 2 x 2 grid,
    // next to the matching crop of the design reference image.
    let reference = design.join("theme-directions.png");
    let mut grid_names = Vec::new();
    for (theme_name, top, height) in REFERENCE_BLOCKS {
        let mut panels = String::new();
        for (kind, caption) in GRID_KINDS {
            let name = format!("{theme_name}_{kind}");
            let file = copy_image(&name);
            let _ = writeln!(
                panels,
                "<figure><img src=\"{file}\"><figcaption>{name}: {caption}</figcaption></figure>"
            );
            grid_names.push(name);
        }
        let mut reference_crop = String::new();
        if reference.is_file() {
            // A percentage background offset places the crop at `top` of the sheet.
            let y = f64::from(top) / f64::from(REFERENCE_SIZE.1 - height) * 100.0;
            let _ = write!(
                reference_crop,
                "<div class=\"crop\" style=\"aspect-ratio:{w}/{height};\
                 background-position:0 {y:.3}%\"></div>",
                w = REFERENCE_SIZE.0
            );
        }
        let _ = writeln!(
            cards,
            "<section class=\"pair\"><div class=\"grid\">{panels}</div>{reference_crop}</section>"
        );
    }
    // The Midnight area image beside the reference's Midnight line panel, which shows the
    // gradient look.
    if reference.is_file() {
        let file = copy_image("midnight_area");
        let (x, y, w, h) = REFERENCE_MIDNIGHT_LINE_PANEL;
        let px = f64::from(x) / f64::from(REFERENCE_SIZE.0 - w) * 100.0;
        let py = f64::from(y) / f64::from(REFERENCE_SIZE.1 - h) * 100.0;
        let zoom = f64::from(REFERENCE_SIZE.0) / f64::from(w) * 100.0;
        let _ = writeln!(
            cards,
            "<h2>Area</h2><section class=\"pair\"><figure><img src=\"{file}\">\
             <figcaption>midnight_area</figcaption></figure>\
             <div class=\"crop\" style=\"aspect-ratio:{w}/{h};background-size:{zoom:.2}% auto;\
             background-position:{px:.3}% {py:.3}%\"></div></section>"
        );
    }
    grid_names.push("midnight_area".to_owned());
    let mut rest = String::new();
    for (name, _) in IMAGES {
        if grid_names.iter().any(|n| n == name) {
            continue;
        }
        let file = copy_image(name);
        let _ = writeln!(
            rest,
            "<figure><img src=\"{file}\"><figcaption>{name}: {}</figcaption></figure>",
            caption_of(name)
        );
    }
    let _ = writeln!(cards, "<h2>Edge cases</h2><main>\n{rest}</main>");
    let html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <title>umber gallery</title>\n\
         <style>body{{font-family:sans-serif;margin:24px;background:#eee}}\
         main,.grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(440px,1fr));gap:20px}}\
         .grid{{grid-template-columns:repeat(2,1fr)}}\
         .pair{{display:grid;grid-template-columns:3fr 2fr;gap:20px;align-items:start;\
         margin-bottom:28px}}\
         .crop{{width:100%;background:url(theme-directions.png) no-repeat;\
         background-size:100% auto;border:1px solid #bbb}}\
         figure{{margin:0}}img{{width:100%;border:1px solid #bbb}}\
         figcaption{{font-size:13px;padding-top:4px}}</style></head>\n\
         <body><h1>umber gallery</h1>\n{cards}</body></html>\n"
    );
    std::fs::write(out.join("index.html"), html).unwrap();
}
