//! The corpus behind the refactor output gate.
//!
//! Every case is a figure, a hand-built scene, or a list of public API calls. The gate records,
//! for each scene, a fixed-schema text record of the Scene (every float written as its bit
//! pattern), the SVG bytes in both text modes and the PNG bytes at two DPIs; and, for the API
//! calls, the returned values, every error by category, context, message and display text, and
//! the observable state after each call. See `scripts/check-refactor-output.sh`. The corpus uses
//! only public APIs and the in-memory fallback font fixture, never system fonts, so one revision
//! gives the same bytes on every run. The records never use the `Debug` output of umber types,
//! so a change to a `Debug` implementation does not change them. Changing an existing case
//! changes its outputs, so every baseline has to be regenerated afterwards.
//!
//! This file is included with `#[path]` by `examples/refactor_outputs.rs`.
//!
//! # Coverage
//!
//! Covered:
//!
//! - Figures in the three built-in themes, and in themes changed through every `Theme::with_*`
//!   setter that affects line and scatter figures.
//! - Line and scatter marks: every dash and marker, every color spec, palette wrap-around, NaN
//!   gaps, all-NaN, single-point and constant data, and series of 200 000 points.
//! - Linear, base-2 and base-10 log, and category scales; limits that are narrow, tiny, huge or
//!   far from the data; data far outside the limits.
//! - Headers and axis labels: all four labels, none, and the x label, the y label and the
//!   subtitle each on its own. `font_scale` at its maximum of 10 and at 1e-30. Labels wider than
//!   the layout's text width limit, which are cut to a prefix on a character boundary (in
//!   `corpus/x_end_label.rs`, with tiny figures and crowded category names).
//! - Text: every bundled font in both weights, with and without tabular figures, as scene text
//!   and as layout metrics; characters drawn only by the in-memory fallback font; characters no
//!   font draws.
//! - Hand-built scenes: every item kind, stroke caps, joins and dashes, gradient edge cases,
//!   fill rules, groups nested at and beyond `MAX_GROUP_DEPTH`, and non-finite, huge and
//!   degenerate geometry, text and sizes.
//! - Every getter of `Theme::pigment()`, `Theme::graphite()`, `Theme::midnight()` and
//!   `Theme::default()`, and every public constant except `VERSION`, which changes with each
//!   release.
//! - Every reachable error branch of `Figure::new`, `Figure::add_axes`, `Axes::xlim`,
//!   `Axes::ylim`, `Axes::line`, `Axes::scatter`, `Rgba::hex`, `Rgba::from_ints`, `Alpha::new`,
//!   `LogBase::new`, `ContinuousScale::new`, `ContinuousScale::from_extent`, every validating
//!   `Theme::with_*` setter and `render_png`, including which fault is reported when an input
//!   has several. A setter or constructor that succeeds is recorded by its full returned value
//!   (a whole `Theme`, for instance). After every call on a figure, the scales, labels, limits,
//!   marks with the bits of every stored value, palette and resolved colors are recorded, so a
//!   call that fails must also leave the figure unchanged. Series include NaN gaps with
//!   non-canonical payloads of both signs, which storage must keep bit for bit.
//! - `Figure::clone`: a figure with labels, limits and marks is recorded with its clone, and
//!   again after the clone is changed, so the clone must be equal to and independent of its
//!   source.
//! - Figures with several axes on a grid, and the errors of the grid calls (in
//!   `corpus/grid.rs`).
//!
//! Not covered:
//!
//! - System fonts. The system-font index, its lookup budgets and the font caches are verified
//!   by dedicated in-memory unit tests, not by this gate.
//! - `Figure::to_png` and `Figure::to_svg`, which lay text out with `FontLibrary::global()` and
//!   so may read system fonts. The corpus calls `Figure::to_scene` with its own library and
//!   then the renderers, which is what these two methods do.
//! - The pixmap allocation and PNG encoding failures of `render_png`; inputs that pass its size
//!   checks do not reach them. `Error::Io`, which no public function returns.
//! - Scale, tick, category and downsampling helpers (`ContinuousScale::nice`, `ticks` and
//!   `normalize`, `CategoryScale`, `m4_indices`), path helpers such as `Path::bounds`, and
//!   `ColorSpec::resolve` are observed only through the figures and scenes that use them.
//! - Whether clones share or copy their series data internally; only the observable
//!   independence of a clone is recorded here.
//! - The font files and license texts returned by `fonts::font_bytes` and
//!   `fonts::license_text`, beyond what shaping reads from them.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;

use umber_core::figure::{AxisData, LineOptions, ScatterOptions};
use umber_core::render::{SvgText, render_png, render_svg};
use umber_core::scene::{
    FillRule, GradientStop, Item, LineCap, LineJoin, LinearGradient, MAX_GROUP_DEPTH, Paint, Rect,
    Scene, StrokeStyle,
};
use umber_core::{
    Alpha, ColorSpec, ContinuousScale, Dash, Figure, FillStyle, Font, FontLibrary, GridIndex,
    GridLines, LegendStyle, LogBase, Marker, Path, Point, Rgba, ScaleKind, TextStyle, Theme,
    Transform, Weight,
};

/// Cases of the newer features, one module each, so that a feature edits only its own file.
mod corpus {
    pub mod area;
    pub mod band;
    pub mod bars;
    pub mod r#box;
    pub mod bubbles;
    pub mod error_bars;
    pub mod grid;
    pub mod header;
    pub mod hist;
    pub mod inputs;
    pub mod legend;
    pub mod legend_position;
    pub mod plot_height;
    pub mod rules;
    pub mod share;
    pub mod size;
    pub mod stacked_area;
    pub mod stem;
    pub mod step;
    pub mod ticks;
    pub mod violin;
    pub mod x_end_label;
}

/// PNG resolutions of every case.
pub const DPIS: [u32; 2] = [72, 200];

/// One generated file.
pub struct Output {
    /// `scene`, `svg`, `png`, `errors` or `api`; also the directory the driver writes it to.
    pub kind: &'static str,
    /// File name inside the kind directory.
    pub file: String,
    pub bytes: Vec<u8>,
}

/// The bundled fonts plus the one in-memory fallback fixture, and no system font lookup.
pub fn library() -> FontLibrary {
    FontLibrary::with_fallbacks(vec![
        include_bytes!("../fixtures/fallback-only.ttf").to_vec(),
    ])
}

/// Renders every case and passes each output to `sink`, in a fixed order.
pub fn generate(mut sink: impl FnMut(Output)) {
    let lib = library();
    for (name, scene) in scene_cases(&lib) {
        emit_scene(&name, &scene, &mut sink);
    }
    let mut text = |kind: &'static str, file: &str, body: String| {
        sink(Output {
            kind,
            file: file.into(),
            bytes: body.into_bytes(),
        });
    };
    text("errors", "api.txt", error_cases(&lib));
    text("api", "themes.txt", theme_cases());
    text("api", "constants.txt", constant_cases());
    text("api", "text-layout.txt", text_layout_cases(&lib));
    corpus::area::cases(&lib, &mut sink);
    corpus::bars::cases(&lib, &mut sink);
    corpus::hist::cases(&lib, &mut sink);
    corpus::inputs::cases(&lib, &mut sink);
    corpus::legend::cases(&lib, &mut sink);
    corpus::legend_position::cases(&lib, &mut sink);
    corpus::size::cases(&lib, &mut sink);
    corpus::ticks::cases(&lib, &mut sink);
    corpus::x_end_label::cases(&lib, &mut sink);
    corpus::plot_height::cases(&lib, &mut sink);
    corpus::grid::cases(&lib, &mut sink);
    corpus::header::cases(&lib, &mut sink);
    corpus::step::cases(&lib, &mut sink);
    corpus::rules::cases(&lib, &mut sink);
    corpus::band::cases(&lib, &mut sink);
    corpus::error_bars::cases(&lib, &mut sink);
    corpus::share::cases(&lib, &mut sink);
    corpus::stacked_area::cases(&lib, &mut sink);
    corpus::stem::cases(&lib, &mut sink);
    corpus::bubbles::cases(&lib, &mut sink);
    corpus::r#box::cases(&lib, &mut sink);
    corpus::violin::cases(&lib, &mut sink);
}

fn emit_scene(name: &str, scene: &Scene, sink: &mut impl FnMut(Output)) {
    sink(Output {
        kind: "scene",
        file: format!("{name}.txt"),
        bytes: record::scene(scene).into_bytes(),
    });
    for (mode, suffix) in [
        (SvgText::Editable, "editable"),
        (SvgText::Outlines, "outlines"),
    ] {
        sink(Output {
            kind: "svg",
            file: format!("{name}.{suffix}.svg"),
            bytes: render_svg(scene, mode).into_bytes(),
        });
    }
    for dpi in DPIS {
        // A scene the PNG backend refuses is recorded by its error.
        let bytes = match render_png(scene, dpi) {
            Ok(bytes) => bytes,
            Err(e) => format!("{}\n", record::error(&e)).into_bytes(),
        };
        sink(Output {
            kind: "png",
            file: format!("{name}.dpi{dpi}.png"),
            bytes,
        });
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba { r, g, b, a: 255 }
}

fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

/// A small deterministic generator in `[0, 1)`; integer arithmetic only, so it never depends on
/// the platform's math library.
fn lcg(seed: u32) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
        f64::from((state >> 8) & 0xffff) / 65536.0
    }
}

