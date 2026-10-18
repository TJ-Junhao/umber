//! Output-gate cases for the 64-bit integer inputs, `Error::with_context` and
//! `ColorSpec::palette`.
//!
//! The cases write only a new output file, `errors/inputs.txt`, so no existing manifest entry
//! changes. See `scripts/check-refactor-output.sh`.
//!
//! Covered: histogram bin counts below 1 (negative, `i64::MIN`) with the state a failed call
//! leaves; PNG resolutions that are negative, beyond `u32` and at the `i64` limits, and the two
//! valid bounds; an error re-reported under another API call with its parameter and hint
//! kept, for each error kind, and an I/O error left unchanged; and palette indexes that are
//! negative or valid.

use umber_core::figure::HistOptions;
use umber_core::{Alpha, ColorSpec, Error, FontLibrary, ScaleKind};

use super::super::{Log, Output, linear, record};

pub fn cases(_lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    sink(Output {
        kind: "errors",
        file: "inputs.txt".into(),
        bytes: input_errors().into_bytes(),
    });
}

fn input_errors() -> String {
    let mut rec = Log::default();

    rec.section("Axes.hist, bins below 1");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    for bins in [-1, -30, i64::from(i32::MIN), i64::MIN] {
        rec.on_axes(&mut f, &format!("hist {bins} bins"), |a| {
            a.hist(
                &[1.0, 2.0],
                HistOptions {
                    bins: umber_core::Bins::Count(bins),
                    ..HistOptions::default()
                },
            )
            .map(drop)
        });
    }
    rec.on_axes(&mut f, "hist -1 bins before an infinite value", |a| {
        a.hist(
            &[f64::INFINITY],
            HistOptions {
                bins: umber_core::Bins::Count(-1),
                ..HistOptions::default()
            },
        )
        .map(drop)
    });
    let mut g = rec.new_figure("category/linear", ScaleKind::Category, linear());
    rec.on_axes(&mut g, "hist -1 bins on a category x axis", |a| {
        a.hist(
            &[1.0],
            HistOptions {
                bins: umber_core::Bins::Count(-1),
                ..HistOptions::default()
            },
        )
        .map(drop)
    });

    rec.section("Figure.to_png, dpi");
    let f = rec.new_figure("linear/linear", linear(), linear());
    for dpi in [
        -1,
        i64::MIN,
        i64::from(u32::MAX) + 1,
        i64::from(u32::MAX) + 73,
        i64::MAX,
    ] {
        let result = f.to_png(dpi).map(drop);
        rec.result(&format!("to_png dpi {dpi}"), result, |()| String::new());
    }
    for dpi in [10, 2400] {
        let result = f.to_png(dpi).map(|png| png.starts_with(b"\x89PNG"));
        rec.result(&format!("to_png dpi {dpi}"), result, |png| {
            format!("png={png}")
        });
    }

    rec.section("Error.with_context");
    let styled = f.to_png(-5).unwrap_err();
    rec.raw(&format!(
        "style, before: {}\nstyle, after: {}\n",
        record::error(&styled),
        record::error(&f.to_png(-5).unwrap_err().with_context("Figure.save_png()")),
    ));
    let data = Error::Data {
        context: "Axes.line()",
        message: "y[2] is inf; values must be finite.".into(),
        parameter: Some("y"),
        hint: Some("use NaN to leave a gap in the line."),
    };
    let usage = Error::Usage {
        context: "Figure.add_axes()",
        message: "the figure already has axes.".into(),
        parameter: None,
        hint: Some("create a new Figure for another chart."),
    };
    for (label, error) in [("data", data), ("usage", usage)] {
        let error = error.with_context("Figure.save_svg()");
        rec.raw(&format!("{label}: {}\n", record::error(&error)));
    }
    let io = Error::from(std::io::Error::other("disk full")).with_context("Figure.save_png()");
    rec.raw(&format!("io: {}\n", record::error(&io)));

    rec.section("ColorSpec::palette");
    let half = Alpha::new(0.5).unwrap();
    for (label, index) in [
        ("zero", 0),
        ("seven", 7),
        ("i32::MAX", i64::from(i32::MAX)),
        ("minus one", -1),
        ("i32::MIN", i64::from(i32::MIN)),
        ("i64::MIN", i64::MIN),
    ] {
        for alpha in [Alpha::OPAQUE, half] {
            rec.result(
                &format!("palette {label} alpha {}", record::alpha(alpha)),
                ColorSpec::palette(index, alpha),
                record::color_spec,
            );
        }
    }
    rec.out
}
