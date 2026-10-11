//! Legend layout: measuring and wrapping the legend row, the geometry of chips, and the space
//! reserved on the right for direct labels.
//!
//! The legend row sits in the header, under the subtitle, left-aligned with the title. Its
//! entries are laid out left to right and wrap onto further rows within the inner width (the
//! canvas inside the outer padding); an entry wider than a whole row gets a row of its own and
//! is clipped by the canvas. All rows have the height of the tallest entry. The row depends only
//! on the canvas width, so it is measured once and then placed by each layout pass.
//!
//! A plain entry is its swatch and its label. A chip is a pill as tall as the row, holding a dot
//! and the label inside a horizontal padding. Both styles wrap the same way: an entry box is
//! the swatch, the label and the padding around them, and only the gap between boxes differs.

use super::{Area, BODY_LINE, Label, PADDING, Styles, baseline_in_line, measure, metrics};
use crate::legend::{Legend, RowStyle, Swatch};
use crate::{FontLibrary, Rgba, Theme};

/// The gap between a swatch and its label, in both row styles.
const SWATCH_GAP: f64 = 4.0;
/// The gap between two plain entries in a row.
const ENTRY_GAP: f64 = 12.0;
/// The gap between two chips in a row.
const CHIP_GAP: f64 = 6.0;
/// The padding left of a chip's dot and right of its label.
const CHIP_PADDING_X: f64 = 7.0;
/// The padding above and below a chip's line of text.
const CHIP_PADDING_Y: f64 = 3.0;
/// The diameter of a chip's dot, as a share of the label size.
const CHIP_DOT: f64 = 0.6;
/// The vertical gap between wrapped rows.
const ROW_GAP: f64 = 4.0;
/// The gap between the title or subtitle and the legend row.
pub(super) const ROW_TOP_GAP: f64 = 6.0;
/// The length of a line swatch, in line widths.
const LINE_SWATCH_LENGTH: f64 = 10.0;
/// The side of a square swatch, as a share of the label size.
const SQUARE_SWATCH_SIDE: f64 = 0.8;
/// The top of row `row` (counted from 0) of rows `row_height` tall, relative to the first
/// row's top.
fn row_top(row: usize, row_height: f64) -> f64 {
    row as f64 * (row_height + ROW_GAP)
}

/// A placed swatch: what it shows, in which color, and its box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SwatchBox {
    /// The swatch kind.
    pub kind: Swatch,
    /// The series color, alpha included.
    pub color: Rgba,
    /// The left edge.
    pub x: f64,
    /// The vertical center.
    pub center_y: f64,
    /// The width: the length of a line, the diameter of a dot, the side of a square.
    pub width: f64,
    /// The height: the line width of a line, the diameter of a dot, the side of a square.
    pub height: f64,
}

/// One entry of the legend row: its swatch and its label, and in a chips row the pill around
/// them.
#[derive(Debug, Clone)]
pub(crate) struct RowEntry {
    /// The swatch; in a chips row always a dot.
    pub swatch: SwatchBox,
    /// The label, in the body style, to the right of the swatch.
    pub label: Label,
    /// The box of the pill, as tall as the row, in a chips row; `None` in a plain row.
    pub chip: Option<Area>,
    /// The row the entry is on, counted from 0.
    row: usize,
}

/// The legend row: its entries, placed, on one or more rows of equal height. Empty, with
/// height 0, when the legend has no row entries.
#[derive(Debug, Clone, Default)]
pub(crate) struct LegendRow {
    entries: Vec<RowEntry>,
    /// The height of one row.
    row_height: f64,
    /// The number of rows; 0 without entries.
    rows: usize,
}