fn themes() -> [(&'static str, Theme); 3] {
    [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ]
}

fn numbers(values: Vec<f64>) -> AxisData {
    AxisData::Numbers(values)
}

fn linear() -> ScaleKind {
    ScaleKind::Linear
}

fn log(base: f64) -> ScaleKind {
    ScaleKind::Log {
        base: LogBase::new(base).unwrap(),
    }
}

fn line() -> LineOptions {
    LineOptions::default()
}

fn scatter() -> ScatterOptions {
    ScatterOptions::default()
}

fn figure(w: f64, h: f64, theme: Theme, x: ScaleKind, y: ScaleKind) -> Figure {
    let mut figure = Figure::new(w, h, theme).unwrap();
    figure.add_axes(x, y).unwrap();
    figure
}

fn axes(figure: &mut Figure) -> umber_core::AxesMut<'_> {
    figure
        .axes_at_mut(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
}

fn to_scene(figure: &Figure, lib: &FontLibrary) -> Scene {
    figure.to_scene(lib).unwrap()
}

/// A line of `n` points: a ramp with a deterministic wobble.
fn wobble(n: usize, seed: u32) -> (Vec<f64>, Vec<f64>) {
    let mut next = lcg(seed);
    let x: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let mut level = 0.0;
    let y = x
        .iter()
        .map(|_| {
            level += next() - 0.5;
            level
        })
        .collect();
    (x, y)
}

fn labelled(figure: &mut Figure, title: &str) {
    axes(figure)
        .title(title)
        .subtitle("Fixture subtitle")
        .xlabel("Horizontal")
        .ylabel("Vertical");
}

fn scene_cases(lib: &FontLibrary) -> Vec<(String, Scene)> {
    let mut cases: Vec<(String, Scene)> = Vec::new();
    let mut add = |name: &str, scene: Scene| cases.push((name.to_owned(), scene));

    // ---- Figures: themes ----
    for (theme_name, theme) in themes() {
        let mut empty = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut empty, "Empty axes");
        add(
            &format!("fig-{theme_name}-empty-axes"),
            to_scene(&empty, lib),
        );

        let mut full = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut full, "Line and scatter");
        let (x, y) = wobble(40, 7);
        let (x2, y2) = wobble(25, 99);
        axes(&mut full)
            .line(numbers(x.clone()), &y, line())
            .unwrap();
        axes(&mut full)
            .line(
                numbers(x),
                &y.iter().map(|v| v * 0.5 + 2.0).collect::<Vec<_>>(),
                LineOptions {
                    label: "second".into(),
                    color: ColorSpec::Palette {
                        index: 3,
                        alpha: Alpha::new(0.6).unwrap(),
                    },
                    dash: Dash::Dashed,
                    ..LineOptions::default()
                },
            )
            .unwrap();
        axes(&mut full)
            .scatter(numbers(x2), &y2, scatter())
            .unwrap();
        add(
            &format!("fig-{theme_name}-line-scatter"),
            to_scene(&full, lib),
        );
    }
    add(
        "fig-no-axes",
        to_scene(&Figure::new(2.0, 1.0, Theme::midnight()).unwrap(), lib),
    );

    // ---- Figures: theme tokens ----
    let tokens = [
        Theme::pigment()
            .with_grid_lines(GridLines::Vertical)
            .unwrap()
            .with_grid_dash(Dash::Dotted)
            .unwrap()
            .with_fill_style(FillStyle::Gradient)
            .unwrap(),
        Theme::graphite()
            .with_grid_lines(GridLines::Both)
            .unwrap()
            .with_legend_style(LegendStyle::Chips)
            .unwrap()
            .with_line_width(4.5)
            .unwrap()
            .with_marker_size(9.0)
            .unwrap()
            .with_marker_alpha(0.4)
            .unwrap(),
        Theme::midnight()
            .with_grid_lines(GridLines::None)
            .unwrap()
            .with_legend_style(LegendStyle::Direct)
            .unwrap()
            .with_title_font(Font::SourceSerif4)
            .unwrap()
            .with_body_font(Font::Geist)
            .unwrap()
            .with_font_scale(1.4)
            .unwrap()
            .with_baseline_width(3.0)
            .unwrap(),
        Theme::pigment()
            .with_palette(&[rgb(200, 30, 30), rgb(30, 200, 30)])
            .unwrap()
            .with_background(rgb(250, 245, 230))
            .unwrap()
            .with_text_color(rgb(10, 10, 60))
            .unwrap()
            .with_muted_color(rgb(90, 90, 120))
            .unwrap()
            .with_grid_color(rgb(210, 205, 190))
            .unwrap()
            .with_baseline_color(rgb(40, 40, 40))
            .unwrap(),
    ];
    for (i, theme) in tokens.into_iter().enumerate() {
        let mut f = figure(5.0, 3.5, theme, linear(), linear());
        labelled(&mut f, "Theme tokens");
        for k in 0..3 {
            let (x, y) = wobble(30, 11 + k);
            axes(&mut f).line(numbers(x), &y, line()).unwrap();
        }
        let (x, y) = wobble(15, 5);
        axes(&mut f).scatter(numbers(x), &y, scatter()).unwrap();
        add(&format!("fig-theme-tokens-{i}"), to_scene(&f, lib));
    }

    // ---- Figures: marks ----
    let mut dashes = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut dashes, "Dashes and colors");
    for (k, dash) in [Dash::Solid, Dash::Dashed, Dash::Dotted]
        .into_iter()
        .enumerate()
    {
        let x: Vec<f64> = (0..8).map(f64::from).collect();
        let y: Vec<f64> = x.iter().map(|v| v + 3.0 * k as f64).collect();
        axes(&mut dashes)
            .line(
                numbers(x),
                &y,
                LineOptions {
                    label: format!("dash {k}"),
                    color: ColorSpec::Fixed(rgb(20 + 80 * k as u8, 90, 200)),
                    dash,
                    ..LineOptions::default()
                },
            )
            .unwrap();
    }
    // More automatic series than palette entries, so the palette wraps.
    for k in 0..9 {
        let x: Vec<f64> = (0..5).map(f64::from).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|v| v * 0.3 + f64::from(k) * 0.5 - 4.0)
            .collect();
        axes(&mut dashes).line(numbers(x), &y, line()).unwrap();
    }
    add("fig-line-dashes-and-palette-wrap", to_scene(&dashes, lib));

    let mut markers = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    labelled(&mut markers, "Markers");
    for (k, marker) in [
        Marker::Circle,
        Marker::Square,
        Marker::Triangle,
        Marker::Diamond,
        Marker::Cross,
    ]
    .into_iter()
    .enumerate()
    {
        let mut next = lcg(40 + k as u32);
        let x: Vec<f64> = (0..12).map(|_| next() * 10.0).collect();
        let y: Vec<f64> = (0..12).map(|_| next() * 2.0 + k as f64 * 2.0).collect();
        axes(&mut markers)
            .scatter(
                numbers(x),
                &y,
                ScatterOptions {
                    label: record::marker(marker).into(),
                    color: if k == 4 {
                        ColorSpec::Fixed(rgb(220, 80, 20))
                    } else {
                        ColorSpec::Auto(Alpha::new(0.7).unwrap())
                    },
                    marker,
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
    }
    add("fig-scatter-markers", to_scene(&markers, lib));

    // ---- Figures: scales ----
    for (name, x_scale, y_scale, xs, ys) in [
        (
            "fig-log-y",
            linear(),
            log(10.0),
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            vec![1.0, 10.0, 1000.0, 20.0, 5.0e5],
        ),
        (
            "fig-log-both-base2",
            log(2.0),
            log(2.0),
            vec![1.0, 2.0, 4.0, 16.0, 1024.0],
            vec![0.5, 3.0, 7.0, 0.01, 65536.0],
        ),
        (
            "fig-log-both-base10-small",
            log(10.0),
            log(10.0),
            vec![1.0e-6, 1.0e-3, 1.0, 1.0e3],
            vec![2.0e-9, 4.0e-4, 0.3, 8.0e2],
        ),
    ] {
        let mut f = figure(6.0, 4.0, Theme::pigment(), x_scale, y_scale);
        labelled(&mut f, name);
        axes(&mut f).line(numbers(xs.clone()), &ys, line()).unwrap();
        axes(&mut f).scatter(numbers(xs), &ys, scatter()).unwrap();
        add(name, to_scene(&f, lib));
    }

    let names: Vec<String> = ["north", "south", "east", "west", "north", "centre"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let mut category = figure(6.0, 4.0, Theme::midnight(), ScaleKind::Category, linear());
    labelled(&mut category, "Categories");
    axes(&mut category)
        .line(
            AxisData::Categories(names.clone()),
            &[3.0, 1.0, 4.0, 1.0, 5.0, 9.0],
            line(),
        )
        .unwrap();
    axes(&mut category)
        .scatter(
            AxisData::Categories(vec!["west".into(), "up".into(), "centre".into()]),
            &[2.0, 6.0, 5.0],
            scatter(),
        )
        .unwrap();
    add("fig-category", to_scene(&category, lib));

    // ---- Figures: data edge cases ----
    let mut gaps = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut gaps, "NaN gaps");
    let nan = f64::NAN;
    axes(&mut gaps)
        .line(
            numbers((0..12).map(f64::from).collect()),
            &[1.0, 2.0, nan, 4.0, 5.0, nan, nan, 3.0, 2.0, nan, 8.0, 9.0],
            line(),
        )
        .unwrap();
    axes(&mut gaps)
        .line(
            numbers(vec![
                0.0, nan, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0,
            ]),
            &[nan, 3.0, 3.5, 4.0, 4.5, nan, 5.0, 5.5, 6.0, 6.5, 7.0, nan],
            LineOptions {
                dash: Dash::Dotted,
                ..line()
            },
        )
        .unwrap();
    axes(&mut gaps)
        .scatter(
            numbers((0..6).map(f64::from).collect()),
            &[nan, 7.0, nan, 6.0, 9.0, nan],
            scatter(),
        )
        .unwrap();
    add("fig-nan-gaps", to_scene(&gaps, lib));

    let mut all_nan = figure(4.0, 3.0, Theme::graphite(), linear(), linear());
    axes(&mut all_nan)
        .line(numbers(vec![0.0, 1.0, 2.0]), &[nan, nan, nan], line())
        .unwrap();
    add("fig-all-nan", to_scene(&all_nan, lib));

    let mut single = figure(4.0, 3.0, Theme::pigment(), linear(), linear());
    axes(&mut single)
        .scatter(numbers(vec![5.0]), &[5.0], scatter())
        .unwrap();
    add("fig-single-point", to_scene(&single, lib));

    let mut flat = figure(4.0, 3.0, Theme::pigment(), linear(), linear());
    axes(&mut flat)
        .line(numbers(vec![1.0, 2.0, 3.0]), &[7.0, 7.0, 7.0], line())
        .unwrap();
    add("fig-constant-values", to_scene(&flat, lib));

    for (name, xlim, ylim, x0) in [
        (
            "fig-narrow-limits-offset",
            (1.0, 1.0 + 1.0e-9),
            (5.0, 5.0 + 1.0e-9),
            1.0,
        ),
        (
            "fig-narrow-limits-tiny",
            (0.0, 1.0e-12),
            (0.0, 1.0e-12),
            0.0,
        ),
        (
            "fig-narrow-limits-huge",
            (-1.0e300, 1.0e300),
            (-1.0e300, 1.0e300),
            0.0,
        ),
    ] {
        let mut f = figure(5.0, 3.5, Theme::pigment(), linear(), linear());
        labelled(&mut f, name);
        axes(&mut f).xlim(xlim.0, xlim.1).unwrap();
        axes(&mut f).ylim(ylim.0, ylim.1).unwrap();
        let x: Vec<f64> = (0..5).map(|i| x0 + f64::from(i) * 2.5e-10).collect();
        let y: Vec<f64> = (0..5).map(|i| ylim.0 + f64::from(i) * 2.5e-10).collect();
        axes(&mut f).line(numbers(x.clone()), &y, line()).unwrap();
        axes(&mut f).scatter(numbers(x), &y, scatter()).unwrap();
        add(name, to_scene(&f, lib));
    }

    let mut log_narrow = figure(5.0, 3.5, Theme::graphite(), log(10.0), log(10.0));
    axes(&mut log_narrow).xlim(1.0, 1.0001).unwrap();
    axes(&mut log_narrow).ylim(1.0e-300, 1.0e300).unwrap();
    axes(&mut log_narrow)
        .line(
            numbers(vec![1.00001, 1.00005, 1.00009]),
            &[1.0e-200, 1.0, 1.0e200],
            line(),
        )
        .unwrap();
    add("fig-log-extreme-limits", to_scene(&log_narrow, lib));

    // Data far outside the limits is clipped.
    let mut clipped = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    labelled(&mut clipped, "Clipped");
    axes(&mut clipped).xlim(2.0, 6.0).unwrap();
    axes(&mut clipped).ylim(-1.0, 1.0).unwrap();
    axes(&mut clipped)
        .line(
            numbers(vec![0.0, 1.0, 3.0, 5.0, 7.0, 1.0e6, 1.0e300]),
            &[0.0, 2.0e9, -5.0, 0.5, 1.0e300, -1.0e300, 0.0],
            line(),
        )
        .unwrap();
    axes(&mut clipped)
        .scatter(
            numbers(vec![1.0, 2.0, 4.0, 6.0, 8.0, -1.0e300]),
            &[0.0, 1.0, -1.0, 0.99, 3.0, 0.0],
            scatter(),
        )
        .unwrap();
    add("fig-extreme-clipping", to_scene(&clipped, lib));

    let mut huge = figure(7.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut huge, "Huge data");
    let (x, y) = wobble(200_000, 2024);
    axes(&mut huge).line(numbers(x), &y, line()).unwrap();
    let mut next = lcg(77);
    let sx: Vec<f64> = (0..30_000).map(|_| next() * 200_000.0).collect();
    let sy: Vec<f64> = (0..30_000).map(|_| (next() - 0.5) * 300.0).collect();
    axes(&mut huge)
        .scatter(numbers(sx), &sy, scatter())
        .unwrap();
    add("fig-huge-data", to_scene(&huge, lib));

    let mut huge_nan = figure(7.0, 4.0, Theme::graphite(), linear(), linear());
    let (x, mut y) = wobble(120_000, 31);
    for i in (0..y.len()).step_by(997) {
        y[i] = nan;
    }
    axes(&mut huge_nan).line(numbers(x), &y, line()).unwrap();
    add("fig-huge-data-with-gaps", to_scene(&huge_nan, lib));

    // ---- Figures: text ----
    // U+E000 is drawn only by the in-memory fallback font. The accented and CJK characters
    // have no fallback here and fall back to the missing-glyph path.
    let mut text = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut text)
        .title("Fallback \u{E000} title \u{2212}12.5% caf\u{e9}")
        .subtitle("\u{E000}\u{E000} no system fonts \u{65e5}\u{672c}")
        .xlabel("x \u{E000}")
        .ylabel("\u{2212}y");
    axes(&mut text)
        .line(
            numbers(vec![-3.0, 0.0, 3.0]),
            &[-1000.0, 0.5, 12345.678],
            line(),
        )
        .unwrap();
    add("fig-text-fallback", to_scene(&text, lib));

    let mut wide = figure(14.0, 2.0, Theme::midnight(), linear(), linear());
    labelled(&mut wide, "Wide and short");
    let (x, y) = wobble(60, 3);
    axes(&mut wide).line(numbers(x), &y, line()).unwrap();
    add("fig-wide-short", to_scene(&wide, lib));

    // ---- Hand-built scenes ----
    add("scene-empty", Scene::new(60.0, 40.0, rgb(255, 255, 255)));
    add("scene-zero-size", Scene::new(0.0, -5.0, rgb(255, 255, 255)));
    add("scene-nan-size", Scene::new(f32::NAN, 30.0, rgb(0, 0, 0)));
    add("scene-primitives", primitives_scene(lib));
    add("scene-strokes", strokes_scene());
    add("scene-gradients", gradients_scene());
    add("scene-groups-and-clips", groups_scene());
    add("scene-markers", markers_scene());
    add("scene-hostile", hostile_scene());
    add("scene-hostile-text", hostile_text_scene(lib));
    add("scene-font-weights", font_weights_scene(lib));
    label_cases(lib, &mut add);
    cases
}

