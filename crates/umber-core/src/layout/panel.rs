//! The layout of one panel in two steps around its plot area.
//!
//! [`measure_panel`] measures the panel's texts for resolved scales and the bands they need
//! around the plot area: the header and the y label above it, the y tick label column left of
//! it, the room direct labels reserve right of it, and the x tick label and x label rows below
//! it. Choosing the plot area from these bands is left to the caller: the grid solve
//! ([`super::grid::solve_grid`]) does it for every panel of a figure at once. [`place_panel`]
//! then places the panel's texts, gridlines and legend for the [`PanelSlot`] the solve chose.

use super::inset::TickSpan;
use super::legend::{self, LegendRow};
use super::{
    AXIS_LABEL_GAP, Area, BODY_LINE, END_LABEL_SHARE, HEADER_GAP, Label, Layout, LayoutInput,
    PADDING, PlacedBaseline, Styles, TITLE_GAP, TITLE_LINE, X_LABEL_GAP, Y_LABEL_CENTER,
    Y_LABEL_GAP, baseline_in_line, figure_height_hint, measure, measure_ticks, metrics,
};
use crate::resolve::{AxisTick, NormalizedPos, Resolved, ValueAxis, coincides};
use crate::{Align, Figure, FontLibrary, Result};

/// The figure title size as a multiple of the theme's panel title size.
pub(crate) const FIGURE_TITLE_SCALE: f32 = 1.5;

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

/// The widths a panel is measured for: its legend row wraps within `row_left..row_right`, and
/// its direct labels reserve at most `reserve_cap` right of the plot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PanelSpan {
    /// Where the legend row starts.
    pub row_left: f64,
    /// The furthest right a legend row entry may end before the row wraps.
    pub row_right: f64,
    /// The most room direct labels may reserve right of the plot.
    pub reserve_cap: f64,
}

/// Where a panel's header goes: stacked down from `top`, its title and subtitle starting at
/// `left`, its legend row starting there too and wrapping within `left..right`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeaderBox {
    /// Where the title, the subtitle and the legend row start.
    pub left: f64,
    /// The furthest right a legend row entry may end before the row wraps.
    pub right: f64,
    /// The top of the header.
    pub top: f64,
}

