//! Output-gate cases for kernel density lines.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered: density lines with Scott's bandwidth and with a fixed one, over a density
//! histogram, in the three built-in themes; one line per group; a single value with a fixed
//! bandwidth; large values, and values spanning far less than the bandwidth; a narrow bandwidth
//! drawn through many points; direct labels
//! and explicit limits that cut the lines; and every error branch of `Axes::kde` and
//! `Axes::grouped_kde`, in the order the checks run, with the state a failed call leaves.

use umber_core::figure::{HistOptions, Series};
use umber_core::{
    Bandwidth, Bins, ColorSpec, Dash, FontLibrary, GroupedKdeOptions, HistStat, KdeOptions,
    LegendStyle, ScaleKind, SizeSpec, Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, lcg, linear, log, themes};

/// `n` values around `center`, roughly normal with a spread of about `spread`: the sum of six
/// uniform draws, scaled. Integer and basic float arithmetic only.
fn bell(n: usize, seed: u32, center: f64, spread: f64) -> Vec<f64> {
    let mut next = lcg(seed);
    (0..n)
        .map(|_| {
            let sum: f64 = (0..6).map(|_| next()).sum();
            center + 2.0 * spread * (sum - 3.0)
        })
        .collect()
}

/// Two bells, one at 20 and a smaller one at 45.
fn bimodal() -> Vec<f64> {
    let mut values = bell(400, 3, 20.0, 5.0);
    values.extend(bell(150, 5, 45.0, 4.0));
    values
}

fn kde(label: &str, bandwidth: Bandwidth) -> KdeOptions {
    KdeOptions {
        bandwidth,
        label: label.into(),
        ..KdeOptions::default()
    }
}

