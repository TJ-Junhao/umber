//! The layout of one panel in two steps around its plot area.
//!
//! [`measure_panel`] measures the panel's texts for resolved scales and the bands they need
//! around the plot area: the header and the y label above it, the y tick label column left of
//! it, the room direct labels reserve right of it, and the x tick label and x label rows below
//! it. Choosing the plot area from these bands is left to the caller: for a single panel that
//! fills the canvas, [`super::lay_out`] does it. [`place_panel`] then places the panel's texts,
//! gridlines and legend for that plot area, the header inside the panel's [`Frame`].

use super::inset::TickSpan;
use super::legend::{self, LegendRow};
use super::{
    AXIS_LABEL_GAP, Area, BODY_LINE, END_LABEL_SHARE, HEADER_GAP, Label, Layout, LayoutInput,
    PADDING, Styles, TITLE_GAP, TITLE_LINE, X_LABEL_GAP, Y_LABEL_CENTER, Y_LABEL_GAP,
    baseline_in_line, figure_height_hint, measure, measure_ticks, metrics,
};
use crate::resolve::{NormalizedPos, Resolved, coincides};

/// A rectangle on the canvas, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Frame {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Frame {
    /// The right edge.
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// The bottom edge.
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

/// Which sides of a panel lie on the canvas edge. x tick labels at a canvas edge may reach
/// into the outer padding up to `X_LABEL_EDGE_GAP`; elsewhere their limit is set by the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "a single panel lies on every canvas edge; only panels on a grid need the flags"
)]
pub(crate) struct Edges {
    /// The left side is on the canvas edge.
    pub left: bool,
    /// The right side is on the canvas edge.
    pub right: bool,
    /// The top side is on the canvas edge.
    pub top: bool,
    /// The bottom side is on the canvas edge.
    pub bottom: bool,
}

/// One line of the header, measured: its label and the line box the label is centered in.
#[derive(Debug, Clone)]
struct HeaderLine {
    /// The label, not yet placed.
    label: Label,
    /// The height of the line box.
    line: f64,
    /// The ascent of the label style's primary font.
    ascent: f64,
    /// The descent of the label style's primary font.
    descent: f64,
}

/// The header of a panel, measured: title, subtitle and legend row, top down.
#[derive(Debug, Clone)]
pub(super) struct Header {
    /// The title; `None` when it is empty.
    title: Option<HeaderLine>,
    /// The subtitle; `None` when it is empty.
    subtitle: Option<HeaderLine>,
    /// The height of the legend row; `None` when the row has no entries.
    row: Option<f64>,
}

/// The vertical positions of the header parts below a given top.
struct Stacked {
    /// The title's baseline, when there is a title.
    title: Option<f64>,
    /// The subtitle's baseline, when there is a subtitle.
    subtitle: Option<f64>,
    /// The top of the legend row; where it would start when there is none.
    row: f64,
    /// Where the header ends, the gap below a non-empty header included.
    bottom: f64,
}

/// The header placed: its labels, the top of its legend row and its bottom.
struct PlacedHeader {
    title: Option<Label>,
    subtitle: Option<Label>,
    row: f64,
    bottom: f64,
}

impl Header {
    /// The header parts stacked from `top` down: the title line, the title gap and the subtitle
    /// line, the row gap and the legend row, then the header gap, each only when the header has
    /// the parts it separates. The positions are summed top down in this order, so they are
    /// the same to the bit wherever the header is stacked from the same top.
    fn stack(&self, top: f64) -> Stacked {
        let mut y = top;
        let title = self.title.as_ref().map(|title| {
            let baseline = baseline_in_line(y, title.line, title.ascent, title.descent);
            y += title.line;
            baseline
        });
        let subtitle = self.subtitle.as_ref().map(|subtitle| {
            if title.is_some() {
                y += TITLE_GAP;
            }
            let baseline = baseline_in_line(y, subtitle.line, subtitle.ascent, subtitle.descent);
            y += subtitle.line;
            baseline
        });
        let row = match self.row {
            Some(height) => {
                if title.is_some() || subtitle.is_some() {
                    y += legend::ROW_TOP_GAP;
                }
                let row_top = y;
                y += height;
                row_top
            }
            None => y,
        };
        if title.is_some() || subtitle.is_some() || self.row.is_some() {
            y += HEADER_GAP;
        }
        Stacked {
            title,
            subtitle,
            row,
            bottom: y,
        }
    }

