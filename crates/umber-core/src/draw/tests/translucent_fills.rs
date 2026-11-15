//! Fill colors: a translucent color is used exactly as given and an opaque one gets the mark's
//! default fill opacity, for every mark that has one; and the width of a scatter's joining line.

use super::super::fill::AREA_ALPHA;
use super::{mark_group, merged_layers, nums};
use crate::model::{
    AreaOptions, BandOptions, BoxOptions, BubbleOptions, HistOptions, SpanOptions, StemOptions,
    ViolinOptions,
};
use crate::scene::{Item, Paint};
use crate::{
    Alpha, ColorSpec, Figure, FillStyle, FontLibrary, PointColor, Rgba, ScaleKind, ScatterOptions,
    SizeSpec, Theme,
};

const LIN: ScaleKind = ScaleKind::Linear;
const BLUE: Rgba = Rgba {
    r: 31,
    g: 119,
    b: 180,
    a: 255,
};

/// An alpha byte as a fixed color.
fn fixed(a: u8) -> ColorSpec {
    ColorSpec::Fixed(Rgba { a, ..BLUE })
}

/// Where a mark is drawn on `figure`'s one axes.
type Draw = fn(&mut Figure, ColorSpec);

fn figure(theme: Theme, x: ScaleKind) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(x, LIN).unwrap();
    f
}

/// The opacity the first fill or marker batch of the marks of `f` is drawn at: a flat fill's,
/// the opaque end of a gradient's, or a marker fill's.
fn first_alpha(f: &Figure) -> u8 {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    merged_layers(mark_group(&scene).1)
        .iter()
        .find_map(|item| match item {
            Item::Fill {
                paint: Paint::Solid(color),
                ..
            } => Some(color.a),
            Item::Fill {
                paint: Paint::Linear(gradient),
                ..
            } => Some(gradient.stops[0].color.a),
            Item::Markers {
                fill: Some(color), ..
            } => Some(color.a),
            _ => None,
        })
        .expect("the mark draws a fill")
}

/// Every fill mark with its default opacity in an opaque color, on a theme that fills flat.
fn flat_marks() -> Vec<(&'static str, Draw, u8)> {
    let area = (AREA_ALPHA * 255.0).round() as u8;
    let marker = (0.85_f32 * 255.0).round() as u8;
    vec![
        (
            "band",
            |f, c| {
                f.single_axes_mut()
                    .band(
                        &[0.0, 1.0, 2.0],
                        &[1.0, 2.0, 1.0],
                        &[3.0, 4.0, 3.0],
                        BandOptions {
                            color: c,
                            ..BandOptions::default()
                        },
                    )
                    .unwrap();
            },
            area,
        ),
        (
            "area",
            |f, c| {
                f.single_axes_mut()
                    .area(
                        nums(&[0.0, 1.0, 2.0]),
                        &[3.0, 5.0, 4.0],
                        AreaOptions {
                            color: c,
                            ..AreaOptions::default()
                        },
                    )
                    .unwrap();
            },
            area,
        ),
        (
            "hspan",
            |f, c| {
                f.single_axes_mut()
                    .line(nums(&[0.0, 1.0]), &[0.0, 5.0], Default::default())
                    .unwrap()
                    .hspan(
                        2.0,
                        3.0,
                        SpanOptions {
                            color: c,
                            ..SpanOptions::default()
                        },
                    )
                    .unwrap();
            },
            area,
        ),
        (
            "vspan",
            |f, c| {
                f.single_axes_mut()
                    .line(nums(&[0.0, 1.0]), &[0.0, 5.0], Default::default())
                    .unwrap()
                    .vspan(
                        0.2,
                        0.6,
                        SpanOptions {
                            color: c,
                            ..SpanOptions::default()
                        },
                    )
                    .unwrap();
            },
            area,
        ),
        (
            "hist",
            |f, c| {
                f.single_axes_mut()
                    .hist(
                        &[1.0, 2.0, 2.0, 3.0, 3.0, 3.0],
                        HistOptions {
                            color: c,
                            ..HistOptions::default()
                        },
                    )
                    .unwrap();
            },
            255,
        ),
        (
            "scatter",
            |f, c| {
                f.single_axes_mut()
                    .scatter(
                        nums(&[0.5]),
                        &[0.5],
                        ScatterOptions {
                            color: PointColor::Uniform(c),
                            ..ScatterOptions::default()
                        },
                    )
                    .unwrap();
            },
            marker,
        ),
        (
            "bubbles",
            |f, c| {
                f.single_axes_mut()
                    .bubbles(
                        &[0.5],
                        &[0.5],
                        &[1.0],
                        BubbleOptions {
                            color: PointColor::Uniform(c),
                            ..BubbleOptions::default()
                        },
                    )
                    .unwrap();
            },
            marker,
        ),
        (
            "stem",
            |f, c| {
                f.single_axes_mut()
                    .stem(
                        nums(&[0.5]),
                        &[0.5],
                        StemOptions {
                            color: c,
                            ..StemOptions::default()
                        },
                    )
                    .unwrap();
            },
            marker,
        ),
    ]
}

#[test]
fn translucent_fills_are_used_as_given_and_opaque_ones_get_the_default() {
    for (name, draw, default) in flat_marks() {
        let theme = Theme::pigment();
        for (color, expected) in [
            (fixed(255), default),
            (
                ColorSpec::Fixed(BLUE).with_alpha(Alpha::new(1.0).unwrap()),
                default,
            ),
            (fixed(38), 38),
            (fixed(100), 100),
            (fixed(254), 254),
            (fixed(1), 1),
        ] {
            let mut f = figure(theme.clone(), LIN);
            draw(&mut f, color);
            assert_eq!(first_alpha(&f), expected, "{name} in {color:?}");
        }
        // The default colors draw at the default, in an automatic color too.
        let mut f = figure(theme, LIN);
        draw(&mut f, ColorSpec::AUTO);
        assert_eq!(first_alpha(&f), default, "{name} in an automatic color");
    }
}

