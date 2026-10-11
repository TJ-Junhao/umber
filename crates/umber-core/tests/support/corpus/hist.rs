//! Output-gate cases for histogram figures.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered: histograms in the three built-in themes; the gradient fill switched on and off
//! against each theme's default; 1, 30 and `MAX_BINS` bins; 200 000 values; constant data;
//! all-NaN and empty values; values at the largest magnitudes; a narrow x range that cuts bins
//! at both sides, and narrow limits far from huge values; a log y axis with empty bins and bins
//! of one value; every error branch of `Axes::hist`, including
//! which fault is reported when an input has several, and the state a failed call leaves; and
//! `MAX_BINS` in a constants file of its own.
//!
//! Marks carry no labels, so no legend is drawn.

use std::fmt::Write as _;

use umber_core::figure::HistOptions;
use umber_core::{ColorSpec, FillStyle, FontLibrary, MAX_BINS, Rgba, ScaleKind, Theme};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, lcg, linear, log, themes};

/// `MAX_BINS` as the type of `HistOptions::bins`.
fn max_bins() -> i64 {
    i64::try_from(MAX_BINS).expect("MAX_BINS fits in i64")
}

/// Options with `bins` bins, no label and the automatic color.
fn bins(bins: i64) -> HistOptions {
    HistOptions {
        bins,
        ..HistOptions::default()
    }
}

/// `n` values around 50, roughly normal: the sum of six uniform draws, scaled. Integer and
/// basic float arithmetic only, so the values are the same on every platform.
fn bell(n: usize, seed: u32) -> Vec<f64> {
    let mut next = lcg(seed);
    (0..n)
        .map(|_| {
            let sum: f64 = (0..6).map(|_| next()).sum();
            50.0 + 12.0 * (sum - 3.0)
        })
        .collect()
}

/// `n` values in `0..100` crowded towards 0, so bin counts span several decades, followed by
/// two outliers near 200, which leave the bins between them empty and hold one value each.
fn skewed(n: usize, seed: u32) -> Vec<f64> {
    let mut next = lcg(seed);
    let mut values: Vec<f64> = (0..n)
        .map(|_| {
            let u = next();
            100.0 * u * u * u * u
        })
        .collect();
    values.extend([187.5, 199.0]);
    values
}

/// `n` values with a triangular distribution over `20..80`, without noise: each value is the
/// sum of an even quantile and the bit-reversed index (the Hammersley point set), so every bin of
/// a fine histogram gets close to its expected share.
fn triangle(n: usize) -> Vec<f64> {
    (0..n)
        .map(|k| {
            let u = (k as f64 + 0.5) / n as f64;
            let v = f64::from((k as u32).reverse_bits()) / 4_294_967_296.0;
            20.0 + 30.0 * (u + v)
        })
        .collect()
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let values = bell(2000, 7);
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Histogram");
        axes(&mut f).hist(&values, HistOptions::default()).unwrap();
        emit_scene(&format!("fig-hist-{theme_name}"), &f.to_scene(lib), sink);
    }

    // The gradient switched on where the theme has none, and off where it has one.
    for (name, theme, fill) in [
        ("gradient-pigment", Theme::pigment(), FillStyle::Gradient),
        ("gradient-graphite", Theme::graphite(), FillStyle::Gradient),
        ("solid-midnight", Theme::midnight(), FillStyle::Solid),
    ] {
        let theme = theme.with_fill_style(fill).unwrap();
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Area fill");
        axes(&mut f).hist(&values, HistOptions::default()).unwrap();
        emit_scene(&format!("fig-hist-{name}"), &f.to_scene(lib), sink);
    }

    // One bin holds every value.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f).hist(&values, bins(1)).unwrap();
    emit_scene("fig-hist-one-bin", &f.to_scene(lib), sink);

    // The most bins, far narrower than the gap, which shrinks to half a bin; the gradient
    // anchors at the tallest bin.
    for (theme_name, theme) in [
        ("midnight", Theme::midnight()),
        ("graphite", Theme::graphite()),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Most bins");
        axes(&mut f)
            .hist(&triangle(60_000), bins(max_bins()))
            .unwrap();
        emit_scene(
            &format!("fig-hist-max-bins-{theme_name}"),
            &f.to_scene(lib),
            sink,
        );
    }

    // Many values, binned once per render.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    axes(&mut f)
        .hist(&bell(200_000, 3), HistOptions::default())
        .unwrap();
    emit_scene("fig-hist-200k-values", &f.to_scene(lib), sink);

    // Constant data: binned over v ± 0.5.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f).hist(&[7.0; 20], bins(5)).unwrap();
    emit_scene("fig-hist-constant", &f.to_scene(lib), sink);

    // No finite value: no bins, and the axes fall back to the unit range.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f)
        .hist(&[f64::NAN, f64::NAN], HistOptions::default())
        .unwrap();
    emit_scene("fig-hist-all-nan", &f.to_scene(lib), sink);
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut f).hist(&[], HistOptions::default()).unwrap();
    emit_scene("fig-hist-empty", &f.to_scene(lib), sink);

    // Values at the largest magnitudes, with NaN between them.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f)
        .hist(
            &[-1e300, -1e300, f64::NAN, 0.0, 2e299, 1e300, 1e300, 1e300],
            bins(10),
        )
        .unwrap();
    emit_scene("fig-hist-extreme-values", &f.to_scene(lib), sink);

    // A narrow x range: bins are cut at both sides of the plot, in axis space.
    for (theme_name, theme) in [
        ("midnight", Theme::midnight()),
        ("pigment", Theme::pigment()),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Narrow x range");
        axes(&mut f)
            .hist(&values, HistOptions::default())
            .unwrap()
            .xlim(41.3, 55.7)
            .unwrap();
        emit_scene(
            &format!("fig-hist-narrow-xlim-{theme_name}"),
            &f.to_scene(lib),
            sink,
        );
    }

    // A narrow range far from huge values on both axes: every count lies above the y limits.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f)
        .hist(&[-1e300, 0.0, 5e-11, 1e300], bins(3))
        .unwrap()
        .xlim(1e-11, 1e-10)
        .unwrap()
        .ylim(1e-11, 1e-10)
        .unwrap();
    emit_scene("fig-hist-huge-narrow-limits", &f.to_scene(lib), sink);

    // Log y: bins grow from the baseline, the y axis spans the positive counts, and
    // empty bins draw nothing.
    for (theme_name, theme) in [
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), log(10.0));
        labelled(&mut f, "Counts on a log axis");
        axes(&mut f)
            .hist(&skewed(5000, 5), HistOptions::default())
            .unwrap();
        emit_scene(
            &format!("fig-hist-log-y-{theme_name}"),
            &f.to_scene(lib),
            sink,
        );
    }

    sink(Output {
        kind: "errors",
        file: "hist.txt".into(),
        bytes: hist_errors().into_bytes(),
    });
    let mut constants = String::new();
    let _ = writeln!(constants, "MAX_BINS {MAX_BINS}");
    sink(Output {
        kind: "api",
        file: "constants-hist.txt".into(),
        bytes: constants.into_bytes(),
    });
}

