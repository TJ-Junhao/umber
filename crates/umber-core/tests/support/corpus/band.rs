//! Output-gate cases for bands.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Scenes, in the three built-in themes: a mean line over its 95% band, and a band with NaN
//! gaps in x, in low and in high. In single themes: a band under each legend style (so with the
//! band in the legend row and the line labelled directly in Graphite); explicit limits that cut
//! a band at its sides, above and below, and that sit exactly on its ends; a log y axis; a band
//! beside a scatter, which pads the axes, and beside bars on a grid; a band whose x is not
//! sorted; and 200 000 points, reduced per pixel column.
//!
//! In `errors/band.txt`: every error branch of `Axes::band` in the order its checks run, which
//! fault is reported when an input has several, and the state a failed call leaves.

use umber_core::figure::{AxisData, BandOptions, LineOptions, ScatterOptions};
use umber_core::{
    Alpha, ColorSpec, FontLibrary, Grid, GridIndex, LegendStyle, Placement, ScaleKind, Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, linear, log, themes, wobble};
use umber_core::Figure;

/// A named input of `band`: its x, low and high values.
type Case = (&'static str, Vec<f64>, Vec<f64>, Vec<f64>);

fn palette(index: usize) -> ColorSpec {
    ColorSpec::Palette {
        index,
        alpha: Alpha::new(1.0).unwrap(),
    }
}

fn labelled_band(label: &str, index: usize) -> BandOptions {
    BandOptions {
        label: label.into(),
        color: palette(index),
    }
}

fn labelled_line(label: &str, index: usize) -> LineOptions {
    LineOptions {
        label: label.into(),
        color: palette(index),
        ..LineOptions::default()
    }
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "band.txt".into(),
        bytes: band_errors().into_bytes(),
    });
}

/// A mean of 97 points over 0..=12 and the half width of its interval.
fn estimate() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..97).map(|i| 12.0 * f64::from(i) / 96.0).collect();
    let mean = x
        .iter()
        .map(|x| 10.0 + 3.0 * (x / 2.0).sin() + 0.4 * x)
        .collect();
    let half = x
        .iter()
        .map(|x| 1.2 + 0.8 * (x / 12.0) + 0.3 * x.sin().abs())
        .collect();
    (x, mean, half)
}

/// A figure holding the mean line and its band, labelled, in `theme`.
fn mean_figure(theme: Theme, title: &str, gaps: bool) -> Figure {
    let (x, mean, half) = estimate();
    let mut low: Vec<f64> = mean.iter().zip(&half).map(|(m, h)| m - h).collect();
    let mut high: Vec<f64> = mean.iter().zip(&half).map(|(m, h)| m + h).collect();
    let mut xs = x.clone();
    if gaps {
        // One gap in each of the three arrays, at different places.
        xs[20] = f64::NAN;
        low[50] = f64::NAN;
        high[75] = f64::NAN;
    }
    let mut f = figure(6.0, 4.0, theme, linear(), linear());
    labelled(&mut f, title);
    axes(&mut f)
        .band(&xs, &low, &high, labelled_band("95% interval", 0))
        .unwrap()
        .line(AxisData::Numbers(x), &mean, labelled_line("Mean", 0))
        .unwrap();
    f
}

