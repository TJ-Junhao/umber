//! Shared axes in every mode and theme, and failed calls with the state they leave.

use umber_core::figure::{AxisData, BarOptions, LineOptions};
use umber_core::{
    AxesMut, ExplicitTicks, Figure, FontLibrary, Grid, GridIndex, NumberStyle, Placement, Result,
    ScaleKind, Share, Theme, TickFormat,
};

use super::super::{Log, Output, emit_scene, linear, log, record, themes};

fn at(row: i64, col: i64) -> Placement {
    Placement {
        row: GridIndex::At(row),
        col: GridIndex::At(col),
        ..Placement::default()
    }
}

fn figure(rows: i64, cols: i64, theme: Theme, x: Share, y: Share) -> Figure {
    let grid = Grid::new(rows, cols)
        .unwrap()
        .with_share_x(x)
        .with_share_y(y);
    let (w, h) = grid.auto_size_in();
    Figure::with_grid(w, h, theme, grid).unwrap()
}

fn line(f: &mut Figure, place: Placement, kind: ScaleKind, high: f64) {
    let mut a = f.add_axes_at(place, kind, kind).unwrap();
    let c = a.cells();
    a.title(&format!("Panel {}, {}", c.row(), c.col()))
        .xlabel("Time")
        .ylabel("Level")
        .line(
            AxisData::Numbers(vec![1.0, 2.0, high]),
            &[1.0, high, 2.0],
            LineOptions::default(),
        )
        .unwrap();
}

/// Emits the complete sharing corpus.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let modes = [
        ("none", Share::None),
        ("row", Share::Row),
        ("col", Share::Col),
        ("all", Share::All),
    ];
    for (theme_name, theme) in themes() {
        for (xn, x) in modes {
            for (yn, y) in modes {
                let mut f = figure(2, 2, theme.clone(), x, y);
                for (r, c, high) in [(0, 0, 3.0), (0, 1, 5.0), (1, 0, 10.0), (1, 1, 20.0)] {
                    line(&mut f, at(r, c), linear(), high);
                }
                emit_scene(
                    &format!("share-2x2-x-{xn}-y-{yn}-{theme_name}"),
                    &f.to_scene(lib).unwrap(),
                    sink,
                );
            }
        }
        let mut categories = figure(2, 2, theme.clone(), Share::All, Share::Row);
        // Insertion order differs from the order in which categories enter the shared scale.
        for (r, c, names) in [
            (1, 1, ["d", "b"]),
            (0, 1, ["a", "b"]),
            (1, 0, ["c", "e"]),
            (0, 0, ["b", "c"]),
        ] {
            categories
                .add_axes_at(at(r, c), ScaleKind::Category, linear())
                .unwrap()
                .title(&format!("Region {}, {}", r, c))
                .bar(
                    names.map(str::to_owned).to_vec(),
                    &[2.0 + r as f64, 5.0 + c as f64],
                    BarOptions::default(),
                )
                .unwrap();
        }
        emit_scene(
            &format!("share-category-{theme_name}"),
            &categories.to_scene(lib).unwrap(),
            sink,
        );

        let mut sparse = figure(3, 3, theme.clone(), Share::Col, Share::Row);
        for (r, c) in [(0, 0), (0, 2), (1, 0), (1, 2), (2, 0)] {
            line(&mut sparse, at(r, c), linear(), 4.0 + r as f64 + c as f64);
        }
        emit_scene(
            &format!("share-empty-column-{theme_name}"),
            &sparse.to_scene(lib).unwrap(),
            sink,
        );

        let mut logs = figure(2, 2, theme.clone(), Share::All, Share::All);
        for (r, c) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            line(
                &mut logs,
                at(r, c),
                log(10.0),
                10.0_f64.powi((r * 2 + c + 1) as i32),
            );
        }
        emit_scene(
            &format!("share-log-{theme_name}"),
            &logs.to_scene(lib).unwrap(),
            sink,
        );

        let mut spanning = figure(2, 3, theme, Share::All, Share::Row);
        line(
            &mut spanning,
            Placement {
                col_span: 2,
                ..at(0, 0)
            },
            linear(),
            30.0,
        );
        line(&mut spanning, at(0, 2), linear(), 5.0);
        for c in 0..3 {
            line(&mut spanning, at(1, c), linear(), 10.0 + c as f64);
        }
        emit_scene(
            &format!("share-span-{theme_name}"),
            &spanning.to_scene(lib).unwrap(),
            sink,
        );
    }
    sink(Output {
        kind: "errors",
        file: "share.txt".into(),
        bytes: errors(lib).into_bytes(),
    });
}

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
        Some(ExplicitTicks::TimePositions(times)) => {
            let times: Vec<String> = times.iter().map(|t| t.micros().to_string()).collect();
            format!("time positions [{}]", times.join(","))
        }
        Some(ExplicitTicks::TimeLabeled(ticks)) => {
            let ticks: Vec<String> = ticks
                .iter()
                .map(|(t, label)| format!("{} {}", t.micros(), record::text(label)))
                .collect();
            format!("time labeled [{}]", ticks.join(", "))
        }
    }
}

