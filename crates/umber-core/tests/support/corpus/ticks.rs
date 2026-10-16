//! Output-gate cases for explicit ticks and tick label formats.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered, in every theme: explicit x and y tick positions at irregular steps; ticks with
//! their own labels on both axes; each tick format (automatic with an affix, fixed, percent and
//! scientific) on a linear and on a log y axis, applied to the automatic ticks and, on x, to
//! explicit positions; a percent axis with a suffix; ticks that extend the domain beyond the
//! data on both axes, with bars that keep 0; ticks clipped by explicit limits; and empty tick
//! lists, which leave the axes without ticks and gridlines. Once each: tiny and sparse explicit
//! positions, which are labelled as given and never snapped to zero, and explicit ticks over
//! bars without a finite height, which keep 0 in the domain. Also: every error branch of
//! `TickFormat::fixed`, `TickFormat::percent`, `TickFormat::affix` and the six tick setters of
//! `Axes`, including which fault is reported when an input has several, both orders of the
//! conflict between given labels and a format, the replacement of one tick setting by the next,
//! and the state each call leaves.

use std::fmt::Write as _;

use umber_core::figure::{BarOptions, LineOptions, ScatterOptions};
use umber_core::{
    AxesMut, ExplicitTicks, Figure, FontLibrary, GridIndex, NumberStyle, ScaleKind, TickFormat,
};

use super::super::{
    Log, Output, axes, emit_scene, figure, labelled, linear, log, numbers, record, themes, wobble,
};

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "ticks.txt".into(),
        bytes: tick_errors().into_bytes(),
    });
}

fn strings(labels: &[&str]) -> Vec<String> {
    labels.iter().map(|s| (*s).to_owned()).collect()
}

fn line(label: &str) -> LineOptions {
    LineOptions {
        label: label.into(),
        ..LineOptions::default()
    }
}

/// Two labelled lines over `0..=n - 1` from the deterministic wobble, scaled by `scale` and
/// lifted by `lift`.
fn two_lines(a: &mut AxesMut<'_>, n: usize, scale: f64, lift: f64) {
    for (seed, label) in [(71, "North"), (72, "South")] {
        let (x, y) = wobble(n, seed);
        let y: Vec<f64> = y.iter().map(|v| v * scale + lift).collect();
        a.line(numbers(x), &y, line(label)).unwrap();
    }
}

/// A rising series on a log y axis from about 2 to about 3000, as two lines.
fn log_lines(a: &mut AxesMut<'_>) {
    let x: Vec<f64> = (0..24).map(f64::from).collect();
    let north: Vec<f64> = x.iter().map(|v| 2.0 * 1.35_f64.powf(*v)).collect();
    let south: Vec<f64> = x.iter().map(|v| 5.0 * 1.25_f64.powf(*v)).collect();
    a.line(numbers(x.clone()), &north, line("North"))
        .unwrap()
        .line(numbers(x), &south, line("South"))
        .unwrap();
}

/// The formats each applied to a linear and a log y axis, by name.
fn formats() -> Vec<(&'static str, TickFormat)> {
    vec![
        ("auto-affix", TickFormat::AUTO.affix("~", " u").unwrap()),
        ("fixed", TickFormat::fixed(2).unwrap()),
        ("percent", TickFormat::percent(1).unwrap()),
        ("scientific", TickFormat::scientific()),
    ]
}

