//! The corpus behind the refactor output gate.
//!
//! Every case is a figure or a hand-built scene. For each one the gate records the Scene debug
//! serialization, the SVG bytes in both text modes, and the PNG bytes at two DPIs; see
//! `scripts/check-refactor-output.sh`. The corpus uses only public APIs and the in-memory
//! fallback font fixture, never system fonts, so one revision gives the same bytes on every
//! run. Cases only ever grow: change an existing case and every baseline has to be regenerated.
//!
//! This file is included with `#[path]` by `examples/refactor_outputs.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use umber_core::figure::{AxisData, LineOptions, ScatterOptions};
use umber_core::render::{SvgText, render_png, render_svg};
use umber_core::scene::{
    FillRule, GradientStop, Item, LineCap, LineJoin, LinearGradient, MAX_GROUP_DEPTH, Paint, Rect,
    Scene, StrokeStyle,
};
use umber_core::{
    Alpha, AreaFill, ColorSpec, Dash, Figure, Font, FontLibrary, GridLines, LegendStyle, LogBase,
    Marker, Path, Point, Rgba, ScaleKind, TextStyle, Theme, TickLabelPlacement, Weight,
};

/// PNG resolutions of every case.
pub const DPIS: [u32; 2] = [72, 200];

/// One generated file.
pub struct Output {
    /// `scene`, `svg`, `png` or `errors`; also the directory the driver writes it to.
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
    sink(Output {
        kind: "errors",
        file: "api.txt".into(),
        bytes: error_cases().into_bytes(),
    });
}