fn series(label: &str, values: Vec<f64>) -> Series {
    Series {
        label: label.into(),
        values,
        color: ColorSpec::AUTO,
    }
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let values = bimodal();
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Densities");
        axes(&mut f)
            .hist(
                &values,
                HistOptions {
                    bins: Bins::Count(25),
                    stat: HistStat::Density,
                    label: "Histogram".into(),
                    ..HistOptions::default()
                },
            )
            .unwrap()
            .kde(&values, kde("Scott", Bandwidth::Scott))
            .unwrap()
            .kde(
                &values,
                KdeOptions {
                    dash: Dash::Dashed,
                    ..kde("Narrow", Bandwidth::Fixed(1.0))
                },
            )
            .unwrap();
        emit_scene(
            &format!("fig-kde-{theme_name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "One line per group");
        axes(&mut f)
            .grouped_kde(
                vec![
                    series("North", bell(300, 7, 10.0, 3.0)),
                    series("South", bell(120, 11, 16.0, 2.0)),
                    series("East", bell(60, 13, 12.0, 6.0)),
                ],
                GroupedKdeOptions {
                    width: SizeSpec::times(1.2).unwrap(),
                    ..GroupedKdeOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("fig-kde-groups-{theme_name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }

    // One value with a fixed bandwidth: a single Gaussian.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "One value");
    axes(&mut f)
        .kde(&[3.0, f64::NAN], kde("Kernel", Bandwidth::Fixed(0.5)))
        .unwrap();
    emit_scene("fig-kde-one-value", &f.to_scene(lib).unwrap(), sink);

    // Large values, whose line stays within the supported magnitudes.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    axes(&mut f)
        .kde(&[-1e100, 0.0, 1e100], KdeOptions::default())
        .unwrap();
    emit_scene("fig-kde-large-values", &f.to_scene(lib).unwrap(), sink);

    // Values spanning far less than the bandwidth: one Gaussian.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f)
        .kde(
            &[0.0, 1e-310, 1e-308],
            kde("Tiny span", Bandwidth::Fixed(1.0)),
        )
        .unwrap();
    emit_scene("fig-kde-tiny-span", &f.to_scene(lib).unwrap(), sink);

    // A narrow bandwidth: the line takes as many points as its kernels need.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "Narrow kernels");
    axes(&mut f)
        .kde(&[0.0, 0.5, 1.0], kde("Narrow", Bandwidth::Fixed(1e-3)))
        .unwrap();
    emit_scene("fig-kde-narrow", &f.to_scene(lib).unwrap(), sink);

    // Direct labels, and limits that cut the lines on both sides.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    labelled(&mut f, "Cut lines");
    axes(&mut f)
        .legend(LegendStyle::Direct)
        .kde(&values, kde("Scott", Bandwidth::Scott))
        .unwrap()
        .kde(&values, kde("Wide", Bandwidth::Fixed(8.0)))
        .unwrap()
        .xlim(15.0, 40.0)
        .unwrap();
    emit_scene("fig-kde-cut", &f.to_scene(lib).unwrap(), sink);

    sink(Output {
        kind: "errors",
        file: "kde.txt".into(),
        bytes: errors().into_bytes(),
    });
}

/// Every error branch of `Axes::kde` and `Axes::grouped_kde`, in the order their checks run,
/// and the state after each call.
fn errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let mut rec = Log::default();
    let fixed = |h: f64| kde("", Bandwidth::Fixed(h));

    rec.section("Axes.kde, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "kde ok", |a| {
        a.kde(&[1.0, nan, 2.5, -0.0], kde("kept", Bandwidth::Scott))
            .map(drop)
    });
    for (name, h) in [
        ("zero bandwidth", 0.0),
        ("negative bandwidth", -1.0),
        ("NaN bandwidth", nan),
        ("infinite bandwidth", inf),
        ("huge bandwidth", 1e301),
    ] {
        rec.on_axes(&mut f, &format!("kde {name}"), |a| {
            a.kde(&[inf], fixed(h)).map(drop)
        });
    }
    for (name, values) in [
        ("infinite value", vec![1.0, inf]),
        ("huge value", vec![-1e301]),
        ("empty", vec![]),
        ("only NaN", vec![nan]),
        ("one distinct value", vec![2.0, 2.0, nan]),
    ] {
        rec.on_axes(&mut f, &format!("kde {name}"), |a| {
            a.kde(&values, KdeOptions::default()).map(drop)
        });
    }
    rec.on_axes(&mut f, "kde one value with a bandwidth", |a| {
        a.kde(&[2.0], fixed(1.0)).map(drop)
    });
    rec.on_axes(&mut f, "kde density beyond the magnitudes", |a| {
        a.kde(&[0.0], fixed(1e-305)).map(drop)
    });
    rec.on_axes(&mut f, "kde bandwidth too narrow to draw", |a| {
        a.kde(&[0.0, 0.5, 1.0], fixed(1e-4)).map(drop)
    });
    rec.on_axes(&mut f, "kde bandwidth narrow at the most points", |a| {
        a.kde(&[0.0, 1.0], fixed(4.0 / 16_000.0)).map(drop)
    });
    rec.on_axes(&mut f, "kde curve below the magnitude limit", |a| {
        a.kde(&[-1e300, 0.0], fixed(1e299)).map(drop)
    });
    rec.on_axes(&mut f, "kde curve above the magnitude limit", |a| {
        a.kde(&[1e300], fixed(1e290)).map(drop)
    });
    rec.on_axes(&mut f, "kde span far below the bandwidth", |a| {
        a.kde(&[0.0, 1e-310], fixed(1.0)).map(drop)
    });

    rec.section("Axes.grouped_kde, linear/linear");
    rec.on_axes(&mut f, "grouped_kde ok", |a| {
        a.grouped_kde(
            vec![series("a", vec![1.0, 2.0]), series("b", vec![3.0, 5.0])],
            GroupedKdeOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "grouped_kde no series", |a| {
        a.grouped_kde(Vec::new(), GroupedKdeOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "grouped_kde one distinct value in series 1", |a| {
        a.grouped_kde(
            vec![series("a", vec![1.0, 2.0]), series("b", vec![3.0])],
            GroupedKdeOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "grouped_kde infinite value in series 0", |a| {
        a.grouped_kde(
            vec![series("a", vec![inf]), series("b", vec![3.0])],
            GroupedKdeOptions {
                bandwidth: Bandwidth::Fixed(1.0),
                ..GroupedKdeOptions::default()
            },
        )
        .map(drop)
    });

    for (label, x, y) in [
        ("log/linear", log(10.0), linear()),
        ("time/linear", ScaleKind::Time, linear()),
        ("category/linear", ScaleKind::Category, linear()),
        ("linear/log", linear(), log(10.0)),
        ("linear/time", linear(), ScaleKind::Time),
        ("linear/category", linear(), ScaleKind::Category),
    ] {
        rec.section(&format!("Axes.kde, {label}"));
        let mut f = rec.new_figure(label, x, y);
        rec.on_axes(&mut f, "kde", |a| a.kde(&[1.0, 2.0], fixed(-1.0)).map(drop));
    }
    rec.out
}