#[test]
fn a_translucent_palette_color_is_used_as_given() {
    let half = Alpha::new(0.5).unwrap();
    for (name, draw, _) in flat_marks() {
        let mut f = figure(Theme::graphite(), LIN);
        draw(&mut f, ColorSpec::AUTO.with_alpha(half));
        assert_eq!(first_alpha(&f), half.to_u8(), "{name}");
    }
}

#[test]
fn gradient_fills_start_at_a_translucent_color_as_given() {
    for (color, area, hist) in [(fixed(255), 82, 242), (fixed(100), 100, 100)] {
        let theme = Theme::pigment()
            .with_fill_style(FillStyle::Gradient)
            .unwrap();
        let mut f = figure(theme.clone(), LIN);
        f.single_axes_mut()
            .area(
                nums(&[0.0, 1.0, 2.0]),
                &[3.0, 5.0, 4.0],
                AreaOptions {
                    color,
                    ..AreaOptions::default()
                },
            )
            .unwrap();
        assert_eq!(first_alpha(&f), area, "area in {color:?}");
        let mut f = figure(theme, LIN);
        f.single_axes_mut()
            .hist(
                &[1.0, 2.0, 2.0, 3.0, 3.0, 3.0],
                HistOptions {
                    color,
                    ..HistOptions::default()
                },
            )
            .unwrap();
        assert_eq!(first_alpha(&f), hist, "hist in {color:?}");
    }
}

#[test]
fn boxes_and_violins_use_a_translucent_color_as_given() {
    let groups = || vec![vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]];
    for (color, expected) in [(fixed(255), 82), (fixed(90), 90)] {
        let mut f = figure(Theme::pigment(), ScaleKind::Category);
        f.single_axes_mut()
            .box_plot(
                vec!["A".into()],
                groups(),
                BoxOptions {
                    color,
                    ..BoxOptions::default()
                },
            )
            .unwrap();
        assert_eq!(first_alpha(&f), expected, "box in {color:?}");
        let mut f = figure(Theme::pigment(), ScaleKind::Category);
        f.single_axes_mut()
            .violin(
                vec!["A".into()],
                groups(),
                ViolinOptions {
                    color,
                    ..ViolinOptions::default()
                },
            )
            .unwrap();
        assert_eq!(first_alpha(&f), expected, "violin in {color:?}");
    }
}

/// The stroke widths of the marks of `f`, in drawing order.
fn stroke_widths(f: &Figure) -> Vec<f32> {
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    mark_group(&scene)
        .1
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { style, .. } => Some(style.width),
            _ => None,
        })
        .collect()
}

fn connected(width: SizeSpec, y_error: bool) -> Figure {
    let theme = Theme::pigment();
    let mut f = figure(theme, LIN);
    let ends = crate::ErrorRange::Numbers {
        low: std::sync::Arc::from([0.5, 1.5, 1.0]),
        high: std::sync::Arc::from([1.5, 2.5, 2.0]),
    };
    f.single_axes_mut()
        .scatter(
            nums(&[0.0, 1.0, 2.0]),
            &[1.0, 2.0, 1.5],
            ScatterOptions {
                connect: true,
                connect_width: width,
                y_error: y_error.then_some(ends),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    f
}

#[test]
fn connect_width_sets_the_joining_line_only() {
    let theme = Theme::pigment();
    let half = theme.drawn_line_width(SizeSpec::Auto) * 0.5;
    // Automatic: half the line width, as before.
    assert_eq!(stroke_widths(&connected(SizeSpec::Auto, false)), [half]);
    assert_eq!(
        stroke_widths(&connected(SizeSpec::Auto, true))[0],
        half,
        "the line comes first"
    );
    // A fixed width and a multiple of the line width.
    assert_eq!(
        stroke_widths(&connected(SizeSpec::pt(3.0).unwrap(), false)),
        [3.0]
    );
    assert_eq!(
        stroke_widths(&connected(SizeSpec::times(2.0).unwrap(), false)),
        [theme.drawn_line_width(SizeSpec::times(2.0).unwrap())]
    );
    // The error bars keep their own width.
    let widths = stroke_widths(&connected(SizeSpec::pt(3.0).unwrap(), true));
    assert_eq!(widths[0], 3.0);
    assert!(
        widths.len() > 1 && widths[1..].iter().all(|&w| w == half),
        "{widths:?}"
    );
}

#[test]
fn connect_width_without_connect_is_refused() {
    let mut f = figure(Theme::pigment(), LIN);
    let err = f
        .single_axes_mut()
        .scatter(
            nums(&[0.0, 1.0]),
            &[1.0, 2.0],
            ScatterOptions {
                connect_width: SizeSpec::pt(2.0).unwrap(),
                ..ScatterOptions::default()
            },
        )
        .unwrap_err();
    assert!(matches!(err, crate::Error::Usage { .. }), "{err}");
    assert_eq!(err.parameter(), Some("connect_width"));
    assert!(
        err.hint().is_some_and(|hint| hint.contains("connect=True")),
        "{err}"
    );
    assert!(f.single_axes_mut().marks().is_empty());
    // Automatic is the default and needs no line.
    f.single_axes_mut()
        .scatter(nums(&[0.0, 1.0]), &[1.0, 2.0], ScatterOptions::default())
        .unwrap();
}