    /// Where the header stacked from `top` ends, the gap below a non-empty header included;
    /// `top` itself when the header is empty.
    pub(super) fn bottom(&self, top: f64) -> f64 {
        self.stack(top).bottom
    }

    /// The hint of the error for a figure too short for the plot area, naming only the header
    /// parts this header has.
    pub(super) fn height_hint(&self) -> &'static str {
        figure_height_hint(
            self.title.is_some(),
            self.subtitle.is_some(),
            self.row.is_some(),
        )
    }

    /// The header stacked from `top` down, its labels starting at `left`.
    fn place(self, left: f64, top: f64) -> PlacedHeader {
        let stacked = self.stack(top);
        let at = |line: HeaderLine, baseline: f64| Label {
            x: left,
            y: baseline,
            ..line.label
        };
        PlacedHeader {
            title: self.title.zip(stacked.title).map(|(t, y)| at(t, y)),
            subtitle: self.subtitle.zip(stacked.subtitle).map(|(s, y)| at(s, y)),
            row: stacked.row,
            bottom: stacked.bottom,
        }
    }
}

/// Everything one panel needs around its plot area, measured for resolved scales, and the
/// measured texts that [`place_panel`] places.
#[derive(Debug, Clone)]
pub(crate) struct PanelBands {
    /// The header: title, subtitle, legend row and the gap below them; 0 when empty.
    #[allow(
        dead_code,
        reason = "the band a grid row aligns on; a single panel stacks its header from the \
                  canvas padding instead"
    )]
    pub header: f64,
    /// The y axis label above the top tick label row; 0 without one.
    pub y_label_band: f64,
    /// How far the top y tick label rises above the plot; 0 without y tick labels.
    pub label_rise: f64,
    /// The widest y tick label and [`Y_LABEL_GAP`]; 0 without y tick labels.
    #[allow(
        dead_code,
        reason = "the band a grid column aligns on; a single panel adds the widest label and \
                  the gap to the canvas padding one at a time"
    )]
    pub y_tick_column: f64,
    /// The room direct labels reserve right of the plot, at most [`END_LABEL_SHARE`] of the
    /// canvas width inside the padding; 0 without direct labels.
    pub direct_reserve: f64,
    /// The x tick label row and its gap; 0 without x tick labels.
    pub x_band: f64,
    /// The x axis label row and its gap; 0 without an x axis label.
    pub x_title_band: f64,
    /// The x tick labels as normalized positions and half widths, in tick order.
    pub x_spans: Vec<TickSpan>,
    /// The header, measured.
    pub(super) heading: Header,
    /// The y tick labels at their ticks' positions, not yet placed.
    pub(super) y_labels: Vec<(NormalizedPos, Label)>,
    /// The width of the widest y tick label; 0 without y tick labels.
    pub(super) y_label_width: f64,
    /// The x tick labels at their ticks' positions, not yet placed.
    pub(super) x_labels: Vec<(NormalizedPos, Label)>,
    /// The x axis label, not yet placed; `None` when it is empty.
    xlabel: Option<Label>,
    /// The y axis label, not yet placed; `None` when it is empty.
    ylabel: Option<Label>,
    /// The ascent of the tick label style's primary font.
    tick_ascent: f64,
    /// The tick label size.
    tick_size: f64,
    /// The ascent of the body style's primary font.
    body_ascent: f64,
}