impl LegendRow {
    /// The row entries of `legend`, measured in the body font and wrapped within the inner
    /// width of a `width` pt canvas, with the first row's top at 0.
    ///
    /// A plain entry is its swatch, a 4 pt gap and its label, and entries are 12 pt apart.
    /// Swatches are sized as the marks are drawn: a line `10 ×` the drawn line width long and
    /// that width tall, a dot of the drawn marker diameter, a square of `0.8 ×` the label size.
    /// Plain rows are one body line tall (`1.35 ×` the label size), or as tall as the tallest
    /// swatch.
    ///
    /// A chip is a pill one body line plus `2 × 3` pt tall, holding a dot of `0.6 ×` the label
    /// size, a 4 pt gap and the label, with 7 pt of padding at each end; chips are 6 pt apart.
    ///
    /// Rows are 4 pt apart in both styles, and swatches and labels are centered in them.
    pub fn measure(width: f64, theme: &Theme, legend: &Legend, lib: &FontLibrary) -> LegendRow {
        if legend.is_empty() {
            return LegendRow::default();
        }
        let style = Styles::new(theme).body;
        let size = f64::from(style.size);
        let (ascent, descent) = metrics(lib, &style);
        let row_style = legend.row_style();
        let (padding_x, entry_gap, line) = match row_style {
            RowStyle::Plain => (0.0, ENTRY_GAP, BODY_LINE * size),
            RowStyle::Chips => (
                CHIP_PADDING_X,
                CHIP_GAP,
                BODY_LINE * size + 2.0 * CHIP_PADDING_Y,
            ),
        };
        let mut entries: Vec<RowEntry> = legend
            .row()
            .iter()
            .map(|entry| {
                let (kind, w, h) = swatch_size(theme, row_style, entry.swatch(), size);
                RowEntry {
                    swatch: SwatchBox {
                        kind,
                        color: entry.color(),
                        x: 0.0,
                        center_y: 0.0,
                        width: w,
                        height: h,
                    },
                    label: measure(lib, entry.label(), style),
                    chip: None,
                    row: 0,
                }
            })
            .collect();

        let row_height = entries
            .iter()
            .map(|entry| entry.swatch.height)
            .fold(line, f64::max);
        let right = width - PADDING;
        let (mut x, mut row, mut row_used) = (PADDING, 0_usize, false);
        for entry in &mut entries {
            let entry_width =
                padding_x + entry.swatch.width + SWATCH_GAP + entry.label.width() + padding_x;
            if row_used && x + entry_width > right {
                row += 1;
                x = PADDING;
            }
            let top = row_top(row, row_height);
            entry.row = row;
            entry.swatch.x = x + padding_x;
            entry.swatch.center_y = top + row_height / 2.0;
            entry.label.x = entry.swatch.x + entry.swatch.width + SWATCH_GAP;
            entry.label.y = baseline_in_line(top, row_height, ascent, descent);
            entry.chip = (row_style == RowStyle::Chips).then_some(Area {
                x,
                y: top,
                width: entry_width,
                height: row_height,
            });
            x += entry_width + entry_gap;
            row_used = true;
        }
        LegendRow {
            entries,
            row_height,
            rows: row + 1,
        }
    }

    /// This row moved down so that its first row's top is at `top`. Entries on rows whose top
    /// would be at or below `canvas_height` are dropped: they could not be seen, and dropping
    /// them keeps every coordinate near the canvas however many entries there are. The height
    /// stays that of all rows.
    pub(super) fn placed(&self, top: f64, canvas_height: f64) -> LegendRow {
        let entries = self
            .entries
            .iter()
            .filter(|entry| top + row_top(entry.row, self.row_height) < canvas_height)
            .map(|entry| {
                let mut entry = entry.clone();
                entry.swatch.center_y += top;
                entry.label.y += top;
                if let Some(chip) = &mut entry.chip {
                    chip.y += top;
                }
                entry
            })
            .collect();
        LegendRow { entries, ..*self }
    }

    /// The entries, in series order.
    pub fn entries(&self) -> &[RowEntry] {
        &self.entries
    }

    /// The height of all rows together, gaps included; 0 when there are no entries.
    pub fn height(&self) -> f64 {
        match self.rows.checked_sub(1) {
            Some(last) => row_top(last, self.row_height) + self.row_height,
            None => 0.0,
        }
    }

