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
    AxisData, ColorSpec, Figure, FontLibrary, LineOptions, LogBase, Marker, ScaleKind,
    ScatterOptions, Theme,
};

use golden_compare::{assert_golden, golden_dir};

const DPI: u32 = 150;
const TITLE: &str = "Station output";
const SUBTITLE: &str = "Daily mean, MW";
const YLABEL: &str = "Daily mean, MW";

/// `(name, caption)` of every gallery image, in contact-sheet order.
const IMAGES: [(&str, &str); 10] = [
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
];

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
                ..ScatterOptions::default()
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
            let _ = writeln!(
                cards,
                "<figure><img src=\"{file}\"><figcaption>{caption}</figcaption></figure>"
            );
        }
    }
    for (name, caption) in IMAGES {
        let file = format!("gallery_{name}.png");
        std::fs::copy(golden_dir().join(&file), out.join(&file)).unwrap();
        let _ = writeln!(
            cards,
            "<figure><img src=\"{file}\"><figcaption>{name}: {caption}</figcaption></figure>"
        );
    }
    let html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <title>umber gallery</title>\n\
         <style>body{{font-family:sans-serif;margin:24px;background:#eee}}\
         main{{display:grid;grid-template-columns:repeat(auto-fill,minmax(440px,1fr));gap:20px}}\
         figure{{margin:0}}img{{width:100%;border:1px solid #bbb}}\
         figcaption{{font-size:13px;padding-top:4px}}</style></head>\n\
         <body><h1>umber gallery</h1><main>\n{cards}</main></body></html>\n"
    );
    std::fs::write(out.join("index.html"), html).unwrap();
}
