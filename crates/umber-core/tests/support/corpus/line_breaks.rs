//! Titles, subtitles, text and annotations of several lines, and the refusal of line breaks and
//! control characters in text of one line.

use std::fmt::Write as _;

use umber_core::{
    Align, AnnotateOptions, AxisData, AxisValue, Figure, Font, FontLibrary, Grid, GridIndex,
    LegendStyle, LineOptions, PieOptions, Placement, ScaleKind, ScatterOptions, Series, SizeSpec,
    TextOptions, Theme, TickFormat, ValueLabels,
};

use super::super::{Output, axes, emit_scene, figure, linear, record};

fn number(v: f64) -> AxisValue {
    AxisValue::Number(v)
}

/// A line chart whose title has three lines and whose subtitle has two, in each title
/// alignment.
fn header_cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, align) in [
        ("left", Align::Left),
        ("center", Align::Center),
        ("right", Align::Right),
    ] {
        let theme = Theme::pigment().with_title_align(align).unwrap();
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        axes(&mut f)
            .title("Station output\nDaily mean across the network\nMW")
            .subtitle("Measured hourly\n2026")
            .xlabel("Hour")
            .ylabel("Output")
            .line(
                AxisData::Numbers(vec![0.0, 6.0, 12.0, 18.0, 24.0]),
                &[3.0, 5.0, 9.0, 7.0, 4.0],
                LineOptions {
                    label: "North".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("line-breaks-header-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
}

/// A figure title of two lines over a grid of two panels.
fn grid_case(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let grid = Grid::new(1, 2).unwrap();
    let mut f = Figure::with_grid(9.0, 4.0, Theme::graphite(), grid).unwrap();
    f.title("Regional growth\nacross both regions")
        .subtitle("Weekly totals");
    for (col, title) in [(0, "North"), (1, "South\nand coast")] {
        f.add_axes_at(
            Placement {
                col: GridIndex::At(col),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap()
        .title(title)
        .line(
            AxisData::Numbers(vec![1.0, 2.0, 3.0, 4.0]),
            &[2.0, 4.0, 3.0, 5.0],
            LineOptions::default(),
        )
        .unwrap();
    }
    emit_scene(
        "line-breaks-figure-title-grid",
        &f.to_scene(lib).unwrap(),
        sink,
    );
}

/// Text of several lines in each alignment, and annotations of two lines with an arrow in
/// each of the eight directions.
fn text_cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    let mut ax = axes(&mut f);
    ax.title("Text of several lines")
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(
            AxisData::Numbers(vec![2.0, 5.0, 8.0]),
            &[8.0, 5.5, 3.0],
            ScatterOptions::default(),
        )
        .unwrap();
    for (text, (x, y), align) in [
        (
            "LEFT\nleft edge\n\nafter a blank line",
            (2.0, 8.0),
            Align::Left,
        ),
        ("CENTER\nthe centre of each line", (5.0, 5.5), Align::Center),
        ("RIGHT\nright edge", (8.0, 3.0), Align::Right),
    ] {
        let options = TextOptions {
            align,
            ..TextOptions::default()
        };
        ax.text(text, (number(x), number(y)), options).unwrap();
    }
    emit_scene("line-breaks-text", &f.to_scene(lib).unwrap(), sink);

    let mut f = figure(8.0, 6.0, Theme::pigment(), linear(), linear());
    let mut ax = axes(&mut f);
    let points = [
        ("N", (5.0, 7.0), (0.0, 40.0)),
        ("NE", (7.0, 7.0), (34.0, 34.0)),
        ("E", (7.0, 5.0), (40.0, 0.0)),
        ("SE", (7.0, 3.0), (34.0, -34.0)),
        ("S", (5.0, 3.0), (0.0, -40.0)),
        ("SW", (3.0, 3.0), (-34.0, -34.0)),
        ("W", (3.0, 5.0), (-40.0, 0.0)),
        ("NW", (3.0, 7.0), (-34.0, 34.0)),
    ];
    let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().map(|(_, at, _)| *at).unzip();
    ax.title("Annotations of two lines")
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(AxisData::Numbers(xs), &ys, ScatterOptions::default())
        .unwrap();
    for (direction, (x, y), offset) in points {
        let options = AnnotateOptions {
            offset,
            ..AnnotateOptions::default()
        };
        ax.annotate(
            &format!("{direction}\nwider second line"),
            (number(x), number(y)),
            options,
        )
        .unwrap();
    }
    emit_scene("line-breaks-annotate", &f.to_scene(lib).unwrap(), sink);
}

/// A pie whose title has two lines.
fn pie_case(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut f = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
    f.add_pie_axes()
        .unwrap()
        .title("Orders\nby region, 2026")
        .pie(
            vec!["North".into(), "South".into(), "West".into()],
            &[5.0, 3.0, 2.0],
            PieOptions::default(),
        )
        .unwrap();
    emit_scene("line-breaks-pie-title", &f.to_scene(lib).unwrap(), sink);
}

/// The errors of line breaks and control characters in text of one line, at the call where the
/// call returns a result and when rendering otherwise, and of control characters in text of
/// several lines.
fn error_cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut out = String::new();
    let mut push = |name: &str, err: umber_core::Error| {
        let _ = writeln!(
            out,
            "{name}: {}\nparameter={:?} hint={:?}",
            record::error(&err),
            err.parameter(),
            err.hint(),
        );
    };
    let render = |configure: &dyn Fn(&mut Figure)| {
        let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
        configure(&mut f);
        f.to_scene(lib).unwrap_err()
    };
    push(
        "xlabel line break",
        render(&|f| {
            axes(f).xlabel("Hours\nUTC");
        }),
    );
    push(
        "ylabel tab",
        render(&|f| {
            axes(f).ylabel("MW\tday");
        }),
    );
    push(
        "title carriage return",
        render(&|f| {
            axes(f).title("Line one\r\nLine two");
        }),
    );
    push(
        "subtitle line separator",
        render(&|f| {
            axes(f).subtitle("a\u{2028}b");
        }),
    );
    push(
        "figure title next line",
        render(&|f| {
            f.title("a\u{85}b");
        }),
    );
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    let mut hidden = axes(&mut f);
    hidden.legend(LegendStyle::None, SizeSpec::Auto).unwrap();
    push(
        "legend label line break, legend hidden",
        hidden
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: "North\nSouth".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap_err(),
    );
    let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    push(
        "series label tab",
        axes(&mut f)
            .grouped_bar(
                vec!["A".into()],
                vec![Series {
                    label: "North\tSouth".into(),
                    values: vec![1.0],
                    ..Series::default()
                }],
                ValueLabels::default(),
            )
            .unwrap_err(),
    );
    push(
        "font family line break",
        Font::system("DejaVu Sans\n").unwrap_err(),
    );
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    let at = (number(0.5), number(0.5));
    push(
        "text null",
        axes(&mut f)
            .text("a\0b", at.clone(), TextOptions::default())
            .unwrap_err(),
    );
    push(
        "annotate tab",
        axes(&mut f)
            .annotate("a\tb", at, AnnotateOptions::default())
            .unwrap_err(),
    );
    push(
        "tick label line break",
        axes(&mut f)
            .xticks_labeled(&[0.0, 1.0], vec!["low".into(), "high\nend".into()])
            .unwrap_err(),
    );
    push(
        "affix line break",
        TickFormat::AUTO.affix("", " MW\n").unwrap_err(),
    );
    let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    push(
        "category line break",
        axes(&mut f)
            .line(
                AxisData::Categories(vec!["North".into(), "South\nEast".into()]),
                &[1.0, 2.0],
                LineOptions::default(),
            )
            .unwrap_err(),
    );
    sink(Output {
        kind: "errors",
        file: "line-breaks.txt".into(),
        bytes: out.into_bytes(),
    });
}

/// Emits every case of this module.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    header_cases(lib, sink);
    grid_case(lib, sink);
    text_cases(lib, sink);
    pie_case(lib, sink);
    error_cases(lib, sink);
}