fn snapshot(f: &Figure) -> String {
    let mut out = record::figure(f);
    for a in f.panels() {
        out.push_str(&format!(
            "  ticks row={} col={} x={} x_format={} y={} y_format={}\n",
            a.cells().row(),
            a.cells().col(),
            explicit(a.x_ticks()),
            format(a.x_tick_format()),
            explicit(a.y_ticks()),
            format(a.y_tick_format())
        ));
    }
    out
}

fn state(rec: &mut Log, f: &Figure, before: &Figure, lib: &FontLibrary) {
    rec.raw(&snapshot(f));
    let svg = |f: &Figure| {
        umber_core::render::render_svg(
            &f.to_scene(lib).unwrap(),
            umber_core::render::SvgText::Editable,
        )
    };
    rec.raw(&format!(
        "figure unchanged={} svg unchanged={}\n",
        f == before,
        svg(f) == svg(before)
    ));
}

fn set(a: &mut AxesMut<'_>, operation: usize, other: bool) -> Result<()> {
    let high = if other { 5.0 } else { 10.0 };
    match operation {
        0 => a.xlim(0.0, high),
        1 => a.ylim(0.0, high),
        2 => a.xticks(&[0.0, high]),
        3 => a.yticks(&[0.0, high]),
        4 => a.xticks_labeled(
            &[0.0, 10.0],
            vec!["start".into(), if other { "other" } else { "end" }.into()],
        ),
        5 => a.yticks_labeled(
            &[0.0, 10.0],
            vec!["start".into(), if other { "other" } else { "end" }.into()],
        ),
        6 => a.xtick_format(TickFormat::fixed(if other { 2 } else { 1 })?),
        _ => a.ytick_format(TickFormat::fixed(if other { 2 } else { 1 })?),
    }
    .map(|_| ())
}

fn errors(lib: &FontLibrary) -> String {
    let mut rec = Log::default();
    for (name, x, y) in [
        ("mixed x category", ScaleKind::Category, linear()),
        ("mixed x log", log(2.0), linear()),
        ("mixed y log", linear(), log(2.0)),
    ] {
        rec.section(name);
        let mut f = figure(2, 2, Theme::graphite(), Share::All, Share::All);
        line(&mut f, at(0, 0), linear(), 3.0);
        let before = f.clone();
        rec.result(name, f.add_axes_at(at(1, 1), x, y).map(drop), |()| {
            String::new()
        });
        state(&mut rec, &f, &before, lib);
    }
    for is_x in [true, false] {
        let mut f = figure(2, 2, Theme::graphite(), Share::All, Share::All);
        line(&mut f, at(0, 0), log(10.0), 10.0);
        let before = f.clone();
        let (x, y) = if is_x {
            (log(2.0), log(10.0))
        } else {
            (log(10.0), log(2.0))
        };
        rec.result(
            "different log base",
            f.add_axes_at(at(1, 1), x, y).map(drop),
            |()| String::new(),
        );
        state(&mut rec, &f, &before, lib);
    }
    for (x, y, place) in [
        (
            Share::Row,
            Share::None,
            Placement {
                row_span: 2,
                ..at(0, 1)
            },
        ),
        (
            Share::Col,
            Share::None,
            Placement {
                col_span: 2,
                ..at(1, 0)
            },
        ),
        (
            Share::None,
            Share::Row,
            Placement {
                row_span: 2,
                ..at(0, 1)
            },
        ),
        (
            Share::None,
            Share::Col,
            Placement {
                col_span: 2,
                ..at(1, 0)
            },
        ),
    ] {
        let mut f = figure(2, 2, Theme::graphite(), x, y);
        line(&mut f, at(0, 0), linear(), 3.0);
        let before = f.clone();
        rec.result(
            "span across groups",
            f.add_axes_at(place, linear(), linear()).map(drop),
            |()| String::new(),
        );
        state(&mut rec, &f, &before, lib);
    }
    for (op, name) in [
        "xlim",
        "ylim",
        "xticks",
        "yticks",
        "xticks_labeled",
        "yticks_labeled",
        "xtick_format",
        "ytick_format",
    ]
    .into_iter()
    .enumerate()
    {
        rec.section(name);
        let mut f = figure(2, 1, Theme::graphite(), Share::All, Share::All);
        line(&mut f, at(0, 0), linear(), 3.0);
        set(
            &mut f.axes_at_mut(GridIndex::At(0), GridIndex::At(0)).unwrap(),
            op,
            false,
        )
        .unwrap();
        line(&mut f, at(1, 0), linear(), 10.0);
        rec.raw(&snapshot(&f));
        let before = f.clone();
        rec.result(
            "different value",
            set(
                &mut f.axes_at_mut(GridIndex::At(1), GridIndex::At(0)).unwrap(),
                op,
                true,
            ),
            |()| String::new(),
        );
        state(&mut rec, &f, &before, lib);
        rec.result(
            "same value",
            set(
                &mut f.axes_at_mut(GridIndex::At(1), GridIndex::At(0)).unwrap(),
                op,
                false,
            ),
            |()| String::new(),
        );
        rec.raw(&snapshot(&f));
    }
    rec.out
}