/// Every band figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();

    // A mean over its band, and the same with gaps in x, low and high, in every theme.
    for (theme_name, theme) in themes() {
        out.push((
            format!("band-mean-{theme_name}"),
            mean_figure(theme.clone(), "Mean with a 95% band", false),
        ));
        out.push((
            format!("band-gaps-{theme_name}"),
            mean_figure(theme, "Gaps in x, low and high", true),
        ));
    }

    // Each legend style: the band is in the legend row under all of them.
    for (style_name, style) in [
        ("none", LegendStyle::None),
        ("header", LegendStyle::Header),
        ("chips", LegendStyle::Chips),
        ("direct", LegendStyle::Direct),
    ] {
        let mut f = mean_figure(Theme::graphite(), "Legend styles", false);
        axes(&mut f).legend(style, Default::default()).unwrap();
        out.push((format!("band-legend-{style_name}"), f));
    }

    // Explicit limits: a band that crosses the left and right limits and the top and bottom.
    for (theme_name, theme) in [
        ("pigment", Theme::pigment()),
        ("midnight", Theme::midnight()),
    ] {
        let (x, mean, half) = estimate();
        let low: Vec<f64> = mean.iter().zip(&half).map(|(m, h)| m - 2.0 * h).collect();
        let high: Vec<f64> = mean.iter().zip(&half).map(|(m, h)| m + 2.0 * h).collect();
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Explicit limits");
        axes(&mut f)
            .band(&x, &low, &high, labelled_band("Wide band", 0))
            .unwrap()
            .xlim(3.0, 9.0)
            .unwrap()
            .ylim(9.0, 13.0)
            .unwrap();
        out.push((format!("band-limits-{theme_name}"), f));
    }

    // Limits exactly on the ends of the band, and on its curves.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "Limits on the ends");
    axes(&mut f)
        .band(
            &[1.0, 2.0, 3.0, 4.0],
            &[1.0, 2.0, 1.0, 2.0],
            &[3.0, 4.0, 3.0, 4.0],
            labelled_band("Band", 0),
        )
        .unwrap()
        .xlim(1.0, 4.0)
        .unwrap()
        .ylim(2.0, 3.0)
        .unwrap();
    out.push(("band-limits-exact".to_owned(), f));

    // A band entirely outside the limits, beside one that is inside.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    labelled(&mut f, "Outside the limits");
    axes(&mut f)
        .band(
            &[0.0, 1.0, 2.0],
            &[8.0; 3],
            &[9.0; 3],
            labelled_band("Above", 0),
        )
        .unwrap()
        .band(
            &[0.0, 1.0, 2.0],
            &[1.0; 3],
            &[3.0; 3],
            labelled_band("Inside", 1),
        )
        .unwrap()
        .band(&[5.0, 6.0], &[1.0; 2], &[3.0; 2], labelled_band("Right", 2))
        .unwrap()
        .xlim(0.0, 2.0)
        .unwrap()
        .ylim(0.0, 4.0)
        .unwrap();
    out.push(("band-outside-limits".to_owned(), f));

    // A log y axis: low and high are placed in log space.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), log(10.0));
    labelled(&mut f, "Log y");
    let x: Vec<f64> = (0..40).map(f64::from).collect();
    let center: Vec<f64> = x.iter().map(|x| 10.0_f64.powf(0.05 * x + 1.0)).collect();
    let low: Vec<f64> = center.iter().map(|c| c / 2.0).collect();
    let high: Vec<f64> = center.iter().map(|c| c * 3.0).collect();
    axes(&mut f)
        .band(&x, &low, &high, labelled_band("Range", 0))
        .unwrap()
        .line(AxisData::Numbers(x), &center, labelled_line("Center", 0))
        .unwrap();
    out.push(("band-log-y".to_owned(), f));

    // A log x axis.
    let mut f = figure(6.0, 4.0, Theme::midnight(), log(10.0), linear());
    labelled(&mut f, "Log x");
    axes(&mut f)
        .band(
            &[1.0, 10.0, 100.0, 1000.0],
            &[1.0, 2.0, 1.5, 3.0],
            &[2.0, 4.0, 3.5, 5.0],
            labelled_band("Tiers", 0),
        )
        .unwrap();
    out.push(("band-log-x".to_owned(), f));

    // Beside a scatter, which pads the axes, and two bands in palette order.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    labelled(&mut f, "Bands and points");
    let (x, y) = wobble(30, 91);
    let y: Vec<f64> = y.iter().map(|v| v + 6.0).collect();
    axes(&mut f)
        .band(
            &x,
            &y.iter().map(|v| v - 1.0).collect::<Vec<_>>(),
            &y.iter().map(|v| v + 1.0).collect::<Vec<_>>(),
            BandOptions {
                label: "Inner".into(),
                ..BandOptions::default()
            },
        )
        .unwrap()
        .band(
            &x,
            &y.iter().map(|v| v - 2.5).collect::<Vec<_>>(),
            &y.iter().map(|v| v + 2.5).collect::<Vec<_>>(),
            BandOptions {
                label: "Outer".into(),
                ..BandOptions::default()
            },
        )
        .unwrap()
        .scatter(
            AxisData::Numbers(x),
            &y,
            ScatterOptions {
                label: "Observed".into(),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    out.push(("band-with-scatter".to_owned(), f));

    // Two panels of a grid.
    let grid = Grid::new(1, 2).unwrap();
    let (width, height) = grid.auto_size_in();
    let mut f = Figure::with_grid(width, height, Theme::graphite(), grid).unwrap();
    for (col, (title, offset)) in [("Narrow", 0.5), ("Wide", 2.0)].into_iter().enumerate() {
        let at = Placement {
            row: GridIndex::At(0),
            col: GridIndex::At(col as i64),
            row_span: 1,
            col_span: 1,
        };
        let (x, mean, _) = estimate();
        let low: Vec<f64> = mean.iter().map(|m| m - offset).collect();
        let high: Vec<f64> = mean.iter().map(|m| m + offset).collect();
        f.add_axes_at(at, linear(), linear())
            .unwrap()
            .title(title)
            .band(&x, &low, &high, labelled_band("Band", 0))
            .unwrap()
            .line(AxisData::Numbers(x), &mean, labelled_line("Mean", 0))
            .unwrap();
    }
    out.push(("band-grid".to_owned(), f));

    // Many points: reduced per pixel column, keeping the envelope and the gap.
    let n = 200_000;
    let (x, y) = wobble(n, 101);
    let mut low: Vec<f64> = y.iter().map(|v| v - 1.0).collect();
    let mut high: Vec<f64> = y.iter().map(|v| v + 1.0).collect();
    low[40_000] = y[40_000] - 6.0;
    high[120_000] = y[120_000] + 6.0;
    high[90_000] = f64::NAN;
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f)
        .band(&x, &low, &high, BandOptions::default())
        .unwrap();
    out.push(("band-200k-points".to_owned(), f));

    out
}