fn emit_scene(name: &str, scene: &Scene, sink: &mut impl FnMut(Output)) {
    sink(Output {
        kind: "scene",
        file: format!("{name}.txt"),
        bytes: format!("{scene:?}\n").into_bytes(),
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
        // A scene the PNG backend refuses is recorded by its error text.
        let bytes = match render_png(scene, dpi) {
            Ok(bytes) => bytes,
            Err(e) => format!("error: {e}\n").into_bytes(),
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

fn axes(figure: &mut Figure) -> &mut umber_core::Axes {
    figure.axes_mut().unwrap()
}

fn to_scene(figure: &Figure, lib: &FontLibrary) -> Scene {
    figure.to_scene(lib)
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
            .with_tick_label_placement(TickLabelPlacement::Outside)
            .unwrap()
            .with_area_fill(AreaFill::Gradient)
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
                    label: format!("{marker:?}"),
                    color: if k == 4 {
                        ColorSpec::Fixed(rgb(220, 80, 20))
                    } else {
                        ColorSpec::Auto(Alpha::new(0.7).unwrap())
                    },
                    marker,
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

    let mut long_names = figure(3.0, 3.0, Theme::pigment(), ScaleKind::Category, log(10.0));
    labelled(&mut long_names, "Long category names");
    axes(&mut long_names)
        .line(
            AxisData::Categories(
                (0..14)
                    .map(|i| format!("category number {i} with a long name"))
                    .collect(),
            ),
            &(0..14).map(|i| 10f64.powi(i % 5)).collect::<Vec<_>>(),
            line(),
        )
        .unwrap();
    add("fig-category-crowded-log", to_scene(&long_names, lib));

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

    let mut tiny = figure(
        1.0,
        1.0,
        Theme::pigment().with_font_scale(3.0).unwrap(),
        linear(),
        linear(),
    );
    axes(&mut tiny)
        .title(&"Wide title ".repeat(30))
        .subtitle("Sub")
        .xlabel("x label that is far too long for the figure")
        .ylabel("y label that is far too long for the figure");
    axes(&mut tiny)
        .line(numbers(vec![0.0, 1.0]), &[0.0, 1.0], line())
        .unwrap();
    add("fig-squeezed-layout", to_scene(&tiny, lib));

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

/// Error messages of public APIs, one per line. A refactor must not change a category, a
/// context or a message.
fn error_cases() -> String {
    fn show<T>(out: &mut String, label: &str, result: umber_core::Result<T>) {
        match result {
            Ok(_) => out.push_str(&format!("{label}: ok\n")),
            Err(e) => out.push_str(&format!("{label}: {e:?} | {e}\n")),
        }
    }
    let mut out = String::new();
    for (w, h) in [
        (0.0, 1.0),
        (1.0, -1.0),
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
        (1.0e9, 1.0),
    ] {
        show(
            &mut out,
            &format!("Figure::new({w}, {h})"),
            Figure::new(w, h, Theme::pigment()),
        );
    }
    let mut both_category = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    show(
        &mut out,
        "add_axes category/category",
        both_category
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .map(|_| ()),
    );
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    show(
        &mut out,
        "add_axes linear/linear",
        f.add_axes(linear(), linear()).map(|_| ()),
    );
    show(
        &mut out,
        "add_axes twice",
        f.add_axes(linear(), linear()).map(|_| ()),
    );
    let a = f.axes_mut().unwrap();
    for (lo, hi) in [
        (1.0, 1.0),
        (2.0, 1.0),
        (f64::NAN, 1.0),
        (0.0, f64::INFINITY),
        (1.0, 1.0 + 1.0e-12),
        (-1.0e301, 1.0),
    ] {
        show(
            &mut out,
            &format!("xlim({lo}, {hi})"),
            a.xlim(lo, hi).map(|_| ()),
        );
    }
    show(
        &mut out,
        "line length mismatch",
        a.line(numbers(vec![0.0]), &[1.0, 2.0], line()).map(|_| ()),
    );
    show(
        &mut out,
        "line infinite y",
        a.line(numbers(vec![0.0]), &[f64::INFINITY], line())
            .map(|_| ()),
    );
    show(
        &mut out,
        "line huge x",
        a.line(numbers(vec![1.0e301]), &[1.0], line()).map(|_| ()),
    );
    show(
        &mut out,
        "line categories on number axis",
        a.line(AxisData::Categories(vec!["a".into()]), &[1.0], line())
            .map(|_| ()),
    );
    show(&mut out, "marks after errors", Ok(a.marks().len()));
    let mut g = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let a = g.add_axes(ScaleKind::Category, log(10.0)).unwrap();
    show(&mut out, "category xlim", a.xlim(1.0, 2.0).map(|_| ()));
    show(
        &mut out,
        "category x with numbers",
        a.line(numbers(vec![1.0]), &[1.0], line()).map(|_| ()),
    );
    let mut g = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let a = g.add_axes(log(10.0), log(2.0)).unwrap();
    show(
        &mut out,
        "log x of zero",
        a.line(numbers(vec![0.0]), &[1.0], line()).map(|_| ()),
    );
    show(
        &mut out,
        "log y negative",
        a.scatter(numbers(vec![1.0]), &[-1.0], scatter())
            .map(|_| ()),
    );
    show(
        &mut out,
        "log xlim non-positive",
        a.xlim(0.0, 10.0).map(|_| ()),
    );
    show(&mut out, "LogBase 1", LogBase::new(1.0));
    show(&mut out, "LogBase NaN", LogBase::new(f64::NAN));
    show(&mut out, "Alpha 2", Alpha::new(2.0));
    show(&mut out, "Alpha NaN", Alpha::new(f64::NAN));
    show(&mut out, "Rgba::from_ints", Rgba::from_ints(256, 0, -1));
    let t = Theme::graphite();
    show(&mut out, "with_line_width(-1)", t.with_line_width(-1.0));
    show(
        &mut out,
        "with_line_width(NaN)",
        t.with_line_width(f64::NAN),
    );
    show(&mut out, "with_font_scale(0)", t.with_font_scale(0.0));
    show(&mut out, "with_font_scale(100)", t.with_font_scale(100.0));
    show(&mut out, "with_marker_alpha(2)", t.with_marker_alpha(2.0));
    show(&mut out, "with_marker_size(1e9)", t.with_marker_size(1.0e9));
    show(&mut out, "with_palette(empty)", t.with_palette(&[]));
    show(
        &mut out,
        "with_palette(100 colors)",
        t.with_palette(&vec![rgb(1, 2, 3); 100]),
    );
    show(&mut out, "with_hist_gap(2)", t.with_hist_gap(2.0));
    show(
        &mut out,
        "with_bar_corner_radius(-1)",
        t.with_bar_corner_radius(-1.0),
    );
    show(
        &mut out,
        "with_baseline_width(1e9)",
        t.with_baseline_width(1.0e9),
    );
    let scene = Scene::new(10.0, 10.0, rgb(0, 0, 0));
    for dpi in [0, 9, 10, 2400, 2401, u32::MAX] {
        show(
            &mut out,
            &format!("render_png dpi {dpi}"),
            render_png(&scene, dpi).map(|b| b.len()),
        );
    }
    show(
        &mut out,
        "render_png too large",
        render_png(&Scene::new(1.0e6, 1.0e6, rgb(0, 0, 0)), 2400).map(|b| b.len()),
    );
    show(
        &mut out,
        "render_png tiny dpi large scene",
        render_png(&Scene::new(5.0e5, 5.0e5, rgb(0, 0, 0)), 10).map(|b| b.len()),
    );
    out
}