/// Every tick figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();
    for (theme_name, theme) in themes() {
        // Explicit positions on both axes, at irregular steps, labelled by the automatic format.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Explicit ticks");
        let mut a = axes(&mut f);
        two_lines(&mut a, 40, 3.0, 20.0);
        a.xticks(&[0.0, 7.0, 14.0, 30.0, 39.0])
            .unwrap()
            .yticks(&[5.0, 12.5, 20.0, 27.5, 35.0])
            .unwrap();
        out.push((format!("fig-ticks-positions-{theme_name}"), f));

        // Ticks with their own labels, drawn as given, on both axes.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Labelled ticks");
        let mut a = axes(&mut f);
        two_lines(&mut a, 48, 2.0, 10.0);
        a.xticks_labeled(
            &[0.0, 12.0, 24.0, 36.0, 47.0],
            strings(&[
                "Mon 00:00",
                "Mon 12:00",
                "Tue 00:00",
                "Tue 12:00",
                "Tue 23:00",
            ]),
        )
        .unwrap()
        .yticks_labeled(
            &[0.0, 10.0, 20.0],
            strings(&["low", "normal", "\u{2212}alert\u{2212}"]),
        )
        .unwrap();
        out.push((format!("fig-ticks-labeled-{theme_name}"), f));

        // Each format on the automatic ticks of a linear and of a log y axis; on x, applied to
        // explicit positions.
        for (format_name, format) in formats() {
            let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
            labelled(&mut f, &format!("Format {format_name}, linear"));
            let mut a = axes(&mut f);
            two_lines(&mut a, 30, 0.08, 0.4);
            a.ytick_format(format.clone())
                .unwrap()
                .xticks(&[0.0, 2.5, 10.0, 20.0, 29.0])
                .unwrap()
                .xtick_format(format.clone())
                .unwrap();
            out.push((format!("fig-ticks-{format_name}-linear-{theme_name}"), f));

            let mut f = figure(6.0, 4.0, theme.clone(), linear(), log(10.0));
            labelled(&mut f, &format!("Format {format_name}, log"));
            let mut a = axes(&mut f);
            log_lines(&mut a);
            a.ytick_format(format).unwrap();
            out.push((format!("fig-ticks-{format_name}-log-{theme_name}"), f));
        }

        // A percent axis with a suffix, on explicit y positions over bars.
        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        labelled(&mut f, "Share with suffix");
        axes(&mut f)
            .bar(
                strings(&["North", "East", "South", "West"]),
                &[0.42, 0.18, 0.27, 0.13],
                BarOptions::default(),
            )
            .unwrap()
            .yticks(&[0.0, 0.1, 0.2, 0.3, 0.4, 0.5])
            .unwrap()
            .ytick_format(
                TickFormat::percent(0)
                    .unwrap()
                    .affix("", " of sales")
                    .unwrap(),
            )
            .unwrap();
        out.push((format!("fig-ticks-percent-suffix-{theme_name}"), f));

        // Ticks beyond the data widen the domain on both axes; bars still keep 0.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Ticks extend the domain");
        let (x, y) = wobble(20, 73);
        let y: Vec<f64> = y.iter().map(|v| v + 4.0).collect();
        axes(&mut f)
            .scatter(numbers(x), &y, ScatterOptions::default())
            .unwrap()
            .xticks(&[-10.0, 0.0, 10.0, 20.0, 30.0])
            .unwrap()
            .yticks(&[-2.0, 0.0, 2.0, 4.0, 6.0, 8.0, 10.0])
            .unwrap();
        out.push((format!("fig-ticks-extend-{theme_name}"), f));

        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        labelled(&mut f, "Ticks extend bars");
        axes(&mut f)
            .bar(
                strings(&["a", "b", "c"]),
                &[3.0, 5.5, 4.0],
                BarOptions::default(),
            )
            .unwrap()
            .yticks(&[2.0, 4.0, 6.0, 8.0])
            .unwrap();
        out.push((format!("fig-ticks-extend-bars-{theme_name}"), f));

        // Explicit limits clip the ticks outside them, on a linear x and a log y axis.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), log(10.0));
        labelled(&mut f, "Ticks clipped by limits");
        let mut a = axes(&mut f);
        log_lines(&mut a);
        a.xlim(4.0, 18.0)
            .unwrap()
            .ylim(10.0, 500.0)
            .unwrap()
            .xticks(&[0.0, 4.0, 8.0, 12.0, 16.0, 20.0])
            .unwrap()
            .yticks_labeled(
                &[1.0, 10.0, 30.0, 100.0, 300.0, 1000.0],
                strings(&["1", "ten", "30", "hundred", "300", "1000"]),
            )
            .unwrap();
        out.push((format!("fig-ticks-clipped-{theme_name}"), f));

        // Empty tick lists: no ticks, labels or gridlines on either axis.
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "No ticks");
        let mut a = axes(&mut f);
        two_lines(&mut a, 30, 2.0, 6.0);
        a.xticks(&[]).unwrap().yticks(&[]).unwrap();
        out.push((format!("fig-ticks-empty-{theme_name}"), f));
    }

    // Exact values are never snapped to zero: tiny positions on a linear axis get labels of
    // their own, and sparse positions on a log axis keep their scientific and fixed labels.
    let mut f = figure(6.0, 4.0, umber_core::Theme::pigment(), linear(), log(10.0));
    labelled(&mut f, "Tiny and sparse positions");
    let x: Vec<f64> = (-3..=3).map(|i| f64::from(i) * 1e-24).collect();
    let y: Vec<f64> = [1e-11, 1e-8, 1e-5, 1e-2, 1e1, 1e2, 5e2].to_vec();
    axes(&mut f)
        .scatter(numbers(x), &y, ScatterOptions::default())
        .unwrap()
        .xticks(&[-2e-24, 0.0, 2e-24])
        .unwrap()
        .yticks(&[1e-12, 1.0, 1e3])
        .unwrap()
        .ytick_format(TickFormat::scientific())
        .unwrap();
    out.push(("fig-ticks-tiny-sparse".to_owned(), f));

    // Bars without a finite height keep 0 in the domain that explicit ticks set.
    let mut f = figure(
        6.0,
        4.0,
        umber_core::Theme::graphite(),
        ScaleKind::Category,
        linear(),
    );
    labelled(&mut f, "Ticks over missing bars");
    axes(&mut f)
        .bar(
            strings(&["a", "b"]),
            &[f64::NAN, f64::NAN],
            BarOptions::default(),
        )
        .unwrap()
        .yticks(&[2.0, 4.0])
        .unwrap();
    out.push(("fig-ticks-nan-bars".to_owned(), f));
    out
}

