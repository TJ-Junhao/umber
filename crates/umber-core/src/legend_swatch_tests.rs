//! Legend swatches show each mark's fill as drawn: composited over the plot background, in
//! panel and merged legends alike, while strokes keep the series color.
//!
//! Expected colors are written out. `#D64933` (red) at alpha `a` composited over an opaque
//! background `bg` is `round((c a + bg (255 - a)) / 255)` per channel: for the 0.32 area share
//! (`a = round(255 × 0.32) = 82`) over the pigment background `#FAF6F0`, red is
//! `(214 × 82 + 250 × 173) / 255 = 238.4`, so 238, green `(73 × 82 + 246 × 173) / 255 = 190.4`
//! and blue `(51 × 82 + 240 × 173) / 255 = 179.2`: `(238, 190, 179)`.

use crate::layout::{Layout, RowEntry};
use crate::pipeline::plan_figure;
use crate::{
    AreaOptions, AxisData, AxisValue, BandOptions, BarOptions, BoxOptions, BubbleOptions,
    ColorSpec, Dash, ErrorBarOptions, Figure, FillStyle, FontLibrary, Grid, GridIndex, HistOptions,
    Item, LegendMode, LegendStyle, LineOptions, Paint, Placement, PointColor, Rgba, RuleOptions,
    ScaleKind, ScatterOptions, Series, SpanOptions, StemOptions, StepOptions, Theme, ViolinOptions,
};

/// How a mark's swatch is filled, for the table of expected colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fill {
    /// The 0.32 area share.
    Area,
    /// 0.95 under a gradient, otherwise solid.
    Histogram,
    /// Solid.
    Solid,
    /// The theme's marker alpha: 0.85 for pigment, 0.90 for graphite and midnight.
    Marker,
}

/// `#D64933` at alpha `alpha`, filled as `fill` says in the theme `theme` under `style`, over
/// that theme's background: the alpha share first rounds to 8 bits (82 and 41 for the area
/// share of 255 and 128, 242 and 122 for 0.95, 217 and 109 for 0.85, 230 and 115 for 0.90).
fn expected(theme: &str, alpha: u8, fill: Fill, style: FillStyle) -> Rgba {
    let fill = match (fill, style) {
        (Fill::Histogram, FillStyle::Gradient) => Fill::Histogram,
        (Fill::Histogram, FillStyle::Solid) => Fill::Solid,
        (fill, _) => fill,
    };
    let (r, g, b) = match (theme, alpha, fill) {
        ("pigment", 255, Fill::Area) => (238, 190, 179),
        ("pigment", 255, Fill::Histogram) => (216, 82, 61),
        ("pigment", 255, Fill::Solid) => (214, 73, 51),
        ("pigment", 255, Fill::Marker) => (219, 99, 79),
        ("pigment", 128, Fill::Area) => (244, 218, 210),
        ("pigment", 128, Fill::Histogram) => (233, 163, 150),
        ("pigment", 128, Fill::Solid) => (232, 159, 145),
        ("pigment", 128, Fill::Marker) => (235, 172, 159),
        ("graphite", 255, Fill::Area) => (242, 196, 189),
        ("graphite", 255, Fill::Histogram) => (216, 82, 61),
        ("graphite", 255, Fill::Solid) => (214, 73, 51),
        ("graphite", 255, Fill::Marker) => (218, 91, 71),
        ("graphite", 128, Fill::Area) => (248, 226, 222),
        ("graphite", 128, Fill::Histogram) => (235, 168, 157),
        ("graphite", 128, Fill::Solid) => (234, 164, 153),
        ("graphite", 128, Fill::Marker) => (237, 173, 163),
        ("midnight", 255, Fill::Area) => (81, 37, 33),
        ("midnight", 255, Fill::Histogram) => (204, 70, 50),
        ("midnight", 255, Fill::Solid) => (214, 73, 51),
        ("midnight", 255, Fill::Marker) => (195, 68, 48),
        ("midnight", 128, Fill::Area) => (50, 29, 29),
        ("midnight", 128, Fill::Histogram) => (112, 45, 37),
        ("midnight", 128, Fill::Solid) => (116, 47, 38),
        ("midnight", 128, Fill::Marker) => (106, 44, 37),
        other => panic!("no expected color for {other:?}"),
    };
    Rgba::opaque(r, g, b)
}