/// Where a panel goes on the canvas: its plot area, its header box, the baseline of its x axis
/// label and its frame, the part of the canvas that belongs to it (the whole canvas for a
/// figure of one axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PanelSlot {
    /// The plot area.
    pub plot: Area,
    /// The header box.
    pub header: HeaderBox,
    /// The baseline of the x axis label: under the panel's own x tick labels for a figure of
    /// one axes, and on a grid at the bottom of its row's band below the plots, so that the x
    /// axis labels of one grid row line up.
    pub xlabel_baseline: f64,
    /// The frame.
    pub frame: Frame,
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
    /// Measures the shared title, subtitle and legend stack for a panel or a figure.
    fn measure(
        title: &str,
        subtitle: &str,
        styles: &Styles,
        row: &LegendRow,
        lib: &FontLibrary,
    ) -> Header {
        let title = (!title.is_empty()).then(|| {
            let (ascent, descent) = metrics(lib, &styles.title);
            HeaderLine {
                label: measure(lib, title, styles.title),
                line: TITLE_LINE * f64::from(styles.title.size),
                ascent,
                descent,
            }
        });
        let subtitle = (!subtitle.is_empty()).then(|| {
            let (ascent, descent) = metrics(lib, &styles.body);
            HeaderLine {
                label: measure(lib, subtitle, styles.body),
                line: BODY_LINE * f64::from(styles.body.size),
                ascent,
                descent,
            }
        });
        Header {
            title,
            subtitle,
            row: (!row.is_empty()).then(|| row.height()),
        }
    }

    /// The header parts stacked from `top` down: the title line, the title gap and the subtitle
    /// line, the row gap and the legend row, then the header gap, each only when the header has
    /// the parts it separates. The positions are summed top down in this order, so they are
    /// the same to the bit wherever the header is stacked from the same top.
    fn stack(&self, top: f64) -> Stacked {
        self.stack_with_row(top, self.row)
    }

    /// [`Header::stack`] with a legend row `row` tall in place of this header's own (none when
    /// `None`).
    fn stack_with_row(&self, top: f64, row: Option<f64>) -> Stacked {
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
        let row_top = match row {
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
        if title.is_some() || subtitle.is_some() || row.is_some() {
            y += HEADER_GAP;
        }
        Stacked {
            title,
            subtitle,
            row: row_top,
            bottom: y,
        }
    }

    /// Where the header stacked from `top` ends, the gap below a non-empty header included;
    /// `top` itself when the header is empty.
    pub(super) fn bottom(&self, top: f64) -> f64 {
        self.stack(top).bottom
    }

    /// The header's height with the legend row `row` in place of its own, the gap below a
    /// non-empty header included; 0 when it is empty.
    fn height_with_row(&self, row: &LegendRow) -> f64 {
        let height = (!row.is_empty()).then(|| row.height());
        self.stack_with_row(0.0, height).bottom
    }

    /// The hint of the error for a figure too short for the plot area, naming this header's
    /// parts, a bottom legend and the figure header above it, when present.
    pub(super) fn height_hint(&self, figure_header: bool, bottom_legend: bool) -> &'static str {
        figure_height_hint(
            self.title.is_some(),
            self.subtitle.is_some(),
            self.row.is_some() || bottom_legend,
            figure_header,
        )
    }

    /// The header stacked from `top` down with a legend row `row` tall (none when `None`), its
    /// labels aligned within the header box. The row has its own alignment.
    fn place(self, box_: HeaderBox, align: Align, row: Option<f64>) -> PlacedHeader {
        let stacked = self.stack_with_row(box_.top, row);
        let at = |line: HeaderLine, baseline: f64| Label {
            x: align.position(line.label.width(), box_.left, box_.right),
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

/// The figure-wide header, placed inside the canvas padding above the grid.
#[derive(Debug, Clone)]
pub(crate) struct FigureHeader {
    /// The title at one and a half times the panel title size.
    pub title: Option<Label>,
    /// The subtitle in the body style.
    pub subtitle: Option<Label>,
    /// The merged legend, with each wrapped row aligned with the figure title.
    pub legend: LegendRow,
    /// Where the grid starts, including the gap below a non-empty header.
    pub bottom: f64,
}

impl FigureHeader {
    /// Measures and places the header independently of the grid's columns.
    pub fn measure(
        figure: &Figure,
        legend: &super::MeasuredLegend<'_>,
        lib: &FontLibrary,
    ) -> FigureHeader {
        let theme = figure.theme();
        let mut styles = Styles::new(theme);
        styles.title.size =
            super::scaled(theme.title_size() * FIGURE_TITLE_SCALE, theme.font_scale());
        let box_ = HeaderBox {
            left: PADDING,
            right: figure.width_in() * 72.0 - PADDING,
            top: PADDING,
        };
        let row = legend.row_within(box_.left, box_.right);
        let header = Header::measure(
            figure.title_text(),
            figure.subtitle_text(),
            &styles,
            &row,
            lib,
        );
        let row_height = (!row.is_empty()).then(|| row.height());
        let placed = header.place(box_, theme.title_align(), row_height);
        FigureHeader {
            title: placed.title,
            subtitle: placed.subtitle,
            legend: row
                .aligned(box_.left, box_.right, theme.title_align())
                .placed(placed.row),
            bottom: placed.bottom,
        }
    }

    /// Checks that every merged entry fits the inner canvas width. A figure without panels
    /// must still have room for its header and the bottom padding.
    pub fn check(&self, figure: &Figure, context: &'static str) -> Result<()> {
        self.legend
            .check_width(figure.width_in() * 72.0 - 2.0 * PADDING, context)?;
        if figure.panels().next().is_none()
            && self.bottom > PADDING
            && self.bottom + PADDING > figure.height_in() * 72.0
        {
            return Err(super::figure_too_short(
                context,
                figure.height_in() * 72.0,
                self.bottom + PADDING,
                figure_height_hint(
                    self.title.is_some(),
                    self.subtitle.is_some(),
                    !self.legend.is_empty(),
                    false,
                ),
            ));
        }
        Ok(())
    }
}

/// Everything one panel needs around its plot area, measured for resolved scales, and the
/// measured texts that [`place_panel`] places.
#[derive(Debug, Clone)]
pub(crate) struct PanelBands {
    /// The y axis label above the top tick label row; 0 without one.
    pub y_label_band: f64,
    /// How far the top y tick label rises above the plot; 0 without y tick labels.
    pub label_rise: f64,
    /// The widest y tick label and [`Y_LABEL_GAP`]; 0 without y tick labels.
    pub y_tick_column: f64,
    /// The room direct labels reserve right of the plot, at most the panel's
    /// [`PanelSpan::reserve_cap`]; 0 without direct labels.
    pub direct_reserve: f64,
    /// The column legend and its gap to the plot; uncapped.
    pub right_legend: f64,
    /// The bottom legend and its gap to the x axis.
    pub bottom_legend: f64,
    /// The x tick label row and its gap; 0 without x tick labels.
    pub x_band: f64,
    /// The x axis label row and its gap; 0 without an x axis label.
    pub x_title_band: f64,
    /// How far the bottom y tick label reaches below the plot; 0 without y tick labels.
    pub label_drop: f64,
    /// The x tick labels as normalized positions and half widths, in tick order.
    pub x_spans: Vec<TickSpan>,
    /// The header, measured, with the legend row wrapped as [`measure_panel`] was asked.
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
    pub(super) body_ascent: f64,
    /// The descent of the body style's primary font.
    pub(super) body_descent: f64,
}

impl PanelBands {
    /// The header: title, subtitle, legend row `row` and the gap below them; 0 when empty.
    pub(super) fn header_with_row(&self, row: &LegendRow) -> f64 {
        self.heading.height_with_row(row)
    }

    /// Everything above the plot: the header, the y label and the rise of the top y tick label.
    pub(super) fn above_with_row(&self, row: &LegendRow) -> f64 {
        self.header_with_row(row) + self.y_label_band + self.label_rise
    }

    /// Everything below the plot: the x tick label row and the x label row, or the bottom y
    /// tick label reaching below the plot when that reaches further.
    pub(super) fn below(&self) -> f64 {
        self.axis_below() + self.bottom_legend
    }

    /// The room below the plot for the axes alone, before a bottom legend.
    pub(super) fn axis_below(&self) -> f64 {
        (self.x_band + self.x_title_band).max(self.label_drop)
    }

    /// The widest reserve beside the plot: direct labels or the column legend.
    pub(super) fn right(&self) -> f64 {
        self.direct_reserve.max(self.right_legend)
    }
}

/// Measures the texts of the panel of `input` for the scales of `resolved`, and the bands they
/// need around the plot area, for the widths in `span`.
pub(crate) fn measure_panel(
    input: &LayoutInput<'_>,
    resolved: &Resolved,
    span: PanelSpan,
    show_labels: [bool; 2],
) -> PanelBands {
    let LayoutInput {
        theme,
        axes,
        legend,
        lib,
        ..
    } = *input;
    let styles = Styles::new(theme);

    let row = legend.header_within(span.row_left, span.row_right);
    let heading = Header::measure(axes.title_text(), axes.subtitle_text(), &styles, &row, lib);

    // Tick labels.
    let (tick_ascent, tick_descent) = metrics(lib, &styles.tick);
    let tick_size = f64::from(styles.tick.size);
    let y_labels = if show_labels[1] {
        measure_ticks(lib, &resolved.y.ticks, styles.tick)
    } else {
        Vec::new()
    };
    let x_labels = if show_labels[0] {
        measure_ticks(lib, &resolved.x.ticks, styles.tick)
    } else {
        Vec::new()
    };
    let y_label_width = y_labels
        .iter()
        .map(|(_, label)| label.width())
        .fold(0.0, f64::max);

    // How far the y tick label of a tick at the plot top reaches above it, and that of a tick
    // at the plot bottom below it.
    let (label_rise, label_drop) = if y_labels.is_empty() {
        (0.0, 0.0)
    } else {
        (
            (tick_ascent - Y_LABEL_CENTER * tick_size).max(0.0),
            (Y_LABEL_CENTER * tick_size + tick_descent).max(0.0),
        )
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
        y_label_band,
        label_rise,
        y_tick_column,
        direct_reserve: legend.direct.reserve(span.reserve_cap),
        right_legend: legend.right_reserve(),
        bottom_legend: legend.bottom_reserve(span.row_left, span.row_right),
        x_band,
        x_title_band,
        label_drop,
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
        body_descent,
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
/// `resolved`, placed in `slot`: the tick labels, axis labels, gridlines, baseline and direct
/// labels around and on the plot area, and the header stacked down from the top of the header
/// box, its legend row wrapped within the box.
pub(crate) fn place_panel(
    input: &LayoutInput<'_>,
    resolved: &Resolved,
    bands: PanelBands,
    slot: &PanelSlot,
) -> Layout {
    let PanelSlot {
        plot,
        header: header_box,
        xlabel_baseline,
        frame,
    } = *slot;
    let axis_below = bands.axis_below();
    let PanelBands {
        label_rise,
        heading,
        mut y_labels,
        y_label_width,
        mut x_labels,
        mut xlabel,
        mut ylabel,
        tick_ascent,
        tick_size,
        ..
    } = bands;
    let position = input.legend.position;
    let row = if position == crate::LegendPosition::TopRight {
        input
            .legend
            .header_within(plot.x, plot.right())
            .aligned(plot.x, plot.right(), Align::Right)
    } else {
        input
            .legend
            .header_within(header_box.left, header_box.right)
    };
    let row_height = (!row.is_empty()).then(|| row.height());
    let header = heading.place(header_box, input.theme.title_align(), row_height);
    let row = match position {
        crate::LegendPosition::TopLeft | crate::LegendPosition::TopRight => row.placed(header.row),
        crate::LegendPosition::Right => input
            .legend
            .column(plot.right() + super::placement::RIGHT_GAP)
            .placed(plot.y),
        crate::LegendPosition::Bottom => {
            let axis_bottom = if xlabel.is_some() {
                (xlabel_baseline + bands.body_descent).max(plot.bottom() + axis_below)
            } else {
                plot.bottom() + axis_below
            };
            input
                .legend
                .row_within(header_box.left, header_box.right)
                .placed(axis_bottom + super::placement::BOTTOM_GAP)
        }
        // The final resolved mark geometry chooses the position after layout.
        crate::LegendPosition::Best => input
            .legend
            .column(plot.x + 2.0 * super::placement::BEST_PADDING)
            .placed(plot.y + 2.0 * super::placement::BEST_PADDING),
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
        label.y = xlabel_baseline;
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
    // The baseline is drawn in place of the gridline it coincides with.
    let off_baseline = |axis: ValueAxis| {
        move |tick: &&AxisTick| baseline.axis != axis || !coincides(tick.pos, baseline.pos)
    };
    // A category axis has no gridlines, along y as along x.
    let y_grid = if resolved.y.is_category() {
        Vec::new()
    } else {
        resolved
            .y
            .ticks
            .iter()
            .filter(off_baseline(ValueAxis::Y))
            .map(|tick| plot.y_at(tick.pos))
            .collect()
    };
    let x_grid = if resolved.x.is_category() {
        Vec::new()
    } else {
        resolved
            .x
            .ticks
            .iter()
            .filter(off_baseline(ValueAxis::X))
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
        baseline: PlacedBaseline::on(&plot, baseline),
        frame,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{DirectLabel, MIN_PLOT};
    use super::*;
    use crate::layout::GridInput;
    use crate::model::{AxisData, BarOptions, Figure, LineOptions};
    use crate::pipeline::{plan_figure, with_layout_input};
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
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
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
            bits.text(format!("baseline.axis {:?}", layout.baseline.axis));
            bits.num("baseline.at", layout.baseline.at);
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

    /// The slot of the only axes of a figure of one cell, with the plot area `plot`.
    fn single_slot(grid: &GridInput<'_>, plot: Area, bands: &PanelBands) -> PanelSlot {
        PanelSlot {
            plot,
            xlabel_baseline: plot.bottom() + bands.x_band + AXIS_LABEL_GAP + bands.body_ascent,
            header: HeaderBox {
                left: PADDING,
                right: grid.width - PADDING,
                top: PADDING,
            },
            frame: Frame {
                x: 0.0,
                y: 0.0,
                width: grid.width,
                height: grid.height,
            },
        }
    }

    #[test]
    fn measure_then_place_equals_the_planned_layout_for_every_corpus_style() {
        // For figures in the three themes, with and without title, subtitle, legend row,
        // direct labels, axis labels and tick labels: placing the measured bands into the plot
        // area that the figure's layout chose yields the same `Layout`, field by field, bit
        // for bit.
        let fixtures = panel_fixtures();
        assert!(fixtures.len() >= 12);
        let lib = FontLibrary::global();
        for (name, figure) in &fixtures {
            let (resolved, layout) = plan_figure(figure, (0, 0), lib);
            with_layout_input(figure, (0, 0), lib, |input, grid| {
                let bands =
                    measure_panel(input, &resolved, grid.span(input.axes.cells()), [true; 2]);
                // A figure of one axes has the canvas as its frame.
                let slot = single_slot(grid, layout.plot, &bands);
                assert_eq!(layout.frame, slot.frame, "{name}");
                let placed = place_panel(input, &resolved, bands, &slot);
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
            let (resolved, layout) = plan_figure(figure, (0, 0), lib);
            with_layout_input(figure, (0, 0), lib, |input, grid| {
                let bands =
                    measure_panel(input, &resolved, grid.span(input.axes.cells()), [true; 2]);
                let header = bands.heading.bottom(0.0);
                let plot = layout.plot;
                assert!(
                    close(
                        plot.y,
                        PADDING + header + bands.y_label_band + bands.label_rise
                    ),
                    "{name}"
                );
                assert!(close(layout.header_bottom, PADDING + header), "{name}");
                assert!(
                    close(
                        plot.bottom(),
                        grid.height - PADDING - bands.x_band - bands.x_title_band
                    ),
                    "{name}"
                );
                assert!(plot.x >= PADDING + bands.y_tick_column - 1e-9, "{name}");
                assert!(
                    plot.right() <= grid.width - PADDING - bands.direct_reserve + 1e-9,
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
        let (resolved, layout) = plan_figure(&figure, (0, 0), lib);
        with_layout_input(&figure, (0, 0), lib, |input, grid| {
            let bands = measure_panel(input, &resolved, grid.span(input.axes.cells()), [true; 2]);
            assert!(bands.y_tick_column > Y_LABEL_GAP);
            assert!(
                close(layout.plot.x, PADDING + bands.y_tick_column),
                "{name}"
            );
        });
    }

    #[test]
    fn the_header_is_placed_in_its_box() {
        // The same bands placed with the header box moved right and down: the header moves
        // with the box, everything placed around the plot stays where the plot is.
        let lib = FontLibrary::global();
        let (name, figure) = lines(
            "pigment, all",
            Theme::pigment(),
            (6.0, 4.0),
            Option::None,
            ALL,
        );
        let (resolved, layout) = plan_figure(&figure, (0, 0), lib);
        with_layout_input(&figure, (0, 0), lib, |input, grid| {
            let bands = measure_panel(input, &resolved, grid.span(input.axes.cells()), [true; 2]);
            let mut slot = single_slot(grid, layout.plot, &bands);
            slot.header.left += 30.0;
            slot.header.right += 30.0;
            slot.header.top += 20.0;
            let moved = place_panel(input, &resolved, bands, &slot);
            let title = |l: &Layout| l.title.as_ref().map(|t| (t.x, t.y)).unwrap();
            let (x, y) = title(&layout);
            let (mx, my) = title(&moved);
            assert!(
                (mx - (x + 30.0)).abs() < 1e-9 && (my - (y + 20.0)).abs() < 1e-9,
                "{name}"
            );
            assert!((moved.header_bottom - (layout.header_bottom + 20.0)).abs() < 1e-9);
            let row_at = |l: &Layout| {
                l.legend
                    .entries()
                    .first()
                    .map(|e| (e.swatch.x, e.label.y))
                    .unwrap()
            };
            let (rx, ry) = row_at(&layout);
            let (mrx, mry) = row_at(&moved);
            assert!((mrx - (rx + 30.0)).abs() < 1e-9 && (mry - (ry + 20.0)).abs() < 1e-9);
            assert_eq!(moved.frame, slot.frame);
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
