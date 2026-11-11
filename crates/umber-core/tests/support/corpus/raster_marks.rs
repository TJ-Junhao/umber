//! Marks drawn as one image in vector output, in every theme: a dense scatter between a vector
//! band under it and a vector line over it, with a legend; a long noisy line beside a vector
//! one; a scatter with its own line, error bars and value labels; a color-mapped scatter with a
//! colorbar; and a tripcolor with blank triangles and a colorbar. Also the errors of
//! `raster_dpi` and of an image too large to draw.

use std::fmt::Write as _;
use std::sync::Arc;

use umber_core::figure::{AxisData, BandOptions, LineOptions, ScatterOptions};
use umber_core::{
    ColorBy, ColorbarOptions, ErrorRange, Figure, FontLibrary, MarkRender, Place, PointColor,
    PointLabels, ScaleKind, SvgText, Theme, TriValues, Triangles, TripcolorOptions, pdf_pages,
};

use super::super::{Output, emit_scene, lcg, record, themes};

const RASTER: MarkRender = MarkRender::Raster;

/// `count` points spread over `0..10` by `0..10`, the same every time.
fn points(count: usize, seed: u32) -> (Vec<f64>, Vec<f64>) {
    let mut next = lcg(seed);
    (0..count).map(|_| (10.0 * next(), 10.0 * next())).unzip()
}

fn figure(theme: Theme) -> Figure {
    Figure::new(6.0, 4.0, theme).unwrap()
}

/// (a) A dense scatter drawn as an image between a vector band and a vector line.
fn layered(theme: Theme) -> Figure {
    let (x, y) = points(1500, 3);
    let mut f = figure(theme);
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("A dense scatter drawn as an image")
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .band(
            &[0.0, 10.0],
            &[3.0, 4.0],
            &[6.0, 8.0],
            BandOptions {
                label: "Band".into(),
                ..BandOptions::default()
            },
        )
        .unwrap()
        .scatter(
            AxisData::Numbers(x),
            &y,
            ScatterOptions {
                label: "Points".into(),
                render: RASTER,
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 10.0]),
            &[9.0, 1.0],
            LineOptions {
                label: "Trend".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    f
}

/// (b) A long noisy line drawn as an image beside a vector line.
fn noisy(theme: Theme) -> Figure {
    let mut next = lcg(5);
    let x: Vec<f64> = (0..3000).map(|i| f64::from(i) / 300.0).collect();
    let y: Vec<f64> = x
        .iter()
        .map(|&t| (t * 1.7).sin() * 2.0 + 3.0 * next())
        .collect();
    let mut f = figure(theme);
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("A noisy line drawn as an image")
        .line(
            AxisData::Numbers(x.clone()),
            &y,
            LineOptions {
                label: "Signal".into(),
                render: RASTER,
                ..LineOptions::default()
            },
        )
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 10.0]),
            &[1.5, 1.5],
            LineOptions {
                label: "Mean".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    f
}

/// (c) A scatter drawn as images with its own line, error bars and value labels, which stay
/// vector between and over them.
fn dressed(theme: Theme) -> Figure {
    let mut f = figure(theme);
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Error bars and labels stay vector")
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(
            AxisData::Numbers(vec![1.0, 2.0, 3.0, 4.0, 5.0]),
            &[2.5, 4.0, 5.0, 5.5, 6.5],
            ScatterOptions {
                label: "Folds".into(),
                labels: PointLabels {
                    annotate: true,
                    place: Place::Right,
                    ..PointLabels::default()
                },
                y_error: Some(ErrorRange::Numbers {
                    low: Arc::from([2.0, 3.0, 4.5, 4.0, 6.0]),
                    high: Arc::from([3.0, 4.5, 5.5, 6.5, 7.0]),
                }),
                connect: true,
                render: RASTER,
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    f
}

/// (d) A color-mapped scatter drawn as an image, with a vector colorbar.
fn mapped(theme: Theme) -> Figure {
    let (x, y) = points(1200, 9);
    let values: Vec<f64> = x.iter().zip(&y).map(|(a, b)| (a - b) * a).collect();
    let mut f = figure(theme);
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Colors by value")
        .scatter(
            AxisData::Numbers(x),
            &y,
            ScatterOptions {
                color: PointColor::By(ColorBy::new(values)),
                render: RASTER,
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "Value".into(),
            ..ColorbarOptions::default()
        })
        .unwrap();
    f
}

/// (e) A tripcolor drawn as an image, some triangles blank, with a vector colorbar.
fn mesh(theme: Theme) -> Figure {
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for j in 0..12_u32 {
        for i in 0..12_u32 {
            let shift = if j % 2 == 0 { 0.0 } else { 0.4 };
            x.push(f64::from(i) * 0.8 + shift);
            y.push(f64::from(j) * 0.7);
        }
    }
    let mut triangles = Vec::new();
    let mut values = Vec::new();
    for j in 0..11_i64 {
        for i in 0..11_i64 {
            let corner = j * 12 + i;
            triangles.push([corner, corner + 1, corner + 13]);
            triangles.push([corner, corner + 13, corner + 12]);
            let blank = (4..7).contains(&i) && (4..6).contains(&j);
            for k in 0..2 {
                let value = ((i * 3 + j * 5 + k) % 17) as f64;
                values.push(if blank { f64::NAN } else { value });
            }
        }
    }
    let mut f = Figure::new(5.0, 5.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Triangles drawn as an image")
        .tripcolor(
            &x,
            &y,
            Triangles::Given(&triangles),
            &values,
            TripcolorOptions {
                on: TriValues::Triangles,
                render: RASTER,
                ..TripcolorOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions::default())
        .unwrap();
    f
}

/// Emits the five figures in every theme, then the errors.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (case, figure) in [
            ("scatter", layered(theme.clone())),
            ("line", noisy(theme.clone())),
            ("dressed", dressed(theme.clone())),
            ("color-by", mapped(theme.clone())),
            ("tripcolor", mesh(theme.clone())),
        ] {
            emit_scene(
                &format!("raster-marks-{case}-{name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
    let raster = layered(Theme::pigment());
    let mut out = String::new();
    for dpi in [9, 2401, -1] {
        for (label, err) in [
            ("to_svg", raster.to_svg(SvgText::Outlines, dpi).unwrap_err()),
            ("to_pdf", raster.to_pdf(dpi).unwrap_err()),
            ("pdf_pages", pdf_pages(&[&raster], dpi).unwrap_err()),
        ] {
            let _ = writeln!(
                out,
                "{label} raster_dpi {dpi}: {}\nparameter={:?} hint={:?}",
                record::error(&err),
                err.parameter(),
                err.hint(),
            );
        }
    }
    // The plot of a 6 by 4 in figure at 2,400 dpi is over 2^26 pixels.
    for (label, err) in [
        (
            "to_svg",
            raster.to_svg(SvgText::Outlines, 2400).unwrap_err(),
        ),
        ("to_pdf", raster.to_pdf(2400).unwrap_err()),
        (
            "pdf_pages",
            pdf_pages(&[&figure(Theme::pigment()), &raster], 2400).unwrap_err(),
        ),
    ] {
        let _ = writeln!(
            out,
            "{label} too large: {}\nparameter={:?} hint={:?}",
            record::error(&err),
            err.parameter(),
            err.hint(),
        );
    }
    sink(Output {
        kind: "errors",
        file: "raster-marks.txt".into(),
        bytes: out.into_bytes(),
    });
}