/// Measures the texts of the panel of `input` for the scales of `resolved`, and the bands they
/// need around the plot area.
pub(crate) fn measure_panel(input: &LayoutInput<'_>, resolved: &Resolved) -> PanelBands {
    let LayoutInput {
        width,
        theme,
        axes,
        legend,
        lib,
        ..
    } = *input;
    let styles = Styles::new(theme);

    let title = (!axes.title_text().is_empty()).then(|| {
        let (ascent, descent) = metrics(lib, &styles.title);
        HeaderLine {
            label: measure(lib, axes.title_text(), styles.title),
            line: TITLE_LINE * f64::from(styles.title.size),
            ascent,
            descent,
        }
    });
    let subtitle = (!axes.subtitle_text().is_empty()).then(|| {
        let (ascent, descent) = metrics(lib, &styles.body);
        HeaderLine {
            label: measure(lib, axes.subtitle_text(), styles.body),
            line: BODY_LINE * f64::from(styles.body.size),
            ascent,
            descent,
        }
    });
    let heading = Header {
        title,
        subtitle,
        row: (!legend.row.is_empty()).then(|| legend.row.height()),
    };

    // Tick labels.
    let (tick_ascent, tick_descent) = metrics(lib, &styles.tick);
    let tick_size = f64::from(styles.tick.size);
    let y_labels = measure_ticks(lib, &resolved.y.ticks, styles.tick);
    let x_labels = measure_ticks(lib, &resolved.x.ticks, styles.tick);
    let y_label_width = y_labels
        .iter()
        .map(|(_, label)| label.width())
        .fold(0.0, f64::max);

    // How far the y tick label of a tick at the plot top reaches above it.
    let label_rise = if y_labels.is_empty() {
        0.0
    } else {
        (tick_ascent - Y_LABEL_CENTER * tick_size).max(0.0)
    };

    let xlabel =
        (!axes.xlabel_text().is_empty()).then(|| measure(lib, axes.xlabel_text(), styles.body));
    let ylabel =
        (!axes.ylabel_text().is_empty()).then(|| measure(lib, axes.ylabel_text(), styles.body));
    let (body_ascent, body_descent) = metrics(lib, &styles.body);

    // The y label sits in the header region, above the top label row.
    let y_label_band = if ylabel.is_some() {
        body_ascent + AXIS_LABEL_GAP
    } else {
        0.0
    };
    let y_tick_column = if y_labels.is_empty() {
        0.0
    } else {
        y_label_width + Y_LABEL_GAP
    };
    let x_spans = x_labels
        .iter()
        .map(|(pos, label)| TickSpan {
            pos: pos.get(),
            half: label.width() / 2.0,
        })
        .collect();
    let x_band = if x_labels.is_empty() {
        0.0
    } else {
        X_LABEL_GAP + tick_ascent + tick_descent
    };
    let x_title_band = if xlabel.is_some() {
        AXIS_LABEL_GAP + body_ascent + body_descent
    } else {
        0.0
    };

    PanelBands {
        header: heading.bottom(0.0),
        y_label_band,
        label_rise,
        y_tick_column,
        direct_reserve: legend.direct.reserve(end_label_cap(width)),
        x_band,
        x_title_band,
        x_spans,
        heading,
        y_labels,
        y_label_width,
        x_labels,
        xlabel,
        ylabel,
        tick_ascent,
        tick_size,
        body_ascent,
    }
}

/// The most by which x tick labels may inset one side of the plot area on a canvas `width` pt
/// wide, and the most that direct labels may reserve on its right: [`END_LABEL_SHARE`] of the
/// inner width, the canvas inside the outer padding.
pub(super) fn end_label_cap(width: f64) -> f64 {
    let inner = (width - 2.0 * PADDING).max(0.0);
    END_LABEL_SHARE * inner
}