/// Every error branch of `Axes::hist`, in the order its checks run, and the state after each
/// call.
fn hist_errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let mut rec = Log::default();

    rec.section("Axes.hist, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "hist ok", |a| {
        a.hist(&[1.0, -2.5, nan], HistOptions::default()).map(drop)
    });
    // Non-canonical NaN payloads and signed zeros are stored bit for bit.
    rec.on_axes(
        &mut f,
        "hist NaN and signed zeros, labelled and colored",
        |a| {
            a.hist(
                &[
                    f64::from_bits(0x7ff8_0000_0000_0001),
                    -0.0,
                    0.0,
                    f64::from_bits(0xfff0_0000_dead_beef),
                ],
                HistOptions {
                    bins: 3,
                    label: "kept".into(),
                    color: ColorSpec::Fixed(Rgba::opaque(1, 2, 3)),
                },
            )
            .map(drop)
        },
    );
    rec.on_axes(&mut f, "hist empty", |a| {
        a.hist(&[], HistOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "hist one bin", |a| {
        a.hist(&[1.0], bins(1)).map(drop)
    });
    rec.on_axes(&mut f, "hist MAX_BINS bins", |a| {
        a.hist(&[1.0], bins(max_bins())).map(drop)
    });
    let cases: Vec<(&str, Vec<f64>, i64)> = vec![
        ("zero bins", vec![1.0], 0),
        ("MAX_BINS + 1 bins", vec![1.0], max_bins() + 1),
        ("i64::MAX bins", vec![1.0], i64::MAX),
        ("zero bins before an infinite value", vec![inf], 0),
        ("infinite value", vec![1.0, nan, inf], 30),
        ("negative infinite value", vec![f64::NEG_INFINITY], 30),
        ("huge value", vec![1.0e301], 30),
        ("huge negative value", vec![-1.0e301], 30),
        (
            "huge value[1] before infinite value[2]",
            vec![nan, 1.0e301, inf],
            30,
        ),
        ("values at the bound", vec![1e300, -1e300], 30),
    ];
    for (name, values, count) in cases {
        rec.on_axes(&mut f, &format!("hist {name}"), |a| {
            a.hist(&values, bins(count)).map(drop)
        });
    }

    rec.section("Axes.hist, category/linear");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    rec.on_axes(&mut f, "hist on a category x axis", |a| {
        a.hist(&[inf], HistOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "hist on a category x axis, zero bins", |a| {
        a.hist(&[1.0], bins(0)).map(drop)
    });

    rec.section("Axes.hist, log/linear");
    let mut f = rec.new_figure("log/linear", log(10.0), linear());
    rec.on_axes(&mut f, "hist on a log x axis", |a| {
        a.hist(&[1.0, 10.0], HistOptions::default()).map(drop)
    });
    rec.on_axes(
        &mut f,
        "hist on a log x axis, non-positive and infinite values",
        |a| a.hist(&[-1.0, 0.0, inf], HistOptions::default()).map(drop),
    );
    rec.on_axes(&mut f, "hist on a log x axis, too many bins", |a| {
        a.hist(&[1.0], bins(max_bins() + 1)).map(drop)
    });

    rec.section("Axes.hist, linear/log");
    let mut f = rec.new_figure("linear/log", linear(), log(10.0));
    rec.on_axes(&mut f, "hist ok with non-positive values", |a| {
        a.hist(&[-3.0, 0.0, 2.0], HistOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "hist infinite value", |a| {
        a.hist(&[inf], HistOptions::default()).map(drop)
    });

    rec.section("Axes.hist, linear/category");
    let mut f = rec.new_figure("linear/category", linear(), ScaleKind::Category);
    rec.on_axes(&mut f, "hist on a category y axis", |a| {
        a.hist(&[1.0], HistOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "hist on a category y axis, infinite value", |a| {
        a.hist(&[inf], HistOptions::default()).map(drop)
    });
    rec.out
}
