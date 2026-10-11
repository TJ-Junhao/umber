//! The legend model: one entry per labelled series, with its swatch, and for each legend
//! style which entries go to the legend row and which become direct labels.
//!
//! The model reads series through [`SeriesKey`], never through the marks themselves, so a new
//! mark kind reaches the legend through its keys alone. A key's anchor, the point a direct
//! label is attached to, is found by resolve as well.

use crate::resolve::{Anchor, Limits, ResolvedMark, SeriesKey, SeriesKind};
use crate::{Dash, LegendStyle, Rgba};

/// The small figure drawn before a legend label, showing how its series is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Swatch {
    /// A short line in the series dash.
    Line {
        /// The dash style.
        dash: Dash,
    },
    /// A dot, for point series.
    Dot,
    /// A square, for filled bars.
    Square,
}

impl Swatch {
    /// The swatch of a series drawn as `kind`.
    fn of(kind: SeriesKind) -> Swatch {
        match kind {
            SeriesKind::Line { dash } => Swatch::Line { dash },
            SeriesKind::Points { .. } => Swatch::Dot,
            SeriesKind::Bar => Swatch::Square,
        }
    }
}

/// One legend entry: a non-empty label with its series color and swatch.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LegendEntry {
    label: String,
    color: Rgba,
    swatch: Swatch,
}

impl LegendEntry {
    /// The entry of `key`; `None` when the series is unlabelled.
    fn of(key: &SeriesKey<'_>) -> Option<LegendEntry> {
        (!key.label.is_empty()).then(|| LegendEntry {
            label: key.label.to_owned(),
            color: key.color,
            swatch: Swatch::of(key.kind),
        })
    }

    /// The label; never empty.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The resolved series color, alpha included.
    pub fn color(&self) -> Rgba {
        self.color
    }

    /// The swatch.
    pub fn swatch(&self) -> Swatch {
        self.swatch
    }
}

/// One direct label: a non-empty label, in its line's color, attached to an anchor of the line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DirectEntry<'a> {
    label: String,
    color: Rgba,
    anchor: Anchor<'a>,
}

impl<'a> DirectEntry<'a> {
    /// The label; never empty.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The resolved line color, alpha included.
    pub fn color(&self) -> Rgba {
        self.color
    }

    /// The point of the line the label is attached to.
    pub fn anchor(&self) -> Anchor<'a> {
        self.anchor
    }
}

/// How the entries of a legend row are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowStyle {
    /// Each entry is its swatch and its label.
    Plain,
    /// Each entry is a tinted pill holding a dot and the label.
    Chips,
}

/// The legend of one axes: the entries of its legend row, in series order, how the row draws
/// them, and the direct labels, in series order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Legend<'a> {
    row: Vec<LegendEntry>,
    row_style: RowStyle,
    direct: Vec<DirectEntry<'a>>,
}