/// A tick format as recorded: its style, prefix and suffix.
fn format(f: &TickFormat) -> String {
    let style = match f.style() {
        NumberStyle::Auto => "auto".to_owned(),
        NumberStyle::Fixed(d) => format!("fixed {}", d.get()),
        NumberStyle::Percent(d) => format!("percent {}", d.get()),
        NumberStyle::Scientific => "scientific".to_owned(),
        NumberStyle::Iso => "iso".to_owned(),
    };
    format!(
        "{style} prefix={} suffix={}",
        record::text(f.prefix()),
        record::text(f.suffix())
    )
}

/// Explicit ticks as recorded: the bits of every position, and the labels given with them.
fn explicit(t: Option<&ExplicitTicks>) -> String {
    match t {
        None => "auto".to_owned(),
        Some(ExplicitTicks::Positions(values)) => {
            format!("positions {}", record::floats64(values))
        }
        Some(ExplicitTicks::Labeled(ticks)) => {
            let ticks: Vec<String> = ticks
                .iter()
                .map(|t| format!("{} {}", record::f64(t.value), record::text(&t.label)))
                .collect();
            format!("labeled [{}]", ticks.join(", "))
        }
    }
}

/// The tick settings of both axes of `f`.
fn tick_state(f: &Figure) -> String {
    let a = f.axes_at(GridIndex::Auto, GridIndex::Auto).unwrap();
    format!(
        "  ticks x={} x_format={} y={} y_format={}\n",
        explicit(a.x_ticks()),
        format(a.x_tick_format()),
        explicit(a.y_ticks()),
        format(a.y_tick_format())
    )
}

/// Runs `op` on the axes of `f` and records its result, the figure and its tick settings.
fn on_ticks(
    rec: &mut Log,
    f: &mut Figure,
    label: &str,
    op: impl for<'b, 'f> FnOnce(&'b mut AxesMut<'f>) -> umber_core::Result<&'b mut AxesMut<'f>>,
) {
    rec.on_axes(f, label, |a| op(a).map(drop));
    rec.raw(&tick_state(f));
}