    /// Whether the row has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The swatch a row of `row_style` draws for an entry with `swatch`, and its width and height
/// for a label `size` pt tall.
///
/// A plain row keeps the entry's swatch at its drawn size, so each box is exactly what the
/// drawing fills or strokes; the drawn sizes are finite and capped, and so is the label size.
/// A chips row always draws a dot of `0.6 ×` the label size.
fn swatch_size(
    theme: &Theme,
    row_style: RowStyle,
    swatch: Swatch,
    size: f64,
) -> (Swatch, f64, f64) {
    match (row_style, swatch) {
        (RowStyle::Chips, _) => {
            let d = CHIP_DOT * size;
            (Swatch::Dot, d, d)
        }
        (RowStyle::Plain, Swatch::Line { .. }) => {
            let line_width = f64::from(theme.drawn_line_width());
            (swatch, LINE_SWATCH_LENGTH * line_width, line_width)
        }
        (RowStyle::Plain, Swatch::Dot) => {
            let d = f64::from(theme.drawn_marker_size());
            (swatch, d, d)
        }
        (RowStyle::Plain, Swatch::Square) => {
            let side = SQUARE_SWATCH_SIDE * size;
            (swatch, side, side)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Figure, LineOptions};
    use crate::pipeline::plan_figure;
    use crate::scene::Item;
    use crate::test_support::scene::assert_scene_bounded;
    use crate::{ScaleKind, Theme};

    /// A `w` by `h` in figure in `theme` with a title, a subtitle and a y label, and one line
    /// per label.
    fn figure(w: f64, h: f64, theme: Theme, labels: &[String]) -> Figure {
        let mut f = Figure::new(w, h, theme).unwrap();
        let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.title("Station output")
            .subtitle("Daily mean, MW")
            .ylabel("MW");
        for label in labels {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: label.clone(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        f
    }

    fn names(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("Station {i}")).collect()
    }

    fn rows_of(row: &LegendRow) -> Vec<usize> {
        row.entries().iter().map(|e| e.row).collect()
    }

    #[test]
    fn row_sits_under_the_subtitle_left_aligned_with_the_title() {
        let f = figure(6.0, 4.0, Theme::pigment(), &names(3));
        let layout = plan_figure(&f, FontLibrary::global()).1;
        let (title, subtitle) = (layout.title.unwrap(), layout.subtitle.unwrap());
        let row = &layout.legend;
        assert_eq!(rows_of(row), [0, 0, 0]);
        let first = &row.entries()[0];
        assert_eq!(first.swatch.x, title.x);
        assert!(first.label.y > subtitle.y);
        // The y label and the plot sit below the row.
        let ylabel = layout.ylabel.unwrap();
        let row_bottom = first.swatch.center_y - row.row_height / 2.0 + row.height();
        assert!(row_bottom < ylabel.y - f64::from(ylabel.layout.ascent()));
        assert!(row_bottom < layout.plot.y);
        assert!((layout.header_bottom - (row_bottom + 8.0)).abs() < 1e-9);
        // Entries keep their order, swatch, gap and label left to right.
        for pair in row.entries().windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!((a.label.x - (a.swatch.x + a.swatch.width + SWATCH_GAP)).abs() < 1e-9);
            let a_end = a.label.x + a.label.width();
            assert!((b.swatch.x - (a_end + ENTRY_GAP)).abs() < 1e-9);
        }
    }

    #[test]
    fn row_wraps_within_inner_width() {
        let theme = Theme::pigment();
        let width = 6.0 * 72.0;
        for n in [1, 3, 20, 60] {
            let f = figure(6.0, 4.0, theme.clone(), &names(n));
            let row = plan_figure(&f, FontLibrary::global()).1.legend;
            assert_eq!(row.entries().len(), n);
            let rows = rows_of(&row);
            // Rows only ever advance, by one at a time.
            assert!(rows.windows(2).all(|p| p[1] == p[0] || p[1] == p[0] + 1));
            for entry in row.entries() {
                assert!(entry.swatch.x >= PADDING);
                assert!(entry.label.x + entry.label.width() <= width - PADDING + 1e-9);
            }
            // Every entry that starts a row would not have fitted at the end of the row above.
            for pair in row.entries().windows(2) {
                if pair[1].row > pair[0].row {
                    let end = pair[0].label.x + pair[0].label.width() + ENTRY_GAP;
                    let w = pair[1].swatch.width + SWATCH_GAP + pair[1].label.width();
                    assert!(end + w > width - PADDING, "{n}: {end} + {w}");
                    assert_eq!(pair[1].swatch.x, PADDING);
                }
            }
            let last = rows.last().copied().unwrap_or(0);
            let expected = (last + 1) as f64 * row.row_height + last as f64 * ROW_GAP;
            assert!((row.height() - expected).abs() < 1e-9);
            if n == 20 {
                assert!(last >= 2, "20 entries wrap onto several rows");
            }
        }
    }

    /// The box of `entry`: its chip, or for a plain entry the swatch and label together.
    fn entry_box(entry: &RowEntry) -> (f64, f64) {
        match entry.chip {
            Some(chip) => (chip.x, chip.right()),
            None => (entry.swatch.x, entry.label.x + entry.label.width()),
        }
    }

    #[test]
    fn chips_wrap_like_the_header_row() {
        let width = 6.0 * 72.0;
        let chips_theme = Theme::midnight();
        assert_eq!(chips_theme.legend_style(), crate::LegendStyle::Chips);
        let header_theme = chips_theme
            .clone()
            .with_legend_style(crate::LegendStyle::Header)
            .unwrap();
        let size = f64::from(chips_theme.body_size() * chips_theme.font_scale());
        for n in [1, 3, 20, 60] {
            let chips = plan_figure(
                &figure(6.0, 4.0, chips_theme.clone(), &names(n)),
                FontLibrary::global(),
            )
            .1
            .legend;
            let header = plan_figure(
                &figure(6.0, 4.0, header_theme.clone(), &names(n)),
                FontLibrary::global(),
            )
            .1
            .legend;
            assert_eq!(chips.entries().len(), n);
            assert!(header.entries().iter().all(|e| e.chip.is_none()));
            // A chip is the pill around its dot and label, one body line plus 2 × 3 pt tall.
            assert_eq!(chips.row_height, BODY_LINE * size + 2.0 * CHIP_PADDING_Y);
            for entry in chips.entries() {
                let chip = entry.chip.unwrap();
                let top = row_top(entry.row, chips.row_height) + chips.entries()[0].chip.unwrap().y;
                assert!((chip.y - top).abs() < 1e-9 && chip.height == chips.row_height);
                assert_eq!(entry.swatch.kind, Swatch::Dot);
                assert!((entry.swatch.width - CHIP_DOT * size).abs() < 1e-9);
                assert!((entry.swatch.x - (chip.x + CHIP_PADDING_X)).abs() < 1e-9);
                assert!(
                    (entry.label.x - (entry.swatch.x + entry.swatch.width + SWATCH_GAP)).abs()
                        < 1e-9
                );
                let label_end = entry.label.x + entry.label.width();
                assert!((chip.right() - (label_end + CHIP_PADDING_X)).abs() < 1e-9);
                assert!((entry.swatch.center_y - (chip.y + chip.height / 2.0)).abs() < 1e-9);
            }
            // The same wrapping rule for both styles, each with its own entry gap: rows only
            // advance by one, every box stays within the inner width, and an entry starts a
            // row only when it would not have fitted at the end of the row above.
            for (row, gap) in [(&chips, CHIP_GAP), (&header, ENTRY_GAP)] {
                let rows = rows_of(row);
                assert!(rows.windows(2).all(|p| p[1] == p[0] || p[1] == p[0] + 1));
                for pair in row.entries().windows(2) {
                    let ((_, a_end), (b_start, b_end)) = (entry_box(&pair[0]), entry_box(&pair[1]));
                    assert!(b_start >= PADDING && b_end <= width - PADDING + 1e-9);
                    if pair[1].row > pair[0].row {
                        assert!(a_end + gap + (b_end - b_start) > width - PADDING);
                        assert_eq!(b_start, PADDING);
                    } else {
                        assert!((b_start - (a_end + gap)).abs() < 1e-9);
                    }
                }
                let last = rows.last().copied().unwrap_or(0);
                let expected = (last + 1) as f64 * row.row_height + last as f64 * ROW_GAP;
                assert!((row.height() - expected).abs() < 1e-9);
            }
            if n == 20 {
                assert!(chips.rows >= 3, "20 chips wrap onto several rows");
            }
        }
        // A chip wider than a whole row gets a row of its own, as a header entry does.
        let long = "Long legend label ".repeat(17);
        let labels = ["A".to_owned(), long, "B".to_owned()];
        let row = plan_figure(
            &figure(6.0, 4.0, chips_theme, &labels),
            FontLibrary::global(),
        )
        .1
        .legend;
        assert_eq!(rows_of(&row), [0, 1, 2]);
        assert_eq!(row.entries()[1].chip.unwrap().x, PADDING);
    }

    #[test]
    fn an_entry_wider_than_a_row_gets_its_own_row() {
        let long = "Long legend label ".repeat(17);
        let labels = ["A".to_owned(), long, "B".to_owned()];
        let f = figure(6.0, 4.0, Theme::pigment(), &labels);
        let row = plan_figure(&f, FontLibrary::global()).1.legend;
        assert_eq!(rows_of(&row), [0, 1, 2]);
        let wide = &row.entries()[1];
        assert_eq!(wide.swatch.x, PADDING);
        assert!(wide.label.x + wide.label.width() > 6.0 * 72.0);
    }

    #[test]
    fn swatch_sizes_follow_the_theme() {
        let theme = Theme::graphite()
            .with_legend_style(crate::LegendStyle::Header)
            .unwrap();
        let f = figure(6.0, 4.0, theme.clone(), &names(1));
        let row = plan_figure(&f, FontLibrary::global()).1.legend;
        let swatch = row.entries()[0].swatch;
        let line_width = f64::from(theme.line_width());
        assert!((swatch.width - 10.0 * line_width).abs() < 1e-9);
        assert!((swatch.height - line_width).abs() < 1e-9);
        assert_eq!(
            row.row_height,
            BODY_LINE * f64::from(theme.body_size() * theme.font_scale())
        );
    }

    #[test]
    fn many_entries_never_make_a_negative_plot() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for scale in [1.0, 10.0] {
                let theme = theme.with_font_scale(scale).unwrap();
                for (w, h, n) in [(1.0, 1.0, 50), (6.0, 4.0, 400), (2.0, 1.0, 2000)] {
                    let f = figure(w, h, theme.clone(), &names(n));
                    let layout = plan_figure(&f, FontLibrary::global()).1;
                    let plot = layout.plot;
                    assert!(plot.width >= 1.0 && plot.height >= 1.0, "{plot:?}");
                    assert!(
                        plot.y >= 0.0 && plot.bottom() <= h * 72.0 + 1e-9,
                        "{plot:?}"
                    );
                    assert!(layout.legend.height().is_finite());
                    // Only rows that start on the canvas are kept.
                    for entry in layout.legend.entries() {
                        assert!(entry.swatch.center_y - layout.legend.row_height / 2.0 < h * 72.0);
                    }
                    assert_scene_bounded(&f.to_scene(FontLibrary::global()));
                }
            }
        }
    }