fn themes() -> [(&'static str, Theme); 3] {
    [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ]
}

/// Every color the legend of `layout` draws with: fills, markers and strokes.
fn legend_colors(theme: &Theme, layout: &Layout) -> Vec<Rgba> {
    let mut colors = Vec::new();
    if let Some(item) = crate::draw::legend::legend(theme, layout) {
        collect_colors(&item, &mut colors);
    }
    colors
}

fn collect_colors(item: &Item, colors: &mut Vec<Rgba>) {
    match item {
        Item::Stroke { style, .. } => colors.push(style.color),
        Item::Markers {
            fill: Some(color), ..
        } => colors.push(*color),
        Item::Fill {
            paint: Paint::Solid(color),
            ..
        } => colors.push(*color),
        Item::Group { items, .. } => items.iter().for_each(|item| collect_colors(item, colors)),
        _ => {}
    }
}

/// A figure in `theme` with the one filled mark `name`, labelled, in `color`, and how its
/// swatch is filled.
fn filled(name: &str, theme: &Theme, color: Rgba) -> (Figure, Fill) {
    let category = matches!(name, "bar" | "box" | "violin");
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    let mut a = f
        .add_axes(
            if category {
                ScaleKind::Category
            } else {
                ScaleKind::Linear
            },
            ScaleKind::Linear,
        )
        .unwrap();
    let (label, spec) = (name.to_owned(), ColorSpec::Fixed(color));
    let fill = match name {
        "area" => {
            a.area(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                AreaOptions {
                    label,
                    color: spec,
                    ..AreaOptions::default()
                },
            )
            .unwrap();
            Fill::Area
        }
        "stacked-area" => {
            a.stacked_area(
                &[0.0, 1.0],
                vec![Series {
                    label,
                    color: spec,
                    values: vec![1.0, 2.0],
                }],
            )
            .unwrap();
            Fill::Area
        }
        "histogram" => {
            a.hist(
                &[1.0, 2.0, 3.0],
                HistOptions {
                    label,
                    color: spec,
                    bins: crate::Bins::Count(2),
                    ..HistOptions::default()
                },
            )
            .unwrap();
            Fill::Histogram
        }
        "bar" => {
            a.bar(
                vec!["A".into()],
                &[1.0],
                BarOptions {
                    label,
                    color: spec,
                    ..BarOptions::default()
                },
            )
            .unwrap();
            Fill::Solid
        }
        "box" => {
            a.box_plot(
                vec!["A".into()],
                vec![vec![1.0, 2.0, 3.0]],
                BoxOptions { label, color: spec },
            )
            .unwrap();
            Fill::Area
        }
        "violin" => {
            a.violin(
                vec!["A".into()],
                vec![vec![1.0, 2.0, 3.0]],
                ViolinOptions { label, color: spec },
            )
            .unwrap();
            Fill::Area
        }
        "bubbles" => {
            a.bubbles(
                &[0.5],
                &[0.5],
                &[1.0],
                BubbleOptions {
                    label,
                    color: PointColor::Uniform(spec),
                    ..BubbleOptions::default()
                },
            )
            .unwrap();
            Fill::Marker
        }
        "stem" => {
            a.stem(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                StemOptions {
                    label,
                    color: spec,
                    ..StemOptions::default()
                },
            )
            .unwrap();
            Fill::Marker
        }
        "scatter" => {
            a.scatter(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                ScatterOptions {
                    label,
                    color: PointColor::Uniform(spec),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
            Fill::Marker
        }
        "band" => {
            a.band(
                &[0.0, 1.0],
                &[1.0, 2.0],
                &[2.0, 3.0],
                BandOptions { label, color: spec },
            )
            .unwrap();
            Fill::Area
        }
        "hspan" => {
            a.hspan(1.0, 2.0, SpanOptions { label, color: spec })
                .unwrap();
            Fill::Area
        }
        "vspan" => {
            a.vspan(1.0, 2.0, SpanOptions { label, color: spec })
                .unwrap();
            Fill::Area
        }
        _ => unreachable!(),
    };
    (f, fill)
}

const FILLED: [&str; 12] = [
    "area",
    "stacked-area",
    "histogram",
    "bar",
    "box",
    "violin",
    "bubbles",
    "scatter",
    "stem",
    "band",
    "hspan",
    "vspan",
];

#[test]
fn filled_swatches_show_the_fill_as_drawn_over_the_background() {
    for (name, theme) in themes() {
        for fill_style in [FillStyle::Solid, FillStyle::Gradient] {
            for style in [LegendStyle::Header, LegendStyle::Chips] {
                for alpha in [255, 128] {
                    let theme = theme
                        .with_fill_style(fill_style)
                        .unwrap()
                        .with_legend_style(style)
                        .unwrap();
                    let color = Rgba {
                        a: alpha,
                        ..Rgba::RED
                    };
                    for mark in FILLED {
                        let (f, fill) = filled(mark, &theme, color);
                        let expected = expected(name, alpha, fill, fill_style);
                        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
                        let entries = layout.legend.entries();
                        assert!(!entries.is_empty(), "{mark}");
                        for entry in entries {
                            let case = format!("{name} {mark}: {fill_style:?}, {style:?}, {alpha}");
                            assert_eq!(entry.swatch.fill, expected, "{case}");
                            assert_eq!(entry.swatch.color, color, "{case}");
                        }
                        let colors = legend_colors(&theme, &layout);
                        assert!(colors.contains(&expected), "{mark}: {colors:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn stroked_swatches_and_direct_labels_keep_the_series_color() {
    let color = Rgba {
        a: 128,
        ..Rgba::RED
    };
    let spec = ColorSpec::Fixed(color);
    let numbers = || AxisData::Numbers(vec![0.0, 1.0]);
    for style in [LegendStyle::Header, LegendStyle::Chips] {
        let mut f = Figure::new(
            6.0,
            4.0,
            Theme::graphite().with_legend_style(style).unwrap(),
        )
        .unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                numbers(),
                &[1.0, 2.0],
                LineOptions {
                    label: "Line".into(),
                    color: spec,
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .step(
                numbers(),
                &[1.0, 2.0],
                StepOptions {
                    label: "Step".into(),
                    color: spec,
                    ..StepOptions::default()
                },
            )
            .unwrap()
            .hline(
                1.5,
                RuleOptions {
                    label: "Rule".into(),
                    color: spec,
                    ..RuleOptions::default()
                },
            )
            .unwrap()
            .vline(
                AxisValue::Number(0.5),
                RuleOptions {
                    label: "Vertical".into(),
                    color: spec,
                    ..RuleOptions::default()
                },
            )
            .unwrap()
            .y_error_bars(
                numbers(),
                &[0.5, 1.5],
                &[1.5, 2.5],
                ErrorBarOptions {
                    label: "Errors".into(),
                    color: spec,
                    ..ErrorBarOptions::default()
                },
            )
            .unwrap();
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        assert_eq!(layout.legend.entries().len(), 5, "{style:?}");
        for entry in layout.legend.entries() {
            assert_eq!(entry.swatch.fill, color, "{}: {style:?}", entry.label.text);
            assert_eq!(entry.swatch.color, color);
        }
        if style == LegendStyle::Header {
            let colors = legend_colors(f.theme(), &layout);
            assert!(colors.iter().all(|c| *c == color), "{colors:?}");
        }
    }
    // An area's direct label keeps the series color, while its legend row swatch would not.
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .area(
            numbers(),
            &[1.0, 2.0],
            AreaOptions {
                label: "Area".into(),
                color: spec,
                ..AreaOptions::default()
            },
        )
        .unwrap();
    let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(layout.direct[0].color, color);
    assert!(layout.legend.entries().is_empty());
}

/// The swatch of an area or a stacked area under a header legend: a rectangle filled with the
/// fill as drawn, under an edge stroked in the series color, dash and width, all inside the
/// swatch box.
#[test]
fn area_swatches_are_filled_rectangles_under_a_stroked_edge() {
    for (name, theme) in themes() {
        let theme = theme.with_legend_style(LegendStyle::Header).unwrap();
        for (mark, dash) in [
            ("area", Dash::Solid),
            ("area", Dash::Dashed),
            ("stacked-area", Dash::Solid),
        ] {
            let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
            let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            if mark == "area" {
                a.area(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[1.0, 2.0],
                    AreaOptions {
                        label: "Area".into(),
                        color: ColorSpec::Fixed(Rgba::RED),
                        dash,
                        ..AreaOptions::default()
                    },
                )
                .unwrap();
            } else {
                a.stacked_area(
                    &[0.0, 1.0],
                    vec![Series {
                        label: "Layer".into(),
                        color: ColorSpec::Fixed(Rgba::RED),
                        values: vec![1.0, 2.0],
                    }],
                )
                .unwrap();
            }
            let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
            let entry: &RowEntry = &layout.legend.entries()[0];
            let Some(Item::Group { items, .. }) = crate::draw::legend::legend(&theme, &layout)
            else {
                panic!("a legend group")
            };
            let Item::Group { items: swatch, .. } = &items[0] else {
                panic!(
                    "{name} {mark}: a filled swatch is a group, not {:?}",
                    items[0]
                );
            };
            let [
                Item::Fill {
                    path: fill,
                    paint: Paint::Solid(fill_color),
                    ..
                },
                Item::Stroke { path: edge, style },
            ] = swatch.as_slice()
            else {
                panic!("{name} {mark}: a fill and its edge, not {swatch:?}");
            };
            assert_eq!(
                *fill_color,
                expected(name, 255, Fill::Area, FillStyle::Solid)
            );
            assert_eq!(style.color, Rgba::RED);
            assert_eq!(style.dash.is_empty(), dash == Dash::Solid, "{name} {mark}");
            let b = entry.swatch;
            let (top, bottom) = (b.center_y - b.height / 2.0, b.center_y + b.height / 2.0);
            let half = f64::from(style.width) / 2.0;
            let (lo, hi) = fill.bounds().unwrap();
            let (edge_lo, edge_hi) = edge.bounds().unwrap();
            // The fill spans the box from the edge's centre line to its bottom.
            assert!((f64::from(lo.x) - b.x).abs() < 1e-3);
            assert!((f64::from(hi.x) - (b.x + b.width)).abs() < 1e-3);
            assert!((f64::from(lo.y) - (top + half)).abs() < 1e-3);
            assert!((f64::from(hi.y) - bottom).abs() < 1e-3);
            // The edge is a horizontal line along the top, its stroke inside the box.
            assert_eq!(edge_lo.y, edge_hi.y);
            assert!((f64::from(edge_lo.y) - (top + half)).abs() < 1e-3);
            assert!(f64::from(edge_lo.x) >= b.x - 1e-3);
            assert!(f64::from(edge_hi.x) <= b.x + b.width + 1e-3);
            assert!(
                b.height >= 4.0 * half,
                "{name} {mark}: the edge leaves room for the fill"
            );
        }
    }
}

#[test]
fn swatches_composite_over_a_translucent_background() {
    // Source-over: with fill alpha `f = 82 / 255` and background alpha `b`, the alpha is
    // `f + b (1 - f)` and each channel `(c f + c_b b (1 - f)) / alpha`.
    for (background, swatch) in [
        // Over a transparent background the swatch is the translucent fill itself.
        (
            Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            Rgba {
                r: 214,
                g: 73,
                b: 51,
                a: 82,
            },
        ),
        // Over white at 128: alpha `82 + 128 × 173 / 255 = 168.8`, so 169; red
        // `(214 × 82 + 255 × 128 × 173 / 255) / 168.8 = 235.1`, green
        // `(73 × 82 + 128 × 173) / 168.8 = 166.6`, blue `(51 × 82 + 128 × 173) / 168.8 = 155.9`.
        (
            Rgba {
                r: 255,
                g: 255,
                b: 255,
                a: 128,
            },
            Rgba {
                r: 235,
                g: 167,
                b: 156,
                a: 169,
            },
        ),
        // Over the pigment background `#FAF6F0` at 128.
        (
            Rgba {
                r: 0xFA,
                g: 0xF6,
                b: 0xF0,
                a: 128,
            },
            Rgba {
                r: 233,
                g: 162,
                b: 148,
                a: 169,
            },
        ),
    ] {
        let theme = Theme::pigment().with_background(background).unwrap();
        let mut f = Figure::new(4.0, 3.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .band(
                &[0.0, 1.0, 2.0],
                &[0.0, 0.5, 0.2],
                &[1.0, 1.5, 1.2],
                BandOptions {
                    label: "Band".into(),
                    color: ColorSpec::Fixed(Rgba::RED),
                },
            )
            .unwrap();
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        assert_eq!(
            layout.legend.entries()[0].swatch.fill,
            swatch,
            "{background:?}"
        );
        assert!(legend_colors(&theme, &layout).contains(&swatch));
    }
}

#[test]
fn band_and_span_swatches_match_in_every_theme_and_legend_mode() {
    // Red, green and blue at the 0.32 area share (alpha 82) over each theme's background.
    let expected = |theme: &str| match theme {
        "pigment" => [(238, 190, 179), (195, 216, 193), (186, 200, 222)],
        "graphite" => [(242, 196, 189), (198, 223, 203), (189, 206, 232)],
        "midnight" => [(81, 37, 33), (37, 63, 47), (29, 46, 76)],
        _ => unreachable!(),
    };
    for (name, theme) in themes() {
        for mode in [LegendMode::PerPanel, LegendMode::Merged] {
            for style in [LegendStyle::Header, LegendStyle::Chips, LegendStyle::Direct] {
                let theme = theme.with_legend_style(style).unwrap();
                let mut f = Figure::with_grid(
                    8.0,
                    4.0,
                    theme.clone(),
                    Grid::new(1, 2).unwrap().with_legend(mode),
                )
                .unwrap();
                for col in 0..2 {
                    let mut a = f
                        .add_axes_at(
                            Placement {
                                row: GridIndex::At(0),
                                col: GridIndex::At(col),
                                ..Placement::default()
                            },
                            ScaleKind::Linear,
                            ScaleKind::Linear,
                        )
                        .unwrap();
                    a.band(
                        &[0.0, 1.0],
                        &[1.0, 2.0],
                        &[2.0, 3.0],
                        BandOptions {
                            label: "Band".into(),
                            color: ColorSpec::Fixed(Rgba::RED),
                        },
                    )
                    .unwrap();
                    a.hspan(
                        1.0,
                        2.0,
                        SpanOptions {
                            label: "Horizontal".into(),
                            color: ColorSpec::Fixed(Rgba::GREEN),
                        },
                    )
                    .unwrap();
                    a.vspan(
                        0.2,
                        0.8,
                        SpanOptions {
                            label: "Vertical".into(),
                            color: ColorSpec::Fixed(Rgba::BLUE),
                        },
                    )
                    .unwrap();
                }
                let scene = f.to_scene(FontLibrary::global()).unwrap();
                let mut colors = Vec::new();
                for item in &scene.items {
                    collect_colors(item, &mut colors);
                }
                for (r, g, b) in expected(name) {
                    let expected = Rgba::opaque(r, g, b);
                    assert!(
                        colors.contains(&expected),
                        "{name} {mode:?}, {style:?}: missing {expected:?} in {colors:?}"
                    );
                }
            }
        }
    }
}