/// The layout of the panel of `input` with the texts measured in `bands` for the scales of
/// `resolved`, placed for the plot area `plot`: the tick labels, axis labels, gridlines,
/// baseline and direct labels around and on the plot, and the header stacked down from the
/// top of `frame` inside the outer padding.
pub(crate) fn place_panel(
    input: &LayoutInput<'_>,
    resolved: &Resolved,
    bands: PanelBands,
    frame: Frame,
    plot: Area,
) -> Layout {
    let PanelBands {
        label_rise,
        x_band,
        heading,
        mut y_labels,
        y_label_width,
        mut x_labels,
        mut xlabel,
        mut ylabel,
        tick_ascent,
        tick_size,
        body_ascent,
        ..
    } = bands;
    let has_row = heading.row.is_some();
    let header = heading.place(frame.x + PADDING, frame.y + PADDING);
    let row = if has_row {
        input.legend.row.placed(header.row)
    } else {
        LegendRow::default()
    };

    for (pos, label) in &mut y_labels {
        label.x = plot.x - Y_LABEL_GAP - label.width();
        label.y = plot.y_at(*pos) + Y_LABEL_CENTER * tick_size;
    }
    for (pos, label) in &mut x_labels {
        label.x = plot.x_at(*pos) - label.width() / 2.0;
        label.y = plot.bottom() + X_LABEL_GAP + tick_ascent;
    }
    if let Some(label) = &mut xlabel {
        label.x = plot.x + plot.width / 2.0 - label.width() / 2.0;
        label.y = plot.bottom() + x_band + AXIS_LABEL_GAP + body_ascent;
    }
    if let Some(label) = &mut ylabel {
        // Left-aligned with the left edge of the y tick label column.
        label.x = if y_labels.is_empty() {
            plot.x
        } else {
            plot.x - Y_LABEL_GAP - y_label_width
        };
        label.y = plot.y - label_rise - AXIS_LABEL_GAP;
    }

    let direct = input.legend.direct.placed(resolved, plot, frame);
    let baseline = resolved.baseline;
    let baseline_y = plot.y_at(baseline);
    let y_grid = resolved
        .y
        .ticks
        .iter()
        .filter(|tick| !coincides(tick.pos, baseline))
        .map(|tick| plot.y_at(tick.pos))
        .collect();
    let x_grid = if resolved.x.is_category() {
        Vec::new()
    } else {
        resolved
            .x
            .ticks
            .iter()
            .map(|tick| plot.x_at(tick.pos))
            .collect()
    };

    Layout {
        plot,
        header_bottom: header.bottom,
        title: header.title,
        subtitle: header.subtitle,
        legend: row,
        direct,
        xlabel,
        ylabel,
        x_tick_labels: x_labels.into_iter().map(|(_, label)| label).collect(),
        y_tick_labels: y_labels.into_iter().map(|(_, label)| label).collect(),
        y_grid,
        x_grid,
        baseline_y,
        frame,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{DirectLabel, MIN_PLOT};
    use super::*;
    use crate::model::{AxisData, BarOptions, Figure, LineOptions};
    use crate::pipeline::{plan, with_layout_input};
    use crate::{FontLibrary, LegendStyle, ScaleKind, Theme};

    /// Three labelled lines over 30 days, with the header parts and axis labels asked for.
    fn lines(
        name: &'static str,
        theme: Theme,
        (width, height): (f64, f64),
        style: Option<LegendStyle>,
        texts: [&str; 4],
    ) -> (&'static str, Figure) {
        let mut f = Figure::new(width, height, theme).unwrap();
        let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let [title, subtitle, xlabel, ylabel] = texts;
        axes.title(title)
            .subtitle(subtitle)
            .xlabel(xlabel)
            .ylabel(ylabel);
        if let Some(style) = style {
            axes.legend(style);
        }
        for (i, label) in ["North", "South", "East"].into_iter().enumerate() {
            axes.line(
                AxisData::Numbers(vec![0.0, 12.5, 30.0]),
                &[i as f64, 40.0 + 7.0 * i as f64, 100.0 - 3.0 * i as f64],
                LineOptions {
                    label: label.to_owned(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        (name, f)
    }

    const ALL: [&str; 4] = ["Station output", "Daily mean, MW", "Day", "MW"];
    const NONE: [&str; 4] = ["", "", "", ""];

    /// Figures in the three themes, with and without each header part, axis label, legend
    /// style, tick labels and plot inset, named for the failure message.
    fn panel_fixtures() -> Vec<(&'static str, Figure)> {
        use LegendStyle::{Chips, Direct, Header, None};
        let six_by_four = (6.0, 4.0);
        let mut fixtures = vec![
            lines(
                "pigment, all",
                Theme::pigment(),
                six_by_four,
                Option::None,
                ALL,
            ),
            lines(
                "graphite, all",
                Theme::graphite(),
                six_by_four,
                Option::None,
                ALL,
            ),
            lines(
                "midnight, all",
                Theme::midnight(),
                six_by_four,
                Option::None,
                ALL,
            ),
            lines(
                "pigment, chips",
                Theme::pigment(),
                six_by_four,
                Some(Chips),
                ALL,
            ),
            lines(
                "graphite, header",
                Theme::graphite(),
                six_by_four,
                Some(Header),
                ALL,
            ),
            lines(
                "midnight, direct",
                Theme::midnight(),
                six_by_four,
                Some(Direct),
                ALL,
            ),
            lines(
                "pigment, bare",
                Theme::pigment(),
                six_by_four,
                Some(None),
                NONE,
            ),
            lines(
                "graphite, title and x label",
                Theme::graphite(),
                (5.0, 3.0),
                Some(None),
                ["Load", "", "Day", ""],
            ),
            lines(
                "midnight, subtitle and y label",
                Theme::midnight(),
                (4.0, 3.5),
                Some(Header),
                ["", "Daily mean", "", "MW"],
            ),
            lines(
                "pigment, scaled fonts",
                Theme::pigment().with_font_scale(1.7).unwrap(),
                six_by_four,
                Option::None,
                ALL,
            ),
            lines(
                "graphite, small",
                Theme::graphite(),
                (2.5, 2.0),
                Some(Direct),
                ALL,
            ),
            lines(
                "pigment, wrapped row",
                Theme::pigment(),
                (2.2, 3.0),
                Some(Header),
                ALL,
            ),
        ];

        // No tick labels at all.
        let mut f = Figure::new(3.0, 2.0, Theme::midnight()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .title("Load")
            .xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap();
        fixtures.push(("midnight, no tick labels", f));

        // A wide last x tick label that insets the plot, and long y tick labels.
        let mut f = Figure::new(4.0, 3.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xticks_labeled(
                &[0.0, 1.0, 2.0],
                vec!["Mon".into(), "Tue".into(), "Wednesday, week 32".into()],
            )
            .unwrap()
            .yticks_labeled(&[0.0, 10.0], vec!["none".into(), "ten thousand".into()])
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 2.0]),
                &[0.0, 10.0],
                LineOptions::default(),
            )
            .unwrap();
        fixtures.push(("pigment, inset by end labels", f));

        // Categories on a bar chart, with a log y axis elsewhere.
        let mut f = Figure::new(5.0, 3.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .title("Sales")
            .bar(
                vec![
                    "North region".into(),
                    "South".into(),
                    "Far east region".into(),
                ],
                &[3.0, 5.0, 2.0],
                BarOptions::default(),
            )
            .unwrap();
        fixtures.push(("graphite, categories", f));
        let mut f = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
        f.add_axes(
            ScaleKind::Linear,
            ScaleKind::Log {
                base: crate::LogBase::TEN,
            },
        )
        .unwrap()
        .ylabel("Count")
        .line(
            AxisData::Numbers(vec![1.0, 2.0, 3.0]),
            &[1.0, 120.0, 54_000.0],
            LineOptions::default(),
        )
        .unwrap();
        fixtures.push(("midnight, log y", f));
        fixtures
    }

    /// Every number of a layout as its bits, and every text, style and color, each line named,
    /// in a fixed order: two layouts are equal to the bit exactly when their lines are equal.
    #[derive(Default)]
    struct Bits(Vec<String>);

    impl Bits {
        fn num(&mut self, name: &str, v: f64) {
            self.0.push(format!("{name} {:016x}", v.to_bits()));
        }

        fn text(&mut self, line: String) {
            self.0.push(line);
        }

        fn area(&mut self, name: &str, a: &Area) {
            self.num(&format!("{name}.x"), a.x);
            self.num(&format!("{name}.y"), a.y);
            self.num(&format!("{name}.width"), a.width);
            self.num(&format!("{name}.height"), a.height);
        }

        fn label(&mut self, name: &str, label: &Label) {
            self.text(format!("{name} {:?} {:?}", label.text, label.style));
            self.num(&format!("{name}.x"), label.x);
            self.num(&format!("{name}.y"), label.y);
            self.num(&format!("{name}.width"), label.width());
        }

        fn labels<'a>(&mut self, name: &str, labels: impl IntoIterator<Item = &'a Label>) {
            for (i, label) in labels.into_iter().enumerate() {
                self.label(&format!("{name}[{i}]"), label);
            }
        }

        fn of(layout: &Layout) -> Vec<String> {
            let mut bits = Bits::default();
            bits.area("plot", &layout.plot);
            bits.num("header_bottom", layout.header_bottom);
            bits.labels("title", &layout.title);
            bits.labels("subtitle", &layout.subtitle);
            bits.num("legend.height", layout.legend.height());
            for (i, entry) in layout.legend.entries().iter().enumerate() {
                let name = format!("legend[{i}]");
                let swatch = &entry.swatch;
                bits.text(format!(
                    "{name}.swatch {:?} {:?}",
                    swatch.kind, swatch.color
                ));
                bits.num(&format!("{name}.swatch.x"), swatch.x);
                bits.num(&format!("{name}.swatch.center_y"), swatch.center_y);
                bits.num(&format!("{name}.swatch.width"), swatch.width);
                bits.num(&format!("{name}.swatch.height"), swatch.height);
                if let Some(chip) = &entry.chip {
                    bits.area(&format!("{name}.chip"), chip);
                }
                bits.label(&format!("{name}.label"), &entry.label);
            }
            for (i, DirectLabel { label, color, dot }) in layout.direct.iter().enumerate() {
                let name = format!("direct[{i}]");
                bits.text(format!("{name}.color {color:?}"));
                bits.labels(&format!("{name}.label"), label);
                if let Some(dot) = dot {
                    bits.num(&format!("{name}.dot.x"), dot.x);
                    bits.num(&format!("{name}.dot.y"), dot.y);
                    bits.num(&format!("{name}.dot.diameter"), dot.diameter);
                }
            }
            bits.labels("xlabel", &layout.xlabel);
            bits.labels("ylabel", &layout.ylabel);
            bits.labels("x_tick_labels", &layout.x_tick_labels);
            bits.labels("y_tick_labels", &layout.y_tick_labels);
            for (i, y) in layout.y_grid.iter().enumerate() {
                bits.num(&format!("y_grid[{i}]"), *y);
            }
            for (i, x) in layout.x_grid.iter().enumerate() {
                bits.num(&format!("x_grid[{i}]"), *x);
            }
            bits.num("baseline_y", layout.baseline_y);
            let frame = layout.frame;
            bits.area(
                "frame",
                &Area {
                    x: frame.x,
                    y: frame.y,
                    width: frame.width,
                    height: frame.height,
                },
            );
            bits.0
        }
    }

    fn assert_layout_bits_eq(placed: &Layout, expected: &Layout, name: &str) {
        assert_eq!(Bits::of(placed), Bits::of(expected), "{name}");
    }

    #[test]
    fn measure_then_place_equals_lay_out_for_every_corpus_style() {
        // For figures in the three themes, with and without title, subtitle, legend row,
        // direct labels, axis labels and tick labels: placing the measured bands into the plot
        // area that `lay_out` chose yields the same `Layout`, field by field, bit for bit.
        let fixtures = panel_fixtures();
        assert!(fixtures.len() >= 12);
        let lib = FontLibrary::global();
        for (name, figure) in &fixtures {
            with_layout_input(figure, lib, |input, marks| {
                let (resolved, layout) = plan(input, marks).expect(name);
                let bands = measure_panel(input, &resolved);
                let frame = Frame {
                    x: 0.0,
                    y: 0.0,
                    width: input.width,
                    height: input.height,
                };
                let placed = place_panel(input, &resolved, bands, frame, layout.plot);
                assert_layout_bits_eq(&placed, &layout, name);
            });
        }
    }

    #[test]
    fn the_bands_add_up_to_the_single_panel_layout() {
        // Above the plot: the padding, the header, the y label and the label rise; below it:
        // the x tick label row, the x label and the padding. Left of a plot that no x tick
        // label insets: the padding and the y tick label column.
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let lib = FontLibrary::global();
        for (name, figure) in &panel_fixtures() {
            with_layout_input(figure, lib, |input, marks| {
                let (resolved, layout) = plan(input, marks).expect(name);
                let bands = measure_panel(input, &resolved);
                let plot = layout.plot;
                assert!(
                    close(
                        plot.y,
                        PADDING + bands.header + bands.y_label_band + bands.label_rise
                    ),
                    "{name}"
                );
                assert!(
                    close(layout.header_bottom, PADDING + bands.header),
                    "{name}"
                );
                assert!(
                    close(
                        plot.bottom(),
                        input.height - PADDING - bands.x_band - bands.x_title_band
                    ),
                    "{name}"
                );
                assert!(plot.x >= PADDING + bands.y_tick_column - 1e-9, "{name}");
                assert!(
                    plot.right() <= input.width - PADDING - bands.direct_reserve + 1e-9,
                    "{name}"
                );
                assert_eq!(bands.x_spans.len(), layout.x_tick_labels.len(), "{name}");
                assert!(plot.height >= MIN_PLOT, "{name}");
            });
        }
        // Without an x tick label reaching past the padding, the column alone sets the left.
        let (name, figure) = lines(
            "pigment, all",
            Theme::pigment(),
            (6.0, 4.0),
            Option::None,
            ALL,
        );
        with_layout_input(&figure, lib, |input, marks| {
            let (resolved, layout) = plan(input, marks).expect(name);
            let bands = measure_panel(input, &resolved);
            assert!(bands.y_tick_column > Y_LABEL_GAP);
            assert!(
                close(layout.plot.x, PADDING + bands.y_tick_column),
                "{name}"
            );
        });
    }

    #[test]
    fn the_header_is_placed_inside_the_frame() {
        // The same bands placed in a frame moved right and down: the header moves with the
        // frame, everything placed around the plot stays where the plot is.
        let lib = FontLibrary::global();
        let (name, figure) = lines(
            "pigment, all",
            Theme::pigment(),
            (6.0, 4.0),
            Option::None,
            ALL,
        );
        with_layout_input(&figure, lib, |input, marks| {
            let (resolved, layout) = plan(input, marks).expect(name);
            let frame = Frame {
                x: 30.0,
                y: 20.0,
                width: input.width - 30.0,
                height: input.height - 20.0,
            };
            let bands = measure_panel(input, &resolved);
            let moved = place_panel(input, &resolved, bands, frame, layout.plot);
            let title = |l: &Layout| l.title.as_ref().map(|t| (t.x, t.y)).unwrap();
            let (x, y) = title(&layout);
            let (mx, my) = title(&moved);
            assert!((mx - (x + 30.0)).abs() < 1e-9 && (my - (y + 20.0)).abs() < 1e-9);
            assert!((moved.header_bottom - (layout.header_bottom + 20.0)).abs() < 1e-9);
            let row_y = |l: &Layout| l.legend.entries().first().map(|e| e.label.y).unwrap();
            assert!((row_y(&moved) - (row_y(&layout) + 20.0)).abs() < 1e-9);
            assert_eq!(moved.frame, frame);
            let ticks = |l: &Layout| -> Vec<u64> {
                l.y_tick_labels
                    .iter()
                    .chain(&l.x_tick_labels)
                    .flat_map(|t| [t.x.to_bits(), t.y.to_bits()])
                    .collect()
            };
            assert_eq!(ticks(&moved), ticks(&layout));
        });
    }
}