fn polygon(points: &[(f32, f32)]) -> Path {
    let mut path = Path::new();
    for (i, (x, y)) in points.iter().enumerate() {
        if i == 0 {
            path.move_to(pt(*x, *y));
        } else {
            path.line_to(pt(*x, *y));
        }
    }
    path.close();
    path
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
}

fn circle(r: f32) -> Path {
    const K: f32 = 0.552_284_8;
    let k = r * K;
    let mut path = Path::new();
    path.move_to(pt(r, 0.0));
    path.cubic_to(pt(r, k), pt(k, r), pt(0.0, r));
    path.cubic_to(pt(-k, r), pt(-r, k), pt(-r, 0.0));
    path.cubic_to(pt(-r, -k), pt(-k, -r), pt(0.0, -r));
    path.cubic_to(pt(k, -r), pt(r, -k), pt(r, 0.0));
    path.close();
    path
}

fn fill(path: Path, color: Rgba) -> Item {
    Item::Fill {
        path,
        paint: Paint::Solid(color),
        rule: FillRule::NonZero,
    }
}

fn stroke(path: Path, width: f32, dash: Vec<f32>, cap: LineCap, join: LineJoin) -> Item {
    Item::Stroke {
        path,
        style: StrokeStyle {
            color: rgb(20, 20, 20),
            width,
            dash,
            cap,
            join,
        },
    }
}

fn style(font: Font, weight: Weight, size: f32, tabular: bool) -> TextStyle {
    TextStyle {
        font,
        weight,
        size,
        tabular_figures: tabular,
    }
}

/// The scene behind the golden test, plus a quadratic and a text run per bundled font.
fn primitives_scene(lib: &FontLibrary) -> Scene {
    let mut scene = Scene::new(288.0, 216.0, rgb(255, 255, 255));
    scene.push(fill(
        polygon(&[(20.0, 90.0), (80.0, 90.0), (50.0, 30.0)]),
        rgb(31, 78, 121),
    ));
    scene.push(Item::Fill {
        path: rect_path(100.0, 30.0, 100.0, 60.0),
        paint: Paint::Linear(LinearGradient {
            start: pt(100.0, 30.0),
            end: pt(200.0, 90.0),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: rgb(230, 120, 30),
                },
                GradientStop {
                    offset: 0.5,
                    color: rgb(240, 220, 90),
                },
                GradientStop {
                    offset: 1.0,
                    color: rgb(30, 130, 120),
                },
            ],
        }),
        rule: FillRule::NonZero,
    });
    let mut wave = Path::new();
    wave.move_to(pt(215.0, 85.0));
    wave.cubic_to(pt(235.0, 20.0), pt(255.0, 100.0), pt(275.0, 35.0));
    wave.quad_to(pt(280.0, 80.0), pt(250.0, 90.0));
    scene.push(stroke(
        wave,
        3.0,
        vec![6.0, 5.0],
        LineCap::Round,
        LineJoin::Round,
    ));
    let mut next = lcg(12345);
    let positions = (0..50)
        .map(|_| {
            pt(
                20.0 + (next() * 130.0) as f32,
                110.0 + (next() * 50.0) as f32,
            )
        })
        .collect();
    scene.push(Item::Markers {
        shape: circle(4.0),
        fill: Some(rgb(40, 120, 190)),
        stroke: Some(StrokeStyle {
            color: rgb(255, 255, 255),
            width: 0.8,
            dash: vec![],
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }),
        positions,
    });
    scene.push(Item::Group {
        clip: Some(Rect {
            x: 170.0,
            y: 115.0,
            width: 60.0,
            height: 40.0,
        }),
        items: vec![fill(
            circle(25.0).translated(205.0, 135.0),
            rgb(110, 60, 160),
        )],
    });
    let black = rgb(20, 20, 20);
    let runs = [
        (
            "Revenue \u{2212}12.5%",
            Font::Inter,
            Weight::SemiBold,
            13.0,
            false,
        ),
        (
            "Q1 Q2 Q3 0123456789",
            Font::SourceSerif4,
            Weight::Regular,
            10.0,
            true,
        ),
        ("Geist AV To", Font::Geist, Weight::Regular, 11.0, false),
        (
            "Fallback \u{E000}",
            Font::Inter,
            Weight::Regular,
            11.0,
            false,
        ),
    ];
    for (i, (text, font, weight, size, tabular)) in runs.into_iter().enumerate() {
        scene.push_text(
            lib,
            text,
            style(font, weight, size, tabular),
            pt(20.0, 185.0 + 14.0 * i as f32),
            black,
        );
    }
    scene
}

fn strokes_scene() -> Scene {
    let mut scene = Scene::new(200.0, 150.0, rgb(255, 255, 255));
    let mut zigzag = Path::new();
    zigzag.move_to(pt(10.0, 30.0));
    zigzag.line_to(pt(40.0, 10.0));
    zigzag.line_to(pt(70.0, 30.0));
    zigzag.line_to(pt(100.0, 10.0));
    let mut row = 0.0;
    for cap in [LineCap::Butt, LineCap::Round, LineCap::Square] {
        for join in [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel] {
            for dash in [vec![], vec![8.0, 4.0], vec![3.0]] {
                scene.push(stroke(
                    zigzag.translated(if dash.is_empty() { 0.0 } else { 100.0 }, row),
                    4.0,
                    dash,
                    cap,
                    join,
                ));
            }
            row += 5.0;
        }
    }
    scene.push(stroke(
        circle(30.0).translated(150.0, 100.0),
        0.1,
        vec![],
        LineCap::Butt,
        LineJoin::Miter,
    ));
    scene.push(stroke(
        circle(20.0).translated(50.0, 110.0),
        12.0,
        vec![1.0, 1.0],
        LineCap::Round,
        LineJoin::Round,
    ));
    scene.push(Item::Stroke {
        path: rect_path(5.0, 5.0, 190.0, 140.0),
        style: StrokeStyle {
            color: Rgba {
                r: 200,
                g: 20,
                b: 20,
                a: 90,
            },
            width: 2.0,
            dash: vec![0.0, 5.0],
            cap: LineCap::Round,
            join: LineJoin::Miter,
        },
    });
    scene
}

fn gradients_scene() -> Scene {
    let mut scene = Scene::new(160.0, 120.0, rgb(240, 240, 240));
    let stop = |offset: f32, color: Rgba| GradientStop { offset, color };
    let grads = [
        (
            pt(0.0, 0.0),
            pt(80.0, 0.0),
            vec![stop(0.0, rgb(255, 0, 0)), stop(1.0, rgb(0, 0, 255))],
        ),
        (
            pt(0.0, 0.0),
            pt(0.0, 50.0),
            vec![
                stop(
                    0.0,
                    Rgba {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    },
                ),
                stop(0.3, rgb(0, 150, 0)),
                stop(0.3, rgb(255, 255, 0)),
                stop(
                    1.0,
                    Rgba {
                        r: 255,
                        g: 255,
                        b: 255,
                        a: 128,
                    },
                ),
            ],
        ),
        (
            pt(10.0, 10.0),
            pt(70.0, 40.0),
            vec![stop(0.2, rgb(10, 10, 10)), stop(0.8, rgb(200, 100, 0))],
        ),
        (
            pt(5.0, 5.0),
            pt(5.0, 5.0),
            vec![stop(0.0, rgb(1, 2, 3)), stop(1.0, rgb(4, 5, 6))],
        ),
        (
            pt(0.0, 0.0),
            pt(40.0, 0.0),
            vec![stop(0.5, rgb(90, 20, 140))],
        ),
    ];
    for (i, (start, end, stops)) in grads.into_iter().enumerate() {
        let (cx, cy) = (5.0 + 50.0 * (i % 3) as f32, 5.0 + 55.0 * (i / 3) as f32);
        scene.push(Item::Fill {
            path: rect_path(cx, cy, 45.0, 45.0),
            paint: Paint::Linear(LinearGradient {
                start: pt(start.x + cx, start.y + cy),
                end: pt(end.x + cx, end.y + cy),
                stops,
            }),
            rule: if i % 2 == 0 {
                FillRule::NonZero
            } else {
                FillRule::EvenOdd
            },
        });
    }
    // Self-overlapping star: fill rules differ.
    let star = polygon(&[
        (0.0, -20.0),
        (12.0, 16.0),
        (-19.0, -6.0),
        (19.0, -6.0),
        (-12.0, 16.0),
    ]);
    for (i, rule) in [FillRule::NonZero, FillRule::EvenOdd]
        .into_iter()
        .enumerate()
    {
        scene.push(Item::Fill {
            path: star.translated(115.0 + 35.0 * i as f32, 90.0),
            paint: Paint::Solid(rgb(150, 30, 90)),
            rule,
        });
    }
    scene
}

fn groups_scene() -> Scene {
    let mut scene = Scene::new(120.0, 90.0, rgb(255, 255, 255));
    let clip = |x, y, width, height| Rect {
        x,
        y,
        width,
        height,
    };
    scene.push(Item::Group {
        clip: Some(clip(10.0, 10.0, 50.0, 40.0)),
        items: vec![
            fill(circle(30.0).translated(35.0, 30.0), rgb(30, 120, 200)),
            Item::Group {
                clip: Some(clip(30.0, 20.0, 60.0, 60.0)),
                items: vec![fill(rect_path(0.0, 0.0, 120.0, 90.0), rgb(250, 160, 40))],
            },
        ],
    });
    scene.push(Item::Group {
        clip: Some(clip(70.5, 10.25, 30.0, 30.0)),
        items: vec![stroke(
            rect_path(60.0, 0.0, 60.0, 60.0),
            6.0,
            vec![],
            LineCap::Butt,
            LineJoin::Miter,
        )],
    });
    scene.push(Item::Group {
        clip: Some(clip(0.0, 0.0, 0.0, 10.0)),
        items: vec![fill(rect_path(0.0, 0.0, 120.0, 90.0), rgb(0, 0, 0))],
    });
    scene.push(Item::Group {
        clip: Some(clip(-50.0, -50.0, 1.0e7, 1.0e7)),
        items: vec![fill(rect_path(100.0, 70.0, 15.0, 15.0), rgb(10, 140, 60))],
    });
    // Nesting at and beyond the supported depth.
    for depth in [8, MAX_GROUP_DEPTH, MAX_GROUP_DEPTH + 4] {
        let mut item = fill(rect_path(2.0, 60.0, 20.0, 20.0), rgb(120, 0, 120));
        for _ in 0..depth {
            item = Item::Group {
                clip: None,
                items: vec![item],
            };
        }
        scene.push(item);
    }
    scene
}