impl<'a> Legend<'a> {
    /// The legend of `marks` in the effective `style`: the style set on the axes, else the
    /// theme's. `limits` are the axes' limits, which decide where a line's direct label is
    /// attached.
    ///
    /// - `Header` gives one row entry per labelled series, in series order, drawn
    ///   [`RowStyle::Plain`]. Unlabelled series are skipped; duplicate labels are kept.
    /// - `Chips` gives the same entries, drawn [`RowStyle::Chips`].
    /// - `Direct` gives a direct label to every labelled series whose key has an [`Anchor`] (a
    ///   line with a point to label), and puts every other labelled series, lines without an
    ///   anchor included, into a row drawn [`RowStyle::Plain`], exactly as `Header` would show
    ///   them. So every labelled series is labelled somewhere.
    /// - `None` gives nothing.
    pub fn new(style: LegendStyle, marks: &[ResolvedMark<'a>], limits: &Limits) -> Legend<'a> {
        let keys = marks.iter().flat_map(|mark| mark.keys(limits));
        let (mut row, mut direct) = (Vec::new(), Vec::new());
        let row_style = match style {
            LegendStyle::Header | LegendStyle::Direct | LegendStyle::None => RowStyle::Plain,
            LegendStyle::Chips => RowStyle::Chips,
        };
        for key in keys {
            let Some(entry) = LegendEntry::of(&key) else {
                continue;
            };
            match (style, key.anchor) {
                (LegendStyle::None, _) => {}
                (LegendStyle::Direct, Some(anchor)) => direct.push(DirectEntry {
                    label: entry.label,
                    color: entry.color,
                    anchor,
                }),
                (LegendStyle::Header | LegendStyle::Chips | LegendStyle::Direct, _) => {
                    row.push(entry);
                }
            }
        }
        Legend {
            row,
            row_style,
            direct,
        }
    }

    /// The entries of the legend row, in series order.
    pub fn row(&self) -> &[LegendEntry] {
        &self.row
    }

    /// How the row draws its entries.
    pub fn row_style(&self) -> RowStyle {
        self.row_style
    }

    /// The direct labels, in series order.
    pub fn direct(&self) -> &[DirectEntry<'a>] {
        &self.direct
    }

    /// Whether the legend shows nothing: no row entry and no direct label.
    pub fn is_empty(&self) -> bool {
        self.row.is_empty() && self.direct.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Figure, LineOptions, ScatterOptions};
    use crate::resolve::{Limits, resolve_marks};
    use crate::{ColorSpec, Marker, ScaleKind, Theme};

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn line(label: &str, dash: Dash) -> LineOptions {
        LineOptions {
            label: label.to_owned(),
            dash,
            ..LineOptions::default()
        }
    }

    fn scatter(label: &str) -> ScatterOptions {
        ScatterOptions {
            label: label.to_owned(),
            marker: Marker::Triangle,
            ..ScatterOptions::default()
        }
    }

    /// A figure with a labelled solid line, an unlabelled line, a labelled scatter, and a
    /// dashed line whose label repeats the first.
    fn figure() -> Figure {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[1.0, 2.0], line("North", Dash::Solid))
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[2.0, 3.0], line("", Dash::Solid))
            .unwrap()
            .scatter(nums(&[0.5]), &[1.5], scatter("Samples"))
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[3.0, 1.0], line("North", Dash::Dashed))
            .unwrap();
        f
    }

    fn legend_of(f: &Figure, style: LegendStyle) -> Legend<'_> {
        let axes = f.axes().unwrap();
        let marks = resolve_marks(axes.marks(), f.theme().palette());
        Legend::new(style, &marks, &Limits::of(axes))
    }

    #[test]
    fn legend_shows_only_labelled_series() {
        let f = figure();
        let palette = f.theme().palette();
        let legend = legend_of(&f, LegendStyle::Header);
        let labels: Vec<&str> = legend.row().iter().map(LegendEntry::label).collect();
        // Series order; the unlabelled line is skipped and the repeated label is kept.
        assert_eq!(labels, ["North", "Samples", "North"]);
        let colors: Vec<Rgba> = legend.row().iter().map(LegendEntry::color).collect();
        // The unlabelled line still takes its palette slot.
        assert_eq!(colors, [palette[0], palette[2], palette[3]]);
        assert!(!legend.is_empty());
    }

    #[test]
    fn swatch_follows_series_kind() {
        let f = figure();
        let legend = legend_of(&f, LegendStyle::Header);
        let swatches: Vec<Swatch> = legend.row().iter().map(LegendEntry::swatch).collect();
        assert_eq!(
            swatches,
            [
                Swatch::Line { dash: Dash::Solid },
                Swatch::Dot,
                Swatch::Line { dash: Dash::Dashed },
            ]
        );
        assert_eq!(Swatch::of(SeriesKind::Bar), Swatch::Square);
        assert_eq!(
            Swatch::of(SeriesKind::Points {
                marker: Marker::Cross
            }),
            Swatch::Dot
        );
    }

    #[test]
    fn swatch_colors_keep_their_alpha() {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let color = ColorSpec::Fixed(Rgba {
            r: 10,
            g: 20,
            b: 30,
            a: 128,
        });
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                nums(&[0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: "Faded".to_owned(),
                    color,
                    ..LineOptions::default()
                },
            )
            .unwrap();
        let legend = legend_of(&f, LegendStyle::Header);
        assert_eq!(legend.row().first().map(|e| e.color().a), Some(128));
    }

    #[test]
    fn bar_series_get_square_swatches_in_series_order() {
        use crate::model::BarSeries;
        let series = |labels: [&str; 3]| -> Vec<BarSeries> {
            labels
                .map(|label| BarSeries {
                    label: label.to_owned(),
                    heights: vec![1.0, 2.0],
                    color: ColorSpec::AUTO,
                })
                .to_vec()
        };
        let categories = || vec!["A".to_owned(), "B".to_owned()];
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .stacked_bar(categories(), series(["2024", "", "2026"]))
            .unwrap()
            .grouped_bar(categories(), series(["North", "South", "East"]))
            .unwrap();
        let legend = legend_of(&f, LegendStyle::Header);
        let labels: Vec<&str> = legend.row().iter().map(LegendEntry::label).collect();
        assert_eq!(labels, ["2024", "2026", "North", "South", "East"]);
        assert!(legend.row().iter().all(|e| e.swatch() == Swatch::Square));
        // Every series takes a palette slot, the unlabelled one included.
        let palette = f.theme().palette();
        let colors: Vec<Rgba> = legend.row().iter().map(LegendEntry::color).collect();
        assert_eq!(
            colors,
            [palette[0], palette[2], palette[3], palette[4], palette[5]]
        );
    }

    #[test]
    fn none_shows_nothing() {
        let f = figure();
        let legend = legend_of(&f, LegendStyle::None);
        assert!(legend.is_empty());
        assert!(legend.row().is_empty() && legend.direct().is_empty());
    }

    #[test]
    fn chips_show_the_header_entries_as_chips() {
        let f = figure();
        let (header, chips) = (
            legend_of(&f, LegendStyle::Header),
            legend_of(&f, LegendStyle::Chips),
        );
        assert_eq!(header.row_style(), RowStyle::Plain);
        assert_eq!(chips.row_style(), RowStyle::Chips);
        assert_eq!(chips.row(), header.row());
        assert!(!chips.is_empty());
    }

    #[test]
    fn direct_falls_back_for_non_lines() {
        let f = figure();
        let palette = f.theme().palette();
        let legend = legend_of(&f, LegendStyle::Direct);
        // The labelled lines are labelled directly, in series order, in their line colors.
        let direct: Vec<(&str, Rgba)> = legend
            .direct()
            .iter()
            .map(|e| (e.label(), e.color()))
            .collect();
        assert_eq!(direct, [("North", palette[0]), ("North", palette[3])]);
        // The scatter goes to a row exactly as the header row shows it.
        let header = legend_of(&f, LegendStyle::Header);
        assert_eq!(legend.row_style(), RowStyle::Plain);
        assert_eq!(legend.row(), &header.row()[1..2]);
        assert_eq!(legend.row()[0].swatch(), Swatch::Dot);
        assert!(!legend.is_empty());

        // Bars go to the row too, with their square swatches, between the direct lines.
        use crate::model::{BarOptions, BarSeries};
        let categories = || vec!["A".to_owned(), "B".to_owned()];
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Categories(categories()),
                &[1.0, 2.0],
                line("Target", Dash::Dashed),
            )
            .unwrap()
            .bar(
                categories(),
                &[3.0, 4.0],
                BarOptions {
                    label: "Revenue".to_owned(),
                    ..BarOptions::default()
                },
            )
            .unwrap()
            .grouped_bar(
                categories(),
                vec![BarSeries {
                    label: "North".to_owned(),
                    heights: vec![1.0, 2.0],
                    color: ColorSpec::AUTO,
                }],
            )
            .unwrap();
        let legend = legend_of(&f, LegendStyle::Direct);
        let labels: Vec<&str> = legend.row().iter().map(LegendEntry::label).collect();
        assert_eq!(labels, ["Revenue", "North"]);
        assert!(legend.row().iter().all(|e| e.swatch() == Swatch::Square));
        let [target] = legend.direct() else {
            panic!("one direct label");
        };
        assert_eq!(target.label(), "Target");
        assert_eq!(target.anchor().x, crate::resolve::AnchorX::Category("B"));
    }

    /// A figure with one labelled line with `x` and `y`, on an x axis limited to `xlim`.
    fn line_figure(x: AxisData, y: &[f64], xlim: Option<(f64, f64)>) -> Figure {
        let kind = match x {
            AxisData::Numbers(_) => ScaleKind::Linear,
            AxisData::Categories(_) => ScaleKind::Category,
        };
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        let axes = f.add_axes(kind, ScaleKind::Linear).unwrap();
        axes.line(x, y, line("Line", Dash::Solid)).unwrap();
        if let Some((lo, hi)) = xlim {
            axes.xlim(lo, hi).unwrap();
        }
        f
    }

    #[test]
    fn lines_without_an_anchor_fall_back_to_the_row() {
        let nan = f64::NAN;
        let xs = |v: &[f64]| AxisData::Numbers(v.to_vec());
        // An all-NaN line, a line without a finite x, and a line outside the explicit limits.
        for f in [
            line_figure(xs(&[0.0, 1.0]), &[nan, nan], None),
            line_figure(xs(&[nan, nan]), &[1.0, 2.0], None),
            line_figure(xs(&[0.0, 1.0]), &[1.0, 2.0], Some((5.0, 6.0))),
        ] {
            let legend = legend_of(&f, LegendStyle::Direct);
            assert!(legend.direct().is_empty());
            let labels: Vec<&str> = legend.row().iter().map(LegendEntry::label).collect();
            assert_eq!(labels, ["Line"]);
            assert_eq!(legend.row()[0].swatch(), Swatch::Line { dash: Dash::Solid });
        }
    }

    #[test]
    fn unlabelled_marks_give_an_empty_legend() {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap();
        assert!(legend_of(&f, LegendStyle::Header).is_empty());
        assert!(legend_of(&f, LegendStyle::Chips).is_empty());
        assert!(legend_of(&f, LegendStyle::Direct).is_empty());
        let limits = Limits::of(f.axes().unwrap());
        for style in [LegendStyle::Header, LegendStyle::Direct] {
            assert!(Legend::new(style, &[], &limits).is_empty());
        }
    }
}