/// The results of the `TickFormat` constructors and the tick setters of `Axes`, and the state
/// each call leaves.
fn tick_errors() -> String {
    let mut rec = Log::default();
    rec.section("TickFormat::fixed and TickFormat::percent");
    for d in [i64::MIN, -1, 0, 1, 12, 13, 255, 256, i64::MAX] {
        rec.result(&format!("fixed({d})"), TickFormat::fixed(d), |f| format(&f));
        rec.result(&format!("percent({d})"), TickFormat::percent(d), |f| {
            format(&f)
        });
    }
    rec.section("TickFormat values");
    let mut out = String::new();
    let _ = writeln!(out, "AUTO: {}", format(&TickFormat::AUTO));
    let _ = writeln!(out, "default: {}", format(&TickFormat::default()));
    let _ = writeln!(out, "scientific: {}", format(&TickFormat::scientific()));
    let _ = writeln!(
        out,
        "is_auto: AUTO={} AUTO.affix(\"\", \"\")={} AUTO.affix(\"\", \"%\")={} fixed(0)={}",
        TickFormat::AUTO.is_auto(),
        TickFormat::AUTO.affix("", "").unwrap().is_auto(),
        TickFormat::AUTO.affix("", "%").unwrap().is_auto(),
        TickFormat::fixed(0).unwrap().is_auto()
    );
    rec.raw(&out);

    rec.section("TickFormat::affix");
    let base = TickFormat::fixed(1).unwrap();
    let at_cap = "\u{e9}".repeat(1000);
    let long = "\u{e9}".repeat(1001);
    for (label, prefix, suffix) in [
        ("affix(\"$\", \" M\")", "$", " M"),
        ("affix(\"\", \"\")", "", ""),
        (
            "affix(1000 chars, 1000 chars)",
            at_cap.as_str(),
            at_cap.as_str(),
        ),
        ("affix(1001 chars, \"\")", long.as_str(), ""),
        ("affix(\"\", 1001 chars)", "", long.as_str()),
        (
            "affix(1001 chars, 1001 chars)",
            long.as_str(),
            long.as_str(),
        ),
    ] {
        rec.result(label, base.affix(prefix, suffix), |f| {
            let mut s = format(&f);
            s.truncate(80);
            s
        });
    }
    let replaced = base.affix("$", "").unwrap().affix("", " kg").unwrap();
    rec.raw(&format!("affix replaces: {}\n", format(&replaced)));

    rec.section("Axes tick positions, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.raw(&tick_state(&f));
    let bad: [(&str, &[f64]); 8] = [
        ("NaN", &[0.0, f64::NAN]),
        ("inf", &[f64::INFINITY]),
        ("-inf after a repeat", &[1.0, 1.0, f64::NEG_INFINITY]),
        ("repeat", &[0.0, 1.0, 1.0]),
        ("decreasing", &[3.0, 2.0, 1.0]),
        ("beyond the magnitude", &[0.0, 2e300]),
        ("below the magnitude", &[-1e301, 0.0]),
        ("NaN first, then decreasing", &[f64::NAN, 1.0, 0.0]),
    ];
    for (name, positions) in bad {
        on_ticks(&mut rec, &mut f, &format!("xticks {name}"), |a| {
            a.xticks(positions)
        });
        on_ticks(&mut rec, &mut f, &format!("yticks {name}"), |a| {
            a.yticks(positions)
        });
    }
    on_ticks(&mut rec, &mut f, "xticks at the magnitude bounds", |a| {
        a.xticks(&[-1e300, -0.0, 0.0001, 1e300])
    });
    on_ticks(&mut rec, &mut f, "yticks empty", |a| a.yticks(&[]));

    rec.section("Axes tick labels, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    let labels_cases: [(&str, &[f64], Vec<String>); 5] = [
        ("one label short", &[0.0, 1.0], strings(&["a"])),
        ("one label too many", &[0.0], strings(&["a", "b"])),
        (
            "a label of 1001 characters",
            &[0.0, 1.0],
            vec!["ok".to_owned(), "x".repeat(1001)],
        ),
        ("bad positions and labels", &[1.0, 0.0], strings(&["a"])),
        ("no positions, no labels", &[], Vec::new()),
    ];
    for (name, positions, labels) in labels_cases {
        on_ticks(&mut rec, &mut f, &format!("xticks_labeled {name}"), |a| {
            a.xticks_labeled(positions, labels.clone())
        });
        on_ticks(&mut rec, &mut f, &format!("yticks_labeled {name}"), |a| {
            a.yticks_labeled(positions, labels)
        });
    }
    on_ticks(&mut rec, &mut f, "xticks_labeled at the cap", |a| {
        a.xticks_labeled(&[0.0, 2.5], vec!["".to_owned(), "\u{e9}".repeat(1000)])
    });

    rec.section("Axes ticks, log/log");
    let mut f = rec.new_figure("log/log", log(10.0), log(10.0));
    let bad: [(&str, &[f64]); 5] = [
        ("zero", &[0.0, 1.0]),
        ("negative", &[1.0, -10.0]),
        ("below the log range", &[1e-301, 1.0]),
        ("above the log range", &[1.0, 2e300]),
        ("NaN", &[f64::NAN]),
    ];
    for (name, positions) in bad {
        on_ticks(&mut rec, &mut f, &format!("xticks {name}"), |a| {
            a.xticks(positions)
        });
        on_ticks(&mut rec, &mut f, &format!("yticks_labeled {name}"), |a| {
            a.yticks_labeled(positions, vec![String::new(); positions.len()])
        });
    }
    on_ticks(&mut rec, &mut f, "yticks at the log bounds", |a| {
        a.yticks(&[1e-300, 1.0, 1e300])
    });

    rec.section("Axes ticks, category/linear");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    on_ticks(&mut rec, &mut f, "xticks", |a| a.xticks(&[0.0, 1.0]));
    on_ticks(&mut rec, &mut f, "xticks bad positions", |a| {
        a.xticks(&[f64::NAN])
    });
    on_ticks(&mut rec, &mut f, "xticks_labeled", |a| {
        a.xticks_labeled(&[0.0], strings(&["a"]))
    });
    on_ticks(&mut rec, &mut f, "xtick_format AUTO", |a| {
        a.xtick_format(TickFormat::AUTO)
    });
    on_ticks(&mut rec, &mut f, "xtick_format fixed", |a| {
        a.xtick_format(TickFormat::fixed(1).unwrap())
    });
    on_ticks(&mut rec, &mut f, "yticks", |a| a.yticks(&[0.0, 1.0]));

    rec.section("Labels and formats conflict, in both orders");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    on_ticks(&mut rec, &mut f, "xtick_format percent", |a| {
        a.xtick_format(TickFormat::percent(0).unwrap())
    });
    on_ticks(&mut rec, &mut f, "xticks_labeled after a format", |a| {
        a.xticks_labeled(&[0.0, 1.0], strings(&["off", "on"]))
    });
    on_ticks(&mut rec, &mut f, "xticks after a format", |a| {
        a.xticks(&[0.0, 0.5, 1.0])
    });
    on_ticks(&mut rec, &mut f, "xtick_format AUTO", |a| {
        a.xtick_format(TickFormat::AUTO)
    });
    on_ticks(&mut rec, &mut f, "xticks_labeled after AUTO", |a| {
        a.xticks_labeled(&[0.0, 1.0], strings(&["off", "on"]))
    });
    on_ticks(&mut rec, &mut f, "xtick_format fixed after labels", |a| {
        a.xtick_format(TickFormat::fixed(2).unwrap())
    });
    on_ticks(
        &mut rec,
        &mut f,
        "xtick_format AUTO with a suffix after labels",
        |a| a.xtick_format(TickFormat::AUTO.affix("", " s").unwrap()),
    );
    on_ticks(&mut rec, &mut f, "xtick_format AUTO after labels", |a| {
        a.xtick_format(TickFormat::AUTO)
    });
    on_ticks(&mut rec, &mut f, "yticks_labeled", |a| {
        a.yticks_labeled(&[0.0], strings(&["zero"]))
    });
    on_ticks(
        &mut rec,
        &mut f,
        "ytick_format scientific after labels",
        |a| a.ytick_format(TickFormat::scientific()),
    );
    on_ticks(&mut rec, &mut f, "yticks replaces the labels", |a| {
        a.yticks(&[0.0, 2.0])
    });
    on_ticks(&mut rec, &mut f, "ytick_format scientific", |a| {
        a.ytick_format(TickFormat::scientific())
    });
    on_ticks(
        &mut rec,
        &mut f,
        "yticks_labeled after AUTO with a prefix",
        |a| {
            a.ytick_format(TickFormat::AUTO.affix("~", "").unwrap())?
                .yticks_labeled(&[0.0], strings(&["zero"]))
        },
    );
    on_ticks(&mut rec, &mut f, "xticks replaces the labels", |a| {
        a.xticks(&[0.25])
    });
    on_ticks(
        &mut rec,
        &mut f,
        "xtick_format fixed after positions",
        |a| a.xtick_format(TickFormat::fixed(2).unwrap()),
    );

    rec.section("Tick settings survive a clone");
    let copy = f.clone();
    axes(&mut f).yticks(&[9.0]).unwrap();
    rec.raw(&format!(
        "clone: {}source: {}",
        tick_state(&copy),
        tick_state(&f)
    ));
    rec.out
}