/// Every error branch of `Axes::band`, in the order its checks run, and the state after each
/// call.
fn band_errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let band = BandOptions::default;
    let mut rec = Log::default();

    rec.section("Axes.band, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "band ok", |a| {
        a.band(&[0.0, 1.0, 2.0], &[1.0, 2.0, 1.0], &[3.0, 4.0, 3.0], band())
            .map(drop)
    });
    let cases: Vec<Case> = vec![
        (
            "x longer than low",
            vec![0.0, 1.0],
            vec![0.0],
            vec![1.0, 2.0],
        ),
        (
            "x longer than high",
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            vec![2.0],
        ),
        (
            "x shorter than both",
            vec![0.0],
            vec![0.0, 1.0],
            vec![2.0, 3.0],
        ),
        ("length mismatch and infinite x", vec![inf], vec![], vec![]),
        ("infinite x", vec![0.0, inf], vec![0.0, 1.0], vec![2.0, 3.0]),
        (
            "negative infinite low",
            vec![0.0, 1.0],
            vec![f64::NEG_INFINITY, 1.0],
            vec![2.0, 3.0],
        ),
        ("infinite high", vec![0.0], vec![0.0], vec![inf]),
        ("huge x", vec![1.0e301], vec![0.0], vec![1.0]),
        ("huge low", vec![0.0], vec![-1.0e301], vec![1.0]),
        ("huge high", vec![0.0], vec![0.0], vec![1.0e301]),
        (
            "infinite low and infinite x",
            vec![inf],
            vec![inf],
            vec![0.0],
        ),
        (
            "infinite low and infinite high",
            vec![0.0],
            vec![inf],
            vec![inf],
        ),
        (
            "swapped and infinite high",
            vec![0.0, 1.0],
            vec![5.0, 5.0],
            vec![1.0, inf],
        ),
        ("low above high", vec![0.0], vec![2.0], vec![1.0]),
        (
            "first of two swapped",
            vec![0.0, 1.0, 2.0],
            vec![0.0, 5.0, 9.0],
            vec![1.0, 4.0, 2.0],
        ),
        (
            "swapped by one ulp",
            vec![0.0],
            vec![1.0 + f64::EPSILON],
            vec![1.0],
        ),
        (
            "swapped after gaps",
            vec![0.0, 1.0, 2.0, 3.0],
            vec![nan, 3.0, 9.0, 8.0],
            vec![0.0, nan, 2.0, 1.0],
        ),
        (
            "unsorted x",
            vec![0.0, 3.0, 2.0],
            vec![0.0; 3],
            vec![1.0; 3],
        ),
        (
            "unsorted x after a gap",
            vec![0.0, nan, -1.5],
            vec![0.0; 3],
            vec![1.0; 3],
        ),
        (
            "unsorted x and swapped",
            vec![1.0, 0.0],
            vec![5.0; 2],
            vec![1.0; 2],
        ),
        (
            "unsorted x and infinite high",
            vec![1.0, 0.0],
            vec![0.0; 2],
            vec![1.0, inf],
        ),
        ("swapped, large", vec![0.0], vec![1.0e7], vec![1.0e6]),
    ];
    for (name, x, low, high) in cases {
        rec.on_axes(&mut f, &format!("band {name}"), |a| {
            a.band(&x, &low, &high, band()).map(drop)
        });
    }
    // Equal curves, gaps in each array, empty data, with non-canonical NaN payloads and signed
    // zeros: storage keeps every bit.
    let p0 = f64::from_bits(0x7ff8_0000_0000_0001);
    let p1 = f64::from_bits(0xfff0_0000_dead_beef);
    rec.on_axes(&mut f, "band with equal curves", |a| {
        a.band(&[0.0, 1.0], &[2.0, 2.0], &[2.0, 2.0], band())
            .map(drop)
    });
    rec.on_axes(&mut f, "band with NaN payloads and signed zeros", |a| {
        a.band(
            &[-0.0, 0.0, p0, 2.0],
            &[p1, -0.0, 1.0, 0.0],
            &[1.0, 0.0, 2.0, p0],
            band(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "band all NaN", |a| {
        a.band(&[nan, nan], &[nan, nan], &[nan, nan], band())
            .map(drop)
    });
    rec.on_axes(&mut f, "band empty", |a| {
        a.band(&[], &[], &[], band()).map(drop)
    });
    rec.on_axes(&mut f, "band with options", |a| {
        a.band(
            &[umber_core::MAX_MAGNITUDE, umber_core::MAX_MAGNITUDE],
            &[-umber_core::MAX_MAGNITUDE, 0.0],
            &[0.0, umber_core::MAX_MAGNITUDE],
            BandOptions {
                label: "edge".into(),
                color: ColorSpec::Palette {
                    index: 21,
                    alpha: Alpha::new(0.25).unwrap(),
                },
            },
        )
        .map(drop)
    });

    rec.section("Axes.band, category/linear");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    for (name, x, low, high) in [
        ("category x", vec![0.0, 1.0], vec![0.0, 1.0], vec![1.0, 2.0]),
        ("category x, empty", vec![], vec![], vec![]),
        ("category x, length mismatch", vec![0.0], vec![], vec![]),
        ("category x, swapped", vec![0.0], vec![2.0], vec![1.0]),
    ] {
        rec.on_axes(&mut f, &format!("band {name}"), |a| {
            a.band(&x, &low, &high, band()).map(drop)
        });
    }

    rec.section("Axes.band, linear/log10");
    let mut f = rec.new_figure("linear/log10", linear(), log(10.0));
    for (name, x, low, high) in [
        ("zero low", vec![0.0, 1.0], vec![1.0, 0.0], vec![2.0, 3.0]),
        ("negative high", vec![0.0], vec![1.0], vec![-1.0]),
        ("positive", vec![0.0, 1.0], vec![1.0, 2.0], vec![3.0, 4.0]),
        ("NaN low", vec![0.0, 1.0], vec![nan, 2.0], vec![3.0, 4.0]),
        ("swapped", vec![0.0], vec![4.0], vec![3.0]),
        ("negative x is fine", vec![-5.0], vec![1.0], vec![2.0]),
    ] {
        rec.on_axes(&mut f, &format!("band {name}"), |a| {
            a.band(&x, &low, &high, band()).map(drop)
        });
    }

    rec.section("Axes.band, log2/linear");
    let mut f = rec.new_figure("log2/linear", log(2.0), linear());
    for (name, x, low, high) in [
        ("zero x", vec![0.0], vec![1.0], vec![2.0]),
        ("negative x", vec![-1.0], vec![1.0], vec![2.0]),
        ("positive x", vec![2.0, 4.0], vec![1.0, 2.0], vec![3.0, 4.0]),
        ("zero y is fine", vec![2.0], vec![0.0], vec![0.0]),
    ] {
        rec.on_axes(&mut f, &format!("band {name}"), |a| {
            a.band(&x, &low, &high, band()).map(drop)
        });
    }
    rec.out
}