fn markers_scene() -> Scene {
    let mut scene = Scene::new(150.0, 100.0, rgb(255, 255, 255));
    let shapes = [
        circle(5.0),
        rect_path(-4.0, -4.0, 8.0, 8.0),
        polygon(&[(0.0, -6.0), (5.0, 4.0), (-5.0, 4.0)]),
        polygon(&[(0.0, -6.0), (6.0, 0.0), (0.0, 6.0), (-6.0, 0.0)]),
        circle(0.4),
    ];
    for (i, shape) in shapes.into_iter().enumerate() {
        let positions = (0..6)
            .map(|k| {
                pt(
                    12.0 + 22.0 * k as f32 + 0.37 * i as f32,
                    12.0 + 18.0 * i as f32 + 0.21 * k as f32,
                )
            })
            .collect();
        scene.push(Item::Markers {
            shape,
            fill: if i % 2 == 0 {
                Some(rgb(40 * i as u8, 90, 200))
            } else {
                None
            },
            stroke: if i == 0 {
                None
            } else {
                Some(StrokeStyle {
                    color: rgb(200, 40, 40),
                    width: 1.5,
                    dash: vec![],
                    cap: LineCap::Butt,
                    join: LineJoin::Round,
                })
            },
            positions,
        });
    }
    scene.push(Item::Markers {
        shape: circle(3.0),
        fill: Some(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 60,
        }),
        stroke: None,
        positions: Vec::new(),
    });
    scene
}

/// Non-finite, huge and degenerate input; backends skip or fall back on it rather than fail.
fn hostile_scene() -> Scene {
    let nan = f32::NAN;
    let inf = f32::INFINITY;
    let black = rgb(0, 0, 0);
    let mut bad = Path::new();
    bad.move_to(pt(nan, 0.0));
    bad.line_to(pt(1.0, inf));
    bad.quad_to(pt(nan, nan), pt(2.0, 2.0));
    bad.cubic_to(pt(0.0, 0.0), pt(inf, 0.0), pt(3.0, 3.0));
    bad.line_to(pt(5.0, 5.0));
    bad.close();
    let mut huge = Path::new();
    huge.move_to(pt(-3.0e38, -3.0e38));
    huge.line_to(pt(3.0e38, 3.0e38));
    huge.line_to(pt(3.0e38, -3.0e38));
    let mut curve = Path::new();
    curve.move_to(pt(1.0, 1.0));
    curve.quad_to(pt(1.0e10, -1.0e10), pt(30.0, 10.0));
    curve.line_to(pt(20.0, 15.0));
    let mut broken = Path::new();
    broken.move_to(pt(2.0, 10.0));
    broken.line_to(pt(9.0, 10.0));
    broken.line_to(pt(nan, 10.0));
    broken.line_to(pt(30.0, 10.0));
    broken.line_to(pt(38.0, 10.0));
    let mut close_only = Path::new();
    close_only.move_to(pt(20.0, 3.0));
    close_only.close();
    let plain = |style_path: Path, width: f32, dash: Vec<f32>| {
        stroke(style_path, width, dash, LineCap::Butt, LineJoin::Miter)
    };
    let gradient = |stops: Vec<GradientStop>, end: Point| {
        Paint::Linear(LinearGradient {
            start: pt(0.0, 0.0),
            end,
            stops,
        })
    };
    let stop = |offset: f32| GradientStop {
        offset,
        color: black,
    };
    let square = || rect_path(0.0, 0.0, 10.0, 10.0);

    let mut scene = Scene::new(50.0, 50.0, rgb(255, 255, 255));
    scene.push(fill(bad.clone(), black));
    scene.push(fill(huge.clone(), black));
    scene.push(fill(Path::new(), black));
    scene.push(plain(bad.clone(), 1.0, vec![]));
    scene.push(plain(huge, 1.0, vec![2.0, 2.0]));
    scene.push(plain(curve, 1.0, vec![]));
    scene.push(plain(broken, 2.0, vec![]));
    scene.push(stroke(
        close_only,
        6.0,
        vec![],
        LineCap::Round,
        LineJoin::Miter,
    ));
    for width in [0.0, -1.0, nan, inf] {
        scene.push(plain(square(), width, vec![]));
    }
    for dash in [
        vec![0.0, 0.0],
        vec![-1.0, 2.0],
        vec![nan],
        vec![inf, 1.0],
        vec![3.0],
        vec![1.0e-30, 1.0e-30],
        vec![1.0e30],
    ] {
        scene.push(plain(square(), 1.0, dash));
    }
    for paint in [
        gradient(vec![], pt(10.0, 0.0)),
        gradient(vec![stop(0.5)], pt(10.0, 0.0)),
        gradient(vec![stop(0.0), stop(1.0)], pt(0.0, 0.0)),
        gradient(vec![stop(nan), stop(5.0), stop(-2.0)], pt(nan, 0.0)),
        gradient(vec![stop(nan), stop(5.0), stop(-2.0)], pt(10.0, 0.0)),
        gradient(vec![stop(0.0), stop(1.0)], pt(inf, 1.0)),
        gradient(vec![stop(1.0), stop(0.0)], pt(1.0e30, 0.0)),
    ] {
        scene.push(Item::Fill {
            path: square(),
            paint,
            rule: FillRule::EvenOdd,
        });
    }
    for clip in [
        Rect {
            x: 0.0,
            y: 0.0,
            width: -1.0,
            height: 10.0,
        },
        Rect {
            x: nan,
            y: 0.0,
            width: 10.0,
            height: inf,
        },
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0e30,
            height: 1.0e30,
        },
        Rect {
            x: -1.0e30,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    ] {
        scene.push(Item::Group {
            clip: Some(clip),
            items: vec![fill(square(), black)],
        });
    }
    scene.push(Item::Group {
        clip: None,
        items: vec![],
    });
    scene.push(Item::Markers {
        shape: square(),
        fill: Some(black),
        stroke: Some(StrokeStyle {
            color: black,
            width: nan,
            dash: vec![],
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }),
        positions: vec![
            pt(nan, 1.0),
            pt(1.0, inf),
            pt(20.0, 20.0),
            pt(1.0e30, 1.0e30),
        ],
    });
    scene.push(Item::Markers {
        shape: bad,
        fill: Some(black),
        stroke: None,
        positions: vec![pt(5.0, 5.0)],
    });
    scene
}

/// Text records with non-finite numbers, built by hand so no layout is involved.
fn hostile_text_scene(lib: &FontLibrary) -> Scene {
    use umber_core::scene::TextItem;
    let mut scene = Scene::new(120.0, 60.0, rgb(255, 255, 255));
    let mut outline = Path::new();
    outline.move_to(pt(10.0, 10.0));
    outline.line_to(pt(30.0, 10.0));
    outline.line_to(pt(20.0, 25.0));
    outline.close();
    let base = TextItem {
        text: "hand <built> & \"quoted\"".into(),
        style: style(Font::Inter, Weight::Regular, 12.0, false),
        origin: pt(10.0, 25.0),
        color: rgb(30, 30, 30),
        outline: outline.clone(),
        width: 40.0,
    };
    let nan = f32::NAN;
    scene.push(Item::Text(base.clone()));
    for variant in [
        TextItem {
            width: nan,
            ..base.clone()
        },
        TextItem {
            origin: pt(nan, 5.0),
            ..base.clone()
        },
        TextItem {
            origin: pt(1.0e30, 5.0),
            ..base.clone()
        },
        TextItem {
            style: style(Font::Geist, Weight::SemiBold, nan, true),
            ..base.clone()
        },
        TextItem {
            style: style(Font::SourceSerif4, Weight::SemiBold, -3.0, false),
            ..base.clone()
        },
        TextItem {
            text: String::new(),
            ..base.clone()
        },
        TextItem {
            text: "control\u{1}\u{7f}\ttabs \u{E000} \u{10ffff}".into(),
            ..base.clone()
        },
        TextItem {
            outline: Path::new(),
            ..base.clone()
        },
    ] {
        scene.push(Item::Text(variant));
    }
    scene.push_text(
        lib,
        "",
        style(Font::Inter, Weight::Regular, 10.0, false),
        pt(0.0, 0.0),
        rgb(0, 0, 0),
    );
    scene.push_text(
        lib,
        "pushed text",
        style(Font::Inter, Weight::Regular, f32::INFINITY, false),
        pt(0.0, 40.0),
        rgb(0, 0, 0),
    );
    scene.push_text(
        lib,
        "pushed text \u{E000}",
        style(Font::Geist, Weight::Regular, 9.0, false),
        pt(f32::NAN, 40.0),
        rgb(0, 0, 0),
    );
    scene
}

