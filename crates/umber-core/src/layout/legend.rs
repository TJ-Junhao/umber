//! Legend layout: measuring and wrapping the legend row, the geometry of chips, and the space
//! reserved on the right for direct labels.
//!
//! The legend row sits in the header, under the subtitle, left-aligned with the title. Its
//! entries are laid out left to right and wrap onto further rows within the inner width (the
//! canvas inside the outer padding); an entry wider than a whole row gets a row of its own and
//! is clipped by the canvas. All rows have the height of the tallest entry. The row depends only
//! on the canvas width, so it is measured once and then placed by each layout pass.

use super::{BODY_LINE, Label, PADDING, Styles, baseline_in_line, measure, metrics};
use crate::legend::{Legend, Swatch};
use crate::{FontLibrary, Rgba, Theme};

/// The gap between a swatch and its label.
const SWATCH_GAP: f64 = 4.0;
/// The gap between two entries in a row.
const ENTRY_GAP: f64 = 12.0;
/// The vertical gap between wrapped rows.
const ROW_GAP: f64 = 4.0;
/// The gap between the title or subtitle and the legend row.
pub(super) const ROW_TOP_GAP: f64 = 6.0;
/// The length of a line swatch, in line widths.
const LINE_SWATCH_LENGTH: f64 = 10.0;
/// The side of a square swatch, as a share of the label size.
const SQUARE_SWATCH_SIDE: f64 = 0.8;
/// The largest swatch extent, whatever the theme says.
const MAX_SWATCH: f64 = 1.0e5;

/// `v` as a swatch extent: finite and within `0..=MAX_SWATCH`.
fn swatch_extent(v: f64) -> f64 {
    if v.is_finite() && v > 0.0 {
        v.min(MAX_SWATCH)
    } else {
        0.0
    }
}

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

/// One entry of the legend row: its swatch and its label.
#[derive(Debug, Clone)]
pub(crate) struct RowEntry {
    /// The swatch.
    pub swatch: SwatchBox,
    /// The label, in the body style, to the right of the swatch.
    pub label: Label,
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
    /// Each entry is its swatch, a 4 pt gap and its label; entries are 12 pt apart and rows
    /// 4 pt apart. Swatches: a line `10 × line_width` long in the series dash, a dot of
    /// `marker_size`, a square of `0.8 ×` the label size. Rows are one body line tall (`1.35 ×`
    /// the label size), or as tall as the tallest swatch, and swatches and labels are centered
    /// in them.
    pub fn measure(width: f64, theme: &Theme, legend: &Legend, lib: &FontLibrary) -> LegendRow {
        if legend.is_empty() {
            return LegendRow::default();
        }
        let style = Styles::new(theme).body;
        let size = f64::from(style.size);
        let (ascent, descent) = metrics(lib, &style);
        let mut entries: Vec<RowEntry> = legend
            .row()
            .iter()
            .map(|entry| {
                let (w, h) = match entry.swatch() {
                    Swatch::Line { .. } => {
                        let line_width = f64::from(theme.line_width());
                        (
                            swatch_extent(LINE_SWATCH_LENGTH * line_width),
                            swatch_extent(line_width),
                        )
                    }
                    Swatch::Dot => {
                        let d = swatch_extent(f64::from(theme.marker_size()));
                        (d, d)
                    }
                    Swatch::Square => {
                        let side = swatch_extent(SQUARE_SWATCH_SIDE * size);
                        (side, side)
                    }
                };
                RowEntry {
                    swatch: SwatchBox {
                        kind: entry.swatch(),
                        color: entry.color(),
                        x: 0.0,
                        center_y: 0.0,
                        width: w,
                        height: h,
                    },
                    label: measure(lib, entry.label(), style),
                    row: 0,
                }
            })
            .collect();

        let row_height = entries
            .iter()
            .map(|entry| entry.swatch.height)
            .fold(BODY_LINE * size, f64::max);
        let right = width - PADDING;
        let (mut x, mut row, mut row_used) = (PADDING, 0_usize, false);
        for entry in &mut entries {
            let entry_width = entry.swatch.width + SWATCH_GAP + entry.label.width();
            if row_used && x + entry_width > right {
                row += 1;
                x = PADDING;
            }
            let top = row_top(row, row_height);
            entry.row = row;
            entry.swatch.x = x;
            entry.swatch.center_y = top + row_height / 2.0;
            entry.label.x = x + entry.swatch.width + SWATCH_GAP;
            entry.label.y = baseline_in_line(top, row_height, ascent, descent);
            x += entry_width + ENTRY_GAP;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Figure, LineOptions};
    use crate::pipeline::plan_figure;
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
        assert_eq!(swatch_extent(f64::NAN), 0.0);
        assert_eq!(swatch_extent(-1.0), 0.0);
        assert_eq!(swatch_extent(1e300), MAX_SWATCH);
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

    #[test]
    fn huge_swatch_tokens_stay_bounded() {
        let theme = Theme::pigment()
            .with_line_width(1e30)
            .unwrap()
            .with_marker_size(1e30)
            .unwrap();
        let mut f = figure(6.0, 4.0, theme, &names(2));
        f.axes_mut()
            .unwrap()
            .scatter(
                AxisData::Numbers(vec![0.5]),
                &[1.5],
                crate::model::ScatterOptions {
                    label: "Points".into(),
                    ..crate::model::ScatterOptions::default()
                },
            )
            .unwrap();
        let layout = plan_figure(&f, FontLibrary::global()).1;
        for entry in layout.legend.entries() {
            assert!(entry.swatch.width <= MAX_SWATCH && entry.swatch.height <= MAX_SWATCH);
        }
        assert!(layout.plot.width >= 1.0 && layout.plot.height >= 1.0);
        assert_scene_bounded(&f.to_scene(FontLibrary::global()));
    }
}