    /// The bounds of a path item: `(left, top, right, bottom)`.
    fn bounds(item: &Item) -> (f64, f64, f64, f64) {
        let path = match item {
            Item::Fill { path, .. } | Item::Stroke { path, .. } => path,
            Item::Markers { shape, .. } => shape,
            other => panic!("not a path item: {other:?}"),
        };
        let (min, max) = path.bounds().unwrap();
        let f = f64::from;
        (f(min.x), f(min.y), f(max.x), f(max.y))
    }

    /// A 6 by 4 in figure without a title, so its legend row starts at the top padding, with
    /// one labelled dashed line or one labelled scatter.
    fn huge_token_figure(theme: Theme, scatter: bool) -> Figure {
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let (x, y) = (AxisData::Numbers(vec![0.0, 1.0]), [1.0, 2.0]);
        if scatter {
            let options = crate::model::ScatterOptions {
                label: "Points".into(),
                ..crate::model::ScatterOptions::default()
            };
            axes.scatter(x, &y, options).unwrap();
        } else {
            let options = LineOptions {
                label: "Dashed".into(),
                dash: crate::Dash::Dashed,
                ..LineOptions::default()
            };
            axes.line(x, &y, options).unwrap();
        }
        f
    }

    /// The legend's one swatch box, and the marks and legend items of the scene, which must
    /// hold chrome, marks, legend and baseline groups.
    fn swatch_and_items(f: &Figure) -> (SwatchBox, Vec<Item>, Vec<Item>) {
        let layout = plan_figure(f, FontLibrary::global()).1;
        let [entry] = layout.legend.entries() else {
            panic!("one legend entry");
        };
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let [
            _,
            Item::Group { items: marks, .. },
            Item::Group { items: legend, .. },
            _,
        ] = scene.items.as_slice()
        else {
            panic!("chrome, marks, legend and baseline groups");
        };
        (entry.swatch, marks.clone(), legend.clone())
    }

