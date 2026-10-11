//! The legend model: one entry per labelled series, with its swatch, and for each legend
//! style which entries go to the legend row and which become direct labels.
//!
//! The model reads series through [`SeriesKey`], never through the marks themselves, so a new
//! mark kind reaches the legend through its keys alone.

use crate::resolve::{ResolvedMark, SeriesKey, SeriesKind};
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

/// The legend of one axes: the entries of its legend row, in series order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Legend {
    row: Vec<LegendEntry>,
}

impl Legend {
    /// The legend of `marks` in the effective `style`: the style set on the axes, else the
    /// theme's.
    ///
    /// - `Header` gives one row entry per labelled series, in series order. Unlabelled series
    ///   are skipped; duplicate labels are kept.
    /// - `None` gives nothing.
    /// - `Chips` and `Direct` give nothing yet; these styles do not draw entries.
    pub fn new(style: LegendStyle, marks: &[ResolvedMark<'_>]) -> Legend {
        let row = match style {
            LegendStyle::Header => marks
                .iter()
                .flat_map(ResolvedMark::keys)
                .filter_map(|key| LegendEntry::of(&key))
                .collect(),
            LegendStyle::Chips | LegendStyle::Direct | LegendStyle::None => Vec::new(),
        };
        Legend { row }
    }

    /// The entries of the legend row, in series order.
    pub fn row(&self) -> &[LegendEntry] {
        &self.row
    }

    /// Whether the legend shows nothing.
    pub fn is_empty(&self) -> bool {
        self.row.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Figure, LineOptions, ScatterOptions};
    use crate::resolve::resolve_marks;
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

    fn legend_of(f: &Figure, style: LegendStyle) -> Legend {
        let marks = resolve_marks(f.axes().unwrap().marks(), f.theme().palette());
        Legend::new(style, &marks)
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
        let legend = legend_of(&figure(), LegendStyle::Header);
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
    fn other_styles_have_no_row_entries() {
        let f = figure();
        for style in [LegendStyle::None, LegendStyle::Chips, LegendStyle::Direct] {
            assert!(legend_of(&f, style).is_empty(), "{style:?}");
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
        assert!(Legend::new(LegendStyle::Header, &[]).is_empty());
    }
}