/// Figures that exercise header and axis label combinations, font scales and long labels.
fn label_cases(lib: &FontLibrary, add: &mut impl FnMut(&str, Scene)) {
    type Labeller = fn(&mut umber_core::AxesMut<'_>);
    let only: [(&str, Labeller); 3] = [
        ("fig-xlabel-only", |a| {
            a.xlabel("Horizontal only");
        }),
        ("fig-ylabel-only", |a| {
            a.ylabel("Vertical only");
        }),
        ("fig-subtitle-only", |a| {
            a.subtitle("Subtitle only");
        }),
    ];
    for (name, label) in only {
        let mut f = figure(5.0, 3.5, Theme::pigment(), linear(), linear());
        label(&mut axes(&mut f));
        let (x, y) = wobble(20, 17);
        axes(&mut f).line(numbers(x), &y, line()).unwrap();
        add(name, to_scene(&f, lib));
    }

    // At the largest font scale the header, the tick labels and the axis labels take about
    // 10 in, so that figure is 12 in tall.
    for (name, scale, height) in [
        ("fig-font-scale-max", 10.0, 12.0),
        ("fig-font-scale-tiny", 1.0e-30, 4.0),
    ] {
        add(name, to_scene(&font_scale_figure(scale, height), lib));
    }
}

/// A 6 in wide graphite figure `height` in tall at font `scale`, on a linear x and a log y
/// axis, with a title, a subtitle, axis labels and one line.
fn font_scale_figure(scale: f64, height: f64) -> Figure {
    let theme = Theme::graphite().with_font_scale(scale).unwrap();
    let mut f = figure(6.0, height, theme, linear(), log(10.0));
    labelled(&mut f, "Font scale");
    axes(&mut f)
        .line(
            numbers(vec![0.0, 1.0, 2.0, 3.0]),
            &[1.0, 30.0, 5.0, 900.0],
            line(),
        )
        .unwrap();
    f
}

/// Text in every bundled font and weight, with and without tabular figures.
fn font_weights_scene(lib: &FontLibrary) -> Scene {
    let mut scene = Scene::new(420.0, 330.0, rgb(255, 255, 255));
    let mut row = 0.0;
    for font in Font::ALL {
        for weight in Weight::ALL {
            for tabular in [false, true] {
                scene.push_text(
                    lib,
                    "Revenue \u{2212}12.5% 0123456789 AV To fi \u{E000}",
                    style(font, weight, 11.0, tabular),
                    pt(10.0, 16.0 + 15.0 * row),
                    rgb(20, 20, 20),
                );
                row += 1.0;
            }
        }
    }
    scene
}

/// Text form of public values, independent of the `Debug` implementations of umber types.
///
/// Floats are written as the hexadecimal bit pattern of their own width, so every NaN payload
/// and the sign of zero are kept. Strings are quoted with every character outside printable
/// ASCII, and the quote and backslash, escaped as `\u{..}`.
mod record {
    use std::fmt::Write as _;

    use umber_core::figure::{AxisData, Mark};
    use umber_core::scene::{
        FillRule, Item, LineCap, LineJoin, Paint, Rect, Scene, StrokeStyle, TextItem,
    };
    use umber_core::{
        Alpha, Axes, ColorSpec, Dash, Error, Figure, FillStyle, Font, GridLines, LegendStyle,
        Marker, Path, PathVerb, Point, Rgba, ScaleKind, TextStyle, Theme, Transform, Weight,
    };

    pub fn f32(v: f32) -> String {
        format!("{:08x}", v.to_bits())
    }

    pub fn f64(v: f64) -> String {
        format!("{:016x}", v.to_bits())
    }

    pub fn floats32(values: &[f32]) -> String {
        let parts: Vec<String> = values.iter().map(|v| f32(*v)).collect();
        format!("[{}]", parts.join(","))
    }

    pub fn floats64(values: &[f64]) -> String {
        let parts: Vec<String> = values.iter().map(|v| f64(*v)).collect();
        format!("[{}]", parts.join(","))
    }

    pub fn text(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            if matches!(c, ' '..='~') && c != '"' && c != '\\' {
                out.push(c);
            } else {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
        }
        out.push('"');
        out
    }

    pub fn rgba(c: Rgba) -> String {
        format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a)
    }

    pub fn point(p: Point) -> String {
        format!("{} {}", f32(p.x), f32(p.y))
    }

    pub fn rect(r: Rect) -> String {
        format!(
            "rect({} {} {} {})",
            f32(r.x),
            f32(r.y),
            f32(r.width),
            f32(r.height)
        )
    }

    pub fn font(f: Font) -> &'static str {
        match f {
            Font::SourceSerif4 => "source-serif-4",
            Font::IbmPlexSans => "ibm-plex-sans",
            Font::Geist => "geist",
            Font::Inter => "inter",
            Font::JetBrainsMono => "jetbrains-mono",
        }
    }

    pub fn weight(w: Weight) -> &'static str {
        match w {
            Weight::Regular => "regular",
            Weight::SemiBold => "semibold",
        }
    }

    pub fn dash(d: Dash) -> &'static str {
        match d {
            Dash::Solid => "solid",
            Dash::Dashed => "dashed",
            Dash::Dotted => "dotted",
        }
    }

    /// The marker's name as written in labels.
    pub fn marker(m: Marker) -> &'static str {
        match m {
            Marker::Circle => "Circle",
            Marker::Square => "Square",
            Marker::Triangle => "Triangle",
            Marker::Diamond => "Diamond",
            Marker::Cross => "Cross",
        }
    }

    pub fn grid_lines(g: GridLines) -> &'static str {
        match g {
            GridLines::Horizontal => "horizontal",
            GridLines::Vertical => "vertical",
            GridLines::Both => "both",
            GridLines::None => "none",
        }
    }

    pub fn legend(l: LegendStyle) -> &'static str {
        match l {
            LegendStyle::Header => "header",
            LegendStyle::Direct => "direct",
            LegendStyle::Chips => "chips",
            LegendStyle::None => "none",
        }
    }

    pub fn fill_style(f: FillStyle) -> &'static str {
        match f {
            FillStyle::Solid => "solid",
            FillStyle::Gradient => "gradient",
        }
    }

    pub fn alpha(a: Alpha) -> String {
        format!("alpha({} u8={})", f32(a.get()), a.to_u8())
    }

    pub fn color_spec(c: ColorSpec) -> String {
        match c {
            ColorSpec::Auto(a) => format!("auto({})", alpha(a)),
            ColorSpec::Palette { index, alpha: a } => format!("palette({index} {})", alpha(a)),
            ColorSpec::Fixed(c) => format!("fixed({})", rgba(c)),
        }
    }

    pub fn scale_kind(k: ScaleKind) -> String {
        match k {
            ScaleKind::Linear => "linear".into(),
            ScaleKind::Log { base } => format!("log({})", f64(base.get())),
            ScaleKind::Category => "category".into(),
            _ => "unknown-scale".into(),
        }
    }

    pub fn transform(t: Transform) -> String {
        match t {
            Transform::Linear => "linear".into(),
            Transform::Log(base) => format!("log({})", f64(base.get())),
        }
    }

    pub fn text_style(s: &TextStyle) -> String {
        format!(
            "font={} weight={} size={} tabular={}",
            font(s.font),
            weight(s.weight),
            f32(s.size),
            s.tabular_figures
        )
    }

    pub fn error(e: &Error) -> String {
        let fields = |category: &str, context: &str, message: &str| {
            format!(
                "error category={category} context={} message={}",
                text(context),
                text(message)
            )
        };
        let head = match e {
            Error::Data {
                context, message, ..
            } => fields("data", context, message),
            Error::Style {
                context, message, ..
            } => fields("style", context, message),
            Error::Usage {
                context, message, ..
            } => fields("usage", context, message),
            Error::Io(io) => format!("error category=io message={}", text(&io.to_string())),
            _ => "error category=unknown".into(),
        };
        format!(
            "{head} display={} parameter={} hint={}",
            text(&e.to_string()),
            e.parameter().unwrap_or("-"),
            e.hint().map_or_else(|| "-".to_owned(), text)
        )
    }

    pub fn theme(t: &Theme) -> String {
        let colors: Vec<String> = t.palette().iter().map(|c| rgba(*c)).collect();
        let rows = [
            (
                "palette",
                format!("{} [{}]", colors.len(), colors.join(",")),
            ),
            ("background", rgba(t.background())),
            ("text_color", rgba(t.text_color())),
            ("muted_color", rgba(t.muted_color())),
            ("grid_color", rgba(t.grid_color())),
            ("grid_lines", grid_lines(t.grid_lines()).into()),
            ("grid_dash", dash(t.grid_dash()).into()),
            ("grid_width", f32(t.grid_width())),
            ("baseline_color", rgba(t.baseline_color())),
            ("baseline_width", f32(t.baseline_width())),
            ("title_font", font(t.title_font()).into()),
            ("body_font", font(t.body_font()).into()),
            ("title_size", f32(t.title_size())),
            ("body_size", f32(t.body_size())),
            ("font_scale", f32(t.font_scale())),
            ("legend_style", legend(t.legend_style()).into()),
            ("fill_style", fill_style(t.fill_style()).into()),
            ("line_width", f32(t.line_width())),
            ("marker_size", f32(t.marker_size())),
            ("marker_alpha", f32(t.marker_alpha())),
            ("bar_corner_radius", f32(t.bar_corner_radius())),
            ("hist_gap", f32(t.hist_gap())),
        ];
        let mut out = String::new();
        for (name, value) in rows {
            let _ = writeln!(out, "  {name} {value}");
        }
        out
    }

    fn axis_data(d: &AxisData) -> String {
        match d {
            AxisData::Numbers(v) => format!("numbers{}", floats64(v)),
            AxisData::Categories(v) => {
                let names: Vec<String> = v.iter().map(|s| text(s)).collect();
                format!("categories[{}]", names.join(","))
            }
        }
    }

    /// The y values of a series: numbers as a plain float list, the record's format for y,
    /// and names as [`axis_data`] writes them.
    fn y_data(d: &AxisData) -> String {
        match d {
            AxisData::Numbers(v) => floats64(v),
            AxisData::Categories(_) => axis_data(d),
        }
    }

    fn mark(m: &Mark) -> String {
        match m {
            Mark::Line(line) => format!(
                "line x={} y={} label={} color={} dash={}",
                axis_data(line.x()),
                y_data(line.y()),
                text(&line.options().label),
                color_spec(line.options().color),
                dash(line.options().dash)
            ),
            Mark::Scatter(scatter) => format!(
                "scatter x={} y={} label={} color={} marker={}",
                axis_data(scatter.x()),
                y_data(scatter.y()),
                text(&scatter.options().label),
                color_spec(scatter.options().color),
                marker(scatter.options().marker)
            ),
            Mark::Area(area) => format!(
                "area x={} y={} label={} color={} dash={}",
                axis_data(area.x()),
                y_data(area.y()),
                text(&area.options().label),
                color_spec(area.options().color),
                dash(area.options().dash)
            ),
            Mark::Bar(bar) => {
                let names: Vec<String> = bar.categories().iter().map(|s| text(s)).collect();
                format!(
                    "bar categories=[{}] heights={} label={} color={}",
                    names.join(","),
                    floats64(bar.heights()),
                    text(&bar.options().label),
                    color_spec(bar.options().color)
                )
            }
            Mark::GroupedBar(group) => bar_group("grouped-bar", group),
            Mark::StackedBar(group) => bar_group("stacked-bar", group),
            Mark::Hist(hist) => format!(
                "hist values={} bins={} label={} color={}",
                floats64(hist.values()),
                hist.options().bins,
                text(&hist.options().label),
                color_spec(hist.options().color)
            ),
            Mark::Step(step) => format!(
                "step x={} y={} position={} label={} color={} dash={}",
                axis_data(step.x()),
                y_data(step.y()),
                step_position(step.options().position),
                text(&step.options().label),
                color_spec(step.options().color),
                dash(step.options().dash)
            ),
            Mark::HLine(rule) => format!(
                "hline y={} label={} color={} dash={}",
                floats64(&[rule.y()]),
                text(&rule.options().label),
                color_spec(rule.options().color),
                dash(rule.options().dash)
            ),
            Mark::VLine(rule) => format!(
                "vline x={} label={} color={} dash={}",
                match rule.x() {
                    umber_core::AxisValue::Number(x) => floats64(&[*x]),
                    umber_core::AxisValue::Category(name) => text(name),
                },
                text(&rule.options().label),
                color_spec(rule.options().color),
                dash(rule.options().dash)
            ),
            Mark::HSpan(span) => format!(
                "hspan low={} high={} label={} color={}",
                floats64(&[span.low()]),
                floats64(&[span.high()]),
                text(&span.options().label),
                color_spec(span.options().color)
            ),
            Mark::VSpan(span) => format!(
                "vspan low={} high={} label={} color={}",
                floats64(&[span.low()]),
                floats64(&[span.high()]),
                text(&span.options().label),
                color_spec(span.options().color)
            ),
            Mark::StackedArea(mark) => format!(
                "stacked-area x={} series=[{}]",
                floats64(mark.x()),
                mark.series()
                    .map(|s| format!(
                        "label={} values={} color={}",
                        text(s.label()),
                        floats64(s.values()),
                        color_spec(s.color())
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Mark::Band(band) => format!(
                "band x={} low={} high={} label={} color={}",
                floats64(band.x()),
                floats64(band.low()),
                floats64(band.high()),
                text(&band.options().label),
                color_spec(band.options().color)
            ),
            Mark::ErrorBars(bars) => format!(
                "error-bars orientation={} positions={} low={} high={} label={} color={} width={:?} caps={}",
                match bars.orientation() {
                    umber_core::ErrorBarOrientation::Vertical => "vertical",
                    umber_core::ErrorBarOrientation::Horizontal => "horizontal",
                },
                axis_data(bars.positions()),
                floats64(bars.low()),
                floats64(bars.high()),
                text(&bars.options().label),
                color_spec(bars.options().color),
                bars.options().width,
                bars.options().caps
            ),
            Mark::Bubbles(mark) => format!(
                "bubbles x={} y={} sizes={} label={} color={} marker={}",
                floats64(mark.x()),
                floats64(mark.y()),
                floats64(mark.sizes()),
                text(&mark.options().label),
                color_spec(mark.options().color),
                marker(mark.options().marker)
            ),
            Mark::Stem(stem) => format!(
                "stem x={} y={} label={} color={} marker={} size={:?}",
                axis_data(stem.x()),
                y_data(stem.y()),
                text(&stem.options().label),
                color_spec(stem.options().color),
                marker(stem.options().marker),
                stem.options().size
            ),
            Mark::Box(mark) => format!(
                "box categories={:?} groups=[{}] label={} color={}",
                mark.categories(),
                mark.groups()
                    .iter()
                    .map(|g| floats64(g))
                    .collect::<Vec<_>>()
                    .join("; "),
                text(&mark.options().label),
                color_spec(mark.options().color)
            ),
            Mark::GroupedBox(mark) => format!(
                "grouped-box categories={:?} distributions=[{}]",
                mark.categories(),
                mark.distributions()
                    .map(|d| format!(
                        "label={} groups=[{}] color={}",
                        text(d.label()),
                        d.groups()
                            .iter()
                            .map(|g| floats64(g))
                            .collect::<Vec<_>>()
                            .join("; "),
                        color_spec(d.color())
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Mark::Violin(mark) => format!(
                "violin categories={:?} groups=[{}] label={} color={}",
                mark.categories(),
                mark.groups()
                    .iter()
                    .map(|g| floats64(g))
                    .collect::<Vec<_>>()
                    .join("; "),
                text(&mark.options().label),
                color_spec(mark.options().color)
            ),
            Mark::GroupedViolin(mark) => format!(
                "grouped-violin categories={:?} distributions=[{}]",
                mark.categories(),
                mark.distributions()
                    .map(|d| format!(
                        "label={} groups=[{}] color={}",
                        text(d.label()),
                        d.groups()
                            .iter()
                            .map(|g| floats64(g))
                            .collect::<Vec<_>>()
                            .join("; "),
                        color_spec(d.color())
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            _ => "unknown-mark".into(),
        }
    }

    fn step_position(position: umber_core::Step) -> &'static str {
        match position {
            umber_core::Step::Post => "post",
            umber_core::Step::Pre => "pre",
            umber_core::Step::Mid => "mid",
        }
    }

    fn bar_group(kind: &str, group: &umber_core::BarGroupMark) -> String {
        let names: Vec<String> = group.categories().iter().map(|s| text(s)).collect();
        let series: Vec<String> = group
            .series()
            .map(|s| {
                format!(
                    "label={} heights={} color={}",
                    text(s.label()),
                    floats64(s.values()),
                    color_spec(s.color())
                )
            })
            .collect();
        format!(
            "{kind} categories=[{}] series=[{}]",
            names.join(","),
            series.join("; ")
        )
    }

    fn limits(l: Option<(f64, f64)>) -> String {
        l.map_or_else(
            || "none".into(),
            |(lo, hi)| format!("{} {}", f64(lo), f64(hi)),
        )
    }

    fn axes(a: &Axes) -> String {
        let mut out = format!(
            "  axes x={} y={} title={} subtitle={} xlabel={} ylabel={} xlim={} ylim={} marks={}\n",
            scale_kind(a.x_scale()),
            scale_kind(a.y_scale()),
            text(a.title_text()),
            text(a.subtitle_text()),
            text(a.xlabel_text()),
            text(a.ylabel_text()),
            limits(a.x_limits()),
            limits(a.y_limits()),
            a.marks().len()
        );
        for m in a.marks() {
            let _ = writeln!(out, "    {}", mark(m));
        }
        out
    }

    /// The observable state of a figure: its size, its theme's palette in order and its axes.
    pub fn figure(f: &Figure) -> String {
        let palette: Vec<String> = f.theme().palette().iter().map(|c| rgba(*c)).collect();
        let mut out = format!(
            "  figure width_in={} height_in={} palette=[{}]\n",
            f64(f.width_in()),
            f64(f.height_in()),
            palette.join(",")
        );
        let grid = f.grid();
        // A figure of one cell is recorded as it was before figures had grids.
        if grid.rows() * grid.cols() > 1 {
            let _ = writeln!(
                out,
                "  grid rows={} cols={} panels={}",
                grid.rows(),
                grid.cols(),
                f.panels().count()
            );
        }
        let mut panels = f.panels().peekable();
        if panels.peek().is_none() {
            out.push_str("  axes none\n");
        }
        for a in panels {
            if grid.rows() * grid.cols() > 1 {
                let c = a.cells();
                let _ = writeln!(
                    out,
                    "  cells row={} col={} row_span={} col_span={}",
                    c.row(),
                    c.col(),
                    c.row_span(),
                    c.col_span()
                );
            }
            out.push_str(&axes(a));
        }
        out
    }

    /// A line-oriented record of a scene. Each line is indented by its group depth.
    pub fn scene(s: &Scene) -> String {
        let mut w = Writer::default();
        w.line(&format!(
            "scene width={} height={} background={} items={}",
            f32(s.width),
            f32(s.height),
            rgba(s.background),
            s.items.len()
        ));
        w.items(&s.items);
        w.out
    }

    #[derive(Default)]
    struct Writer {
        out: String,
        depth: usize,
    }

    impl Writer {
        fn line(&mut self, s: &str) {
            for _ in 0..self.depth {
                self.out.push(' ');
            }
            self.out.push_str(s);
            self.out.push('\n');
        }

        fn items(&mut self, items: &[Item]) {
            self.depth += 1;
            for item in items {
                self.item(item);
            }
            self.depth -= 1;
        }

        fn item(&mut self, item: &Item) {
            match item {
                Item::Fill { path, paint, rule } => {
                    let rule = match rule {
                        FillRule::NonZero => "nonzero",
                        FillRule::EvenOdd => "evenodd",
                    };
                    self.line(&format!("fill rule={rule}"));
                    self.paint(paint);
                    self.path(path);
                }
                Item::Stroke { path, style } => {
                    self.line(&format!("stroke {}", stroke_style(style)));
                    self.path(path);
                }
                Item::Markers {
                    shape,
                    fill,
                    stroke,
                    positions,
                } => {
                    let fill = fill.map_or_else(|| "none".into(), rgba);
                    let stroke = stroke
                        .as_ref()
                        .map_or_else(|| "none".into(), |s| format!("({})", stroke_style(s)));
                    self.line(&format!(
                        "markers fill={fill} stroke={stroke} positions={}",
                        positions.len()
                    ));
                    for p in positions {
                        self.line(&format!(" at {}", point(*p)));
                    }
                    self.path(shape);
                }
                Item::Text(t) => self.text_item(t),
                Item::Group { clip, items } => {
                    let clip = clip.map_or_else(|| "none".into(), rect);
                    self.line(&format!("group clip={clip} items={}", items.len()));
                    self.items(items);
                    self.line("end group");
                }
            }
        }

        fn text_item(&mut self, t: &TextItem) {
            self.line(&format!(
                "text {} {} origin={} color={} width={}",
                text(&t.text),
                text_style(&t.style),
                point(t.origin),
                rgba(t.color),
                f32(t.width)
            ));
            self.path(&t.outline);
        }

        fn paint(&mut self, paint: &Paint) {
            match paint {
                Paint::Solid(c) => self.line(&format!(" paint solid {}", rgba(*c))),
                Paint::Linear(g) => {
                    self.line(&format!(
                        " paint linear start={} end={} stops={}",
                        point(g.start),
                        point(g.end),
                        g.stops.len()
                    ));
                    for s in &g.stops {
                        self.line(&format!("  stop {} {}", f32(s.offset), rgba(s.color)));
                    }
                }
            }
        }

        fn path(&mut self, path: &Path) {
            self.line(&format!(" path verbs={}", path.verbs().len()));
            for verb in path.verbs() {
                let s = match *verb {
                    PathVerb::MoveTo(p) => format!("  M {}", point(p)),
                    PathVerb::LineTo(p) => format!("  L {}", point(p)),
                    PathVerb::QuadTo(c, p) => format!("  Q {} {}", point(c), point(p)),
                    PathVerb::CubicTo(c1, c2, p) => {
                        format!("  C {} {} {}", point(c1), point(c2), point(p))
                    }
                    PathVerb::Close => "  Z".into(),
                };
                self.line(&s);
            }
        }
    }

    fn stroke_style(s: &StrokeStyle) -> String {
        let cap = match s.cap {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        };
        let join = match s.join {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        };
        format!(
            "color={} width={} dash={} cap={cap} join={join}",
            rgba(s.color),
            f32(s.width),
            floats32(&s.dash)
        )
    }
}

/// Collects the results of public API calls, one per line.
#[derive(Default)]
struct Log {
    out: String,
}

impl Log {
    fn section(&mut self, title: &str) {
        let _ = writeln!(self.out, "## {title}");
    }

    fn result<T>(
        &mut self,
        label: &str,
        result: umber_core::Result<T>,
        ok: impl FnOnce(T) -> String,
    ) {
        let value = match result {
            Ok(v) => {
                let value = ok(v);
                if value.is_empty() || value.starts_with('\n') {
                    format!("ok{value}")
                } else {
                    format!("ok {value}")
                }
            }
            Err(e) => record::error(&e),
        };
        let _ = writeln!(self.out, "{label}: {value}");
    }

    fn raw(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// Runs `op` on the figure's axes, then records its result and the figure's state.
    fn on_axes(
        &mut self,
        figure: &mut Figure,
        label: &str,
        op: impl FnOnce(&mut umber_core::AxesMut<'_>) -> umber_core::Result<()>,
    ) {
        let result = op(&mut figure
            .axes_at_mut(GridIndex::Auto, GridIndex::Auto)
            .unwrap());
        self.result(label, result, |()| String::new());
        self.raw(&record::figure(figure));
    }

    /// Creates a figure with axes, recording both calls.
    fn new_figure(&mut self, label: &str, x: ScaleKind, y: ScaleKind) -> Figure {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let result = f.add_axes(x, y).map(|_| ());
        self.result(label, result, |()| String::new());
        self.raw(&record::figure(&f));
        f
    }
}

/// Values for the numeric setters: below, at and above each bound, values that change when
/// narrowed to `f32`, and non-finite values.
const SETTER_VALUES: [f64; 17] = [
    -1.0,
    -0.0,
    0.0,
    1.0e-50,
    1.0e-30,
    0.5,
    1.0,
    1.000_000_1,
    10.0,
    10.000_001,
    1.0e9,
    3.4e38,
    1.0e39,
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    -1.0e-9,
];

/// Errors, return values and state of public API calls. A refactor must not change a category,
/// a context, a message, which fault an input with several is reported for, or what a failed
/// call leaves behind.
fn error_cases(lib: &FontLibrary) -> String {
    let mut rec = Log::default();
    figure_errors(&mut rec);
    series_errors(&mut rec);
    color_errors(&mut rec);
    scale_errors(&mut rec);
    theme_errors(&mut rec);
    png_errors(&mut rec, lib);
    rec.out
}

fn figure_errors(rec: &mut Log) {
    rec.section("Figure::new");
    for (w, h) in [
        (0.0, 1.0),
        (1.0, -1.0),
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
        (1.0e9, 1.0),
        (200.0, 200.0),
        (200.000_000_1, 1.0),
        (1.0, 200.000_000_1),
        (-1.0, f64::NAN),
        (f64::MIN_POSITIVE, 1.0),
        (-0.0, 1.0),
        (f64::NEG_INFINITY, f64::NEG_INFINITY),
    ] {
        rec.result(
            &format!("Figure::new({}, {})", num(w), num(h)),
            Figure::new(w, h, Theme::pigment()),
            |f| {
                format!(
                    "\n{}{}",
                    record::figure(&f),
                    record::theme(f.theme()).trim_end()
                )
            },
        );
    }

    rec.section("Figure::add_axes");
    for (name, x, y) in [
        ("linear/linear", linear(), linear()),
        (
            "category/category",
            ScaleKind::Category,
            ScaleKind::Category,
        ),
        ("log/category", log(2.0), ScaleKind::Category),
    ] {
        let mut f = rec.new_figure(&format!("add_axes {name}"), x, y);
        let result = f.add_axes(log(10.0), log(10.0)).map(|_| ());
        rec.result(&format!("add_axes {name} then log/log"), result, |()| {
            String::new()
        });
        rec.raw(&record::figure(&f));
    }

    rec.section("Axes limits, linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "xlim(0, 10)", |a| a.xlim(0.0, 10.0).map(drop));
    rec.on_axes(&mut f, "ylim(-1, 1)", |a| a.ylim(-1.0, 1.0).map(drop));
    for (lo, hi) in [
        (1.0, 1.0),
        (2.0, 1.0),
        (f64::NAN, 1.0),
        (1.0, f64::NAN),
        (0.0, f64::INFINITY),
        (f64::NEG_INFINITY, 0.0),
        (1.0, 1.0 + 1.0e-12),
        (0.0, 1.0e-291),
        (-1.0e301, 1.0),
        (1.0, 1.0e301),
        (1.0e301, 1.0e302),
        (f64::NAN, f64::NEG_INFINITY),
        (5.0, -1.0e301),
        (-1.0e300, 1.0e300),
        (1.0, 1.0 + 1.0e-9),
    ] {
        rec.on_axes(&mut f, &format!("xlim({}, {})", num(lo), num(hi)), |a| {
            a.xlim(lo, hi).map(drop)
        });
        rec.on_axes(&mut f, &format!("ylim({}, {})", num(lo), num(hi)), |a| {
            a.ylim(lo, hi).map(drop)
        });
    }

    rec.section("Axes limits, log");
    let mut f = rec.new_figure("log10/log2", log(10.0), log(2.0));
    for (lo, hi) in [
        (0.0, 10.0),
        (-5.0, 10.0),
        (1.0e-301, 10.0),
        (1.0, 1.0e301),
        (1.0, 1.000_000_000_1),
        (1.0, 1.000_000_01),
        (1.0e-300, 1.0e300),
        (10.0, 1.0),
        (0.0, f64::NAN),
        (-1.0, 1.0e301),
    ] {
        rec.on_axes(&mut f, &format!("xlim({}, {})", num(lo), num(hi)), |a| {
            a.xlim(lo, hi).map(drop)
        });
        rec.on_axes(&mut f, &format!("ylim({}, {})", num(lo), num(hi)), |a| {
            a.ylim(lo, hi).map(drop)
        });
    }

    rec.section("Axes limits, category");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    rec.on_axes(&mut f, "xlim(1, 2)", |a| a.xlim(1.0, 2.0).map(drop));
    rec.on_axes(&mut f, "xlim(NaN, NaN)", |a| {
        a.xlim(f64::NAN, f64::NAN).map(drop)
    });

    rec.section("Axes labels");
    rec.on_axes(&mut f, "labels", |a| {
        a.title("T \u{E000}")
            .subtitle("S")
            .xlabel("X \"q\"")
            .ylabel("Y\\");
        Ok(())
    });
    rec.on_axes(&mut f, "labels cleared", |a| {
        a.title("").subtitle("").xlabel("").ylabel("");
        Ok(())
    });

    figure_clone(rec);
}

/// A clone is equal to its source and independent of it.
fn figure_clone(rec: &mut Log) {
    rec.section("Figure::clone");
    let theme = Theme::midnight()
        .with_palette(&[rgb(9, 8, 7), rgb(6, 5, 4), rgb(3, 2, 1)])
        .unwrap()
        .with_hist_gap(0.75)
        .unwrap();
    let mut source = Figure::new(5.0, 3.0, theme).unwrap();
    source
        .add_axes(linear(), log(10.0))
        .unwrap()
        .title("Source")
        .subtitle("Sub")
        .xlabel("X")
        .ylabel("Y")
        .xlim(-2.0, 8.0)
        .unwrap()
        .ylim(0.5, 500.0)
        .unwrap()
        .line(
            numbers(vec![0.0, f64::from_bits(0x7ff8_0000_0000_0003), 2.0]),
            &[1.0, 10.0, f64::from_bits(0xfff8_0000_0000_0009)],
            LineOptions {
                label: "first".into(),
                color: ColorSpec::Palette {
                    index: 2,
                    alpha: Alpha::new(0.5).unwrap(),
                },
                dash: Dash::Dashed,
                ..LineOptions::default()
            },
        )
        .unwrap()
        .scatter(numbers(vec![1.0, 3.0]), &[2.0, 200.0], scatter())
        .unwrap();
    let mut copy = source.clone();
    let both = |rec: &mut Log, when: &str, source: &Figure, copy: &Figure| {
        for (name, f) in [("source", source), ("clone", copy)] {
            rec.raw(&format!("{when}, {name}\n"));
            rec.raw(&record::figure(f));
            rec.raw(&record::theme(f.theme()));
        }
    };
    both(rec, "after clone", &source, &copy);
    // A clone without axes is recorded, not a panic, so the gate reports the difference.
    if let Ok(mut a) = copy.axes_at_mut(GridIndex::Auto, GridIndex::Auto) {
        a.title("Changed clone").xlim(1.0, 2.0).unwrap();
        a.line(numbers(vec![5.0]), &[50.0], line()).unwrap();
    } else {
        rec.raw("the clone has no axes to change\n");
    }
    both(rec, "after changing the clone", &source, &copy);
}

fn series_errors(rec: &mut Log) {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let nums = |v: &[f64]| AxisData::Numbers(v.to_vec());
    let cats = |v: &[&str]| AxisData::Categories(v.iter().map(|s| (*s).to_owned()).collect());

    rec.section("Axes.line and Axes.scatter, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "line ok", |a| {
        a.line(nums(&[0.0, 1.0]), &[1.0, 2.0], line()).map(drop)
    });
    let cases: Vec<(&str, AxisData, Vec<f64>)> = vec![
        ("length mismatch", nums(&[0.0]), vec![1.0, 2.0]),
        (
            "length mismatch and infinite y",
            nums(&[inf]),
            vec![inf, 2.0],
        ),
        ("empty x, one y", nums(&[]), vec![1.0]),
        ("infinite y", nums(&[0.0]), vec![inf]),
        ("negative infinite x", nums(&[f64::NEG_INFINITY]), vec![1.0]),
        ("huge x", nums(&[1.0e301]), vec![1.0]),
        ("huge negative y", nums(&[0.0]), vec![-1.0e301]),
        ("infinite x and infinite y", nums(&[inf]), vec![inf]),
        (
            "huge y[1] before infinite y[2]",
            nums(&[0.0, 1.0, 2.0]),
            vec![nan, 1.0e301, inf],
        ),
        (
            "infinite y[1] before huge y[2]",
            nums(&[0.0, 1.0, 2.0]),
            vec![nan, inf, 1.0e301],
        ),
        (
            "huge x[2], infinite y[0]",
            nums(&[0.0, 1.0, 1.0e301]),
            vec![inf, 1.0, 1.0],
        ),
        ("categories on number axis", cats(&["a"]), vec![1.0]),
        (
            "categories on number axis, length mismatch",
            cats(&["a", "b"]),
            vec![1.0],
        ),
        (
            "categories on number axis, infinite y",
            cats(&["a"]),
            vec![inf],
        ),
    ];
    for (name, x, y) in cases {
        rec.on_axes(&mut f, &format!("line {name}"), |a| {
            a.line(x.clone(), &y, line()).map(drop)
        });
        rec.on_axes(&mut f, &format!("scatter {name}"), |a| {
            a.scatter(x, &y, scatter()).map(drop)
        });
    }
    // Gaps with non-canonical NaN payloads, both signs, quiet and signalling patterns: storage
    // must keep every bit.
    let payloads = [
        f64::from_bits(0x7ff8_0000_0000_0001),
        f64::from_bits(0xfff8_0000_0000_0000),
        f64::from_bits(0x7ff0_0000_0000_0001),
        f64::from_bits(0x7ff4_0000_0000_0000),
        f64::from_bits(0xfff0_0000_dead_beef),
        f64::from_bits(0xffff_ffff_ffff_ffff),
    ];
    let [p0, p1, p2, p3, p4, p5] = payloads;
    rec.on_axes(&mut f, "line and scatter with NaN payloads", |a| {
        a.line(
            nums(&[0.0, p0, 2.0, p1, 4.0, p2, -0.0]),
            &[p3, 1.0, p4, 3.0, p5, 5.0, 0.0],
            line(),
        )?;
        a.scatter(
            nums(&[p5, 1.0, p4, 3.0, p3]),
            &[p2, 7.0, p1, -0.0, p0],
            scatter(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "line with NaN payloads before a fault", |a| {
        a.line(nums(&[p0, p1, 2.0]), &[p2, p3, inf], line())
            .map(drop)
    });
    rec.on_axes(&mut f, "line all NaN and empty", |a| {
        a.line(nums(&[nan, 1.0]), &[2.0, nan], line())?;
        a.scatter(nums(&[]), &[], scatter()).map(drop)
    });
    rec.on_axes(&mut f, "line with options", |a| {
        a.line(
            nums(&[umber_core::MAX_MAGNITUDE, -umber_core::MAX_MAGNITUDE]),
            &[-0.0, 0.0],
            LineOptions {
                label: "edge".into(),
                color: ColorSpec::Palette {
                    index: 21,
                    alpha: Alpha::new(0.25).unwrap(),
                },
                dash: Dash::Dotted,
                ..LineOptions::default()
            },
        )?;
        a.scatter(
            nums(&[1.0]),
            &[1.0],
            ScatterOptions {
                label: "fixed".into(),
                color: ColorSpec::Fixed(rgb(1, 2, 3)).with_alpha(Alpha::new(0.5).unwrap()),
                marker: Marker::Diamond,
                ..ScatterOptions::default()
            },
        )
        .map(drop)
    });

    rec.section("Axes.line and Axes.scatter, category/log10");
    let mut f = rec.new_figure("category/log10", ScaleKind::Category, log(10.0));
    for (name, x, y) in [
        ("numbers on category axis", nums(&[1.0]), vec![1.0]),
        ("numbers on category axis, bad y", nums(&[1.0]), vec![-1.0]),
        ("zero y", cats(&["a"]), vec![0.0]),
        ("negative y", cats(&["a", "b"]), vec![5.0, -1.0]),
        ("tiny y", cats(&["a"]), vec![1.0e-301]),
        ("negative infinite y", cats(&["a"]), vec![f64::NEG_INFINITY]),
        ("huge y", cats(&["a"]), vec![1.0e301]),
        ("smallest y", cats(&["a"]), vec![1.0e-300]),
        ("NaN y", cats(&["a", "b"]), vec![nan, 3.0]),
        ("NaN payload y", cats(&["a", "b", "c"]), vec![p1, 3.0, p4]),
    ] {
        rec.on_axes(&mut f, &format!("line {name}"), |a| {
            a.line(x.clone(), &y, line()).map(drop)
        });
        rec.on_axes(&mut f, &format!("scatter {name}"), |a| {
            a.scatter(x, &y, scatter()).map(drop)
        });
    }

    // Axes with a category y axis are created, and their mark methods refuse them, so no series
    // meets one.
    rec.section("Axes.line and Axes.scatter, category y");
    rec.new_figure("linear/category", linear(), ScaleKind::Category);

    rec.section("Axes.line and Axes.scatter, log2/linear");
    let mut f = rec.new_figure("log2/linear", log(2.0), linear());
    for (name, x, y) in [
        ("zero x", nums(&[0.0]), vec![1.0]),
        ("negative x", nums(&[2.0, -1.0]), vec![1.0, 1.0]),
        ("tiny x", nums(&[1.0e-301]), vec![1.0]),
        ("negative x, infinite y", nums(&[-1.0]), vec![inf]),
        ("negative y is fine", nums(&[1.0]), vec![-1.0]),
    ] {
        rec.on_axes(&mut f, &format!("line {name}"), |a| {
            a.line(x.clone(), &y, line()).map(drop)
        });
        rec.on_axes(&mut f, &format!("scatter {name}"), |a| {
            a.scatter(x, &y, scatter()).map(drop)
        });
    }
}

fn color_errors(rec: &mut Log) {
    rec.section("Rgba::hex");
    for code in [
        "#1A2B3C",
        "#1a2b3c80",
        "#FFFFFF00",
        "1A2B3C",
        "abc",
        "",
        "#",
        "#12345",
        "#1234567",
        "#123456789",
        "#GG0000",
        "#12 456",
        "#+1A2B3",
        "#\u{ff21}\u{ff22}",
        "#\u{e9}\u{e9}\u{e9}",
        "##123456",
        " #123456",
    ] {
        rec.result(
            &format!("Rgba::hex({})", record::text(code)),
            Rgba::hex(code),
            |c| format!("{} {}", record::rgba(c), c.to_hex()),
        );
    }

    rec.section("Rgba::from_ints");
    for (r, g, b) in [
        (0, 0, 0),
        (255, 255, 255),
        (256, 0, -1),
        (0, 300, -1),
        (0, 0, -1),
        (-1, 256, 256),
        (i64::MIN, 0, 0),
        (0, i64::MAX, 0),
        (0, 0, 256),
    ] {
        rec.result(
            &format!("Rgba::from_ints({r}, {g}, {b})"),
            Rgba::from_ints(r, g, b),
            |c| format!("{} {}", record::rgba(c), c.to_hex()),
        );
    }

    rec.section("Alpha::new");
    for a in [
        0.0,
        -0.0,
        0.5,
        0.001,
        0.998,
        1.0,
        1.000_000_1,
        -1.0e-9,
        2.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        rec.result(&format!("Alpha::new({})", num(a)), Alpha::new(a), |alpha| {
            format!(
                "{} {}",
                record::alpha(alpha),
                record::rgba(rgb(10, 20, 30).with_alpha(alpha))
            )
        });
    }
}

fn scale_errors(rec: &mut Log) {
    rec.section("LogBase::new");
    for base in [
        1.0,
        0.5,
        0.0,
        -2.0,
        f64::NAN,
        f64::INFINITY,
        1.000_000_1,
        2.0,
        std::f64::consts::E,
        10.0,
        1.0e300,
    ] {
        rec.result(
            &format!("LogBase::new({})", num(base)),
            LogBase::new(base),
            |b| record::f64(b.get()),
        );
    }

    let transforms = [
        Transform::Linear,
        Transform::Log(LogBase::TEN),
        Transform::Log(LogBase::new(2.0).unwrap()),
    ];
    let domain = |s: ContinuousScale| {
        let (lo, hi) = s.domain();
        format!(
            "{} {} {}",
            record::transform(s.transform()),
            record::f64(lo),
            record::f64(hi)
        )
    };
    rec.section("ContinuousScale::new");
    for t in transforms {
        for (lo, hi) in [
            (0.0, 1.0),
            (1.0, 1.0),
            (f64::NAN, 1.0),
            (0.0, f64::INFINITY),
            (-1.0e301, 1.0),
            (1.0, 1.0 + 1.0e-12),
            (0.0, 1.0e-291),
            (0.0, 10.0),
            (1.0e-301, 10.0),
            (1.0, 1.000_000_000_1),
            (1.0e-300, 1.0e300),
        ] {
            rec.result(
                &format!(
                    "ContinuousScale::new({}, {}, {})",
                    record::transform(t),
                    num(lo),
                    num(hi)
                ),
                ContinuousScale::new(t, lo, hi, "Scale fixture"),
                domain,
            );
        }
    }
    rec.section("ContinuousScale::from_extent");
    for t in transforms {
        for extent in [
            None,
            Some((20.0, 20.0)),
            Some((0.0, 0.0)),
            Some((-3.0, 4.0)),
            Some((f64::NAN, 1.0)),
            Some((1.0, f64::INFINITY)),
            Some((2.0, 1.0)),
            Some((-1.0e301, 0.0)),
            Some((0.0, 1.0)),
            Some((-1.0, 1.0)),
            Some((1.0e-301, 1.0)),
            Some((1.0, 1.0e301)),
            Some((5.0, 5.0)),
            Some((1.0e300, 1.0e300)),
            Some((-1.0e300, -1.0e300)),
            Some((1.0e-300, 1.0e-300)),
            Some((1.0, 1.0 + 1.0e-12)),
        ] {
            let label = extent.map_or_else(
                || "None".to_owned(),
                |(lo, hi): (f64, f64)| format!("{}, {}", num(lo), num(hi)),
            );
            rec.result(
                &format!(
                    "ContinuousScale::from_extent({}, {label})",
                    record::transform(t)
                ),
                ContinuousScale::from_extent(t, extent, "Extent fixture"),
                domain,
            );
        }
    }
}

fn theme_errors(rec: &mut Log) {
    type Setter = fn(&Theme, f64) -> umber_core::Result<Theme>;
    type Getter = fn(&Theme) -> f32;
    let numeric: [(&str, Setter, Getter); 7] = [
        ("with_font_scale", Theme::with_font_scale, Theme::font_scale),
        ("with_line_width", Theme::with_line_width, Theme::line_width),
        (
            "with_marker_size",
            Theme::with_marker_size,
            Theme::marker_size,
        ),
        (
            "with_marker_alpha",
            Theme::with_marker_alpha,
            Theme::marker_alpha,
        ),
        (
            "with_baseline_width",
            Theme::with_baseline_width,
            Theme::baseline_width,
        ),
        (
            "with_bar_corner_radius",
            Theme::with_bar_corner_radius,
            Theme::bar_corner_radius,
        ),
        ("with_hist_gap", Theme::with_hist_gap, Theme::hist_gap),
    ];
    let base = Theme::graphite();
    for (name, set, get) in numeric {
        rec.section(&format!("Theme::{name}"));
        for value in SETTER_VALUES {
            rec.result(&format!("{name}({})", num(value)), set(&base, value), |t| {
                format!("{}\n{}", record::f32(get(&t)), record::theme(&t).trim_end())
            });
        }
    }

    rec.section("Theme::with_palette");
    for n in [0, 1, 8, 16, 17, 100] {
        let colors: Vec<Rgba> = (0..n).map(|i| rgb(i as u8, 2, 3)).collect();
        rec.result(
            &format!("with_palette({n} colors)"),
            base.with_palette(&colors),
            |t| format!("\n{}", record::theme(&t).trim_end()),
        );
    }

    rec.section("Theme setters without validation");
    let c = Rgba {
        r: 1,
        g: 2,
        b: 3,
        a: 4,
    };
    let rows: [(&str, umber_core::Result<Theme>); 10] = [
        ("with_background", base.with_background(c)),
        ("with_text_color", base.with_text_color(c)),
        ("with_muted_color", base.with_muted_color(c)),
        ("with_grid_color", base.with_grid_color(c)),
        ("with_baseline_color", base.with_baseline_color(c)),
        (
            "with_grid_lines",
            base.with_grid_lines(GridLines::Horizontal),
        ),
        ("with_grid_dash", base.with_grid_dash(Dash::Dashed)),
        ("with_title_font", base.with_title_font(Font::JetBrainsMono)),
        ("with_body_font", base.with_body_font(Font::IbmPlexSans)),
        (
            "with_legend_style",
            base.with_legend_style(LegendStyle::Header),
        ),
    ];
    for (name, result) in rows {
        rec.result(name, result, |t| {
            format!("\n{}", record::theme(&t).trim_end())
        });
    }
    rec.result(
        "with_fill_style",
        base.with_fill_style(FillStyle::Gradient),
        |t| format!("\n{}", record::theme(&t).trim_end()),
    );
    rec.raw("unchanged base theme\n");
    rec.raw(&record::theme(&base));
}

fn png_errors(rec: &mut Log, lib: &FontLibrary) {
    rec.section("render_png");
    let digest = |bytes: Vec<u8>| format!("bytes={} fnv1a={:016x}", bytes.len(), fnv1a(&bytes));
    let scene = Scene::new(10.0, 10.0, rgb(0, 0, 0));
    for dpi in [0, 9, 10, 72, 2400, 2401, u32::MAX] {
        rec.result(
            &format!("render_png dpi {dpi}"),
            render_png(&scene, dpi),
            digest,
        );
    }
    let black = rgb(0, 0, 0);
    for (label, w, h, dpi) in [
        ("too large", 1.0e6, 1.0e6, 2400),
        ("tiny dpi, large scene", 5.0e5, 5.0e5, 10),
        ("widest side", 32_767.0, 1.0, 72),
        ("one pixel too wide", 32_768.0, 1.0, 72),
        ("one pixel too tall", 1.0, 32_768.0, 72),
        ("too many pixels", 8_193.0, 8_192.0, 72),
        ("NaN width", f32::NAN, 10.0, 72),
        ("infinite height", 10.0, f32::INFINITY, 72),
        ("NaN width and bad dpi", f32::NAN, 10.0, 0),
        ("zero size", 0.0, 0.0, 72),
        ("negative size", -10.0, -10.0, 72),
        ("sub-pixel size", 0.1, 0.1, 10),
    ] {
        rec.result(
            &format!(
                "render_png {label} ({} x {} at {dpi})",
                num(f64::from(w)),
                num(f64::from(h))
            ),
            render_png(&Scene::new(w, h, black), dpi),
            digest,
        );
    }
    // The renderers take any figure scene; record one through the public figure path.
    let mut f = Figure::new(2.0, 1.5, Theme::midnight()).unwrap();
    f.add_axes(linear(), linear()).unwrap();
    rec.result(
        "render_png figure scene",
        render_png(&f.to_scene(lib).unwrap(), 30),
        digest,
    );
}

/// A short label for a number: plain notation for moderate magnitudes, exponent notation
/// otherwise.
fn num(v: f64) -> String {
    if v == 0.0 || !v.is_finite() || (1.0e-4..1.0e9).contains(&v.abs()) {
        format!("{v}")
    } else {
        format!("{v:e}")
    }
}

/// FNV-1a over `bytes`: a stable digest that needs no dependency.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Every getter of the built-in themes.
fn theme_cases() -> String {
    let mut out = String::new();
    for (name, theme) in themes().into_iter().chain([("default", Theme::default())]) {
        let _ = writeln!(out, "theme {name}");
        out.push_str(&record::theme(&theme));
        let _ = writeln!(out, "  panel_gap {}", record::f32(theme.panel_gap()));
        let _ = writeln!(out, "  box_width {}", record::f32(theme.box_width()));
        let _ = writeln!(out, "  violin_width {}", record::f32(theme.violin_width()));
        let _ = writeln!(
            out,
            "  bubble_max_radius {}",
            record::f32(theme.bubble_max_radius())
        );
    }
    out
}

/// Public constants and fixed lookups.
fn constant_cases() -> String {
    let mut out = String::new();
    for (name, c) in [
        ("BLACK", Rgba::BLACK),
        ("WHITE", Rgba::WHITE),
        ("GRAY", Rgba::GRAY),
        ("RED", Rgba::RED),
        ("ORANGE", Rgba::ORANGE),
        ("YELLOW", Rgba::YELLOW),
        ("GREEN", Rgba::GREEN),
        ("TEAL", Rgba::TEAL),
        ("BLUE", Rgba::BLUE),
        ("PURPLE", Rgba::PURPLE),
        ("PINK", Rgba::PINK),
        ("BROWN", Rgba::BROWN),
    ] {
        let _ = writeln!(out, "Rgba::{name} {} {}", record::rgba(c), c.to_hex());
    }
    let _ = writeln!(out, "Alpha::OPAQUE {}", record::alpha(Alpha::OPAQUE));
    let _ = writeln!(
        out,
        "ColorSpec::AUTO {}",
        record::color_spec(ColorSpec::AUTO)
    );
    let _ = writeln!(out, "LogBase::TEN {}", record::f64(LogBase::TEN.get()));
    let _ = writeln!(
        out,
        "MAX_MAGNITUDE {}",
        record::f64(umber_core::MAX_MAGNITUDE)
    );
    let _ = writeln!(
        out,
        "MIN_LOG_VALUE {}",
        record::f64(umber_core::MIN_LOG_VALUE)
    );
    let _ = writeln!(out, "MAX_GROUP_DEPTH {MAX_GROUP_DEPTH}");
    let _ = writeln!(out, "DPIS {DPIS:?}");
    for font in Font::ALL {
        let _ = writeln!(
            out,
            "Font {} family={} generic={}",
            record::font(font),
            record::text(font.family_name()),
            record::text(font.generic_family())
        );
    }
    let weights: Vec<&str> = Weight::ALL.into_iter().map(record::weight).collect();
    let _ = writeln!(out, "Weight::ALL {}", weights.join(","));
    out
}

/// Layout metrics of every bundled font and weight, through the corpus library.
fn text_layout_cases(lib: &FontLibrary) -> String {
    let mut out = String::new();
    let samples = [
        "",
        "Revenue \u{2212}12.5%",
        "0123456789",
        "AV To fi ffl",
        "\u{E000}\u{E000}",
        "caf\u{e9} \u{65e5}",
    ];
    for font in Font::ALL {
        for weight in Weight::ALL {
            for tabular in [false, true] {
                for size in [11.0, 0.0, -1.0, f32::NAN, 1.0e4] {
                    for sample in samples {
                        let s = style(font, weight, size, tabular);
                        let layout = lib.layout(sample, &s);
                        let outline = layout.outline(pt(0.0, 0.0));
                        let _ = writeln!(
                            out,
                            "{} {} width={} ascent={} descent={} empty={} verbs={}",
                            record::text_style(&s),
                            record::text(sample),
                            record::f32(layout.width()),
                            record::f32(layout.ascent()),
                            record::f32(layout.descent()),
                            layout.is_empty(),
                            outline.verbs().len()
                        );
                    }
                }
            }
        }
    }
    out
}