    #[test]
    fn huge_swatch_tokens_match_the_drawing() {
        let (width, height) = (6.0 * 72.0, 4.0 * 72.0);
        // 12 000 is above the drawn cap; f32::MAX is the largest token a theme accepts.
        for token in [12_000.0, f64::from(f32::MAX)] {
            let theme = Theme::pigment()
                .with_line_width(token)
                .unwrap()
                .with_marker_size(token)
                .unwrap();
            let (stroke, diameter) = (
                f64::from(theme.drawn_line_width()),
                f64::from(theme.drawn_marker_size()),
            );
            assert_eq!((stroke, diameter), (1.0e4, 1.0e4));

            // The line swatch is exactly as wide as the stroke of the line mark.
            let (line, marks, legend) = swatch_and_items(&huge_token_figure(theme.clone(), false));
            assert_eq!((line.width, line.height), (10.0 * stroke, stroke));
            let stroke_of = |items: &[Item]| {
                items.iter().find_map(|item| match item {
                    Item::Stroke { style, .. } => Some(f64::from(style.width)),
                    _ => None,
                })
            };
            assert_eq!(stroke_of(&legend), Some(line.height));
            assert_eq!(stroke_of(&marks), Some(line.height));
            let (_, y0, _, y1) = bounds(&legend[0]);
            assert!(y0 == y1 && (y0 - line.center_y).abs() < 1e-3);

            // The dot swatch is exactly as wide as the scatter markers.
            let (dot, marks, legend) = swatch_and_items(&huge_token_figure(theme, true));
            assert_eq!((dot.width, dot.height), (diameter, diameter));
            let (x0, y0, x1, y1) = bounds(&legend[0]);
            assert!((x1 - x0 - dot.width).abs() < 1.0 && (y1 - y0 - dot.height).abs() < 1.0);
            assert!(((y0 + y1) / 2.0 - dot.center_y).abs() < 1.0);
            let markers = marks
                .iter()
                .find(|item| matches!(item, Item::Markers { .. }))
                .unwrap();
            let (x0, _, x1, _) = bounds(markers);
            assert!(
                (x1 - x0 - dot.width).abs() < 1.0,
                "{} vs {}",
                x1 - x0,
                dot.width
            );

            // Each box starts on the canvas, at the row top.
            for b in [line, dot] {
                let top = b.center_y - b.height / 2.0;
                assert!((top - PADDING).abs() < 1e-9, "{token}: {b:?}");
                assert!(b.x < width && top < height, "{token}: {b:?}");
            }
        }
    }
}
