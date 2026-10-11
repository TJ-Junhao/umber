//! The legend model: one entry per labelled series, with its swatch, and for each legend
//! style which entries go to the legend row and which become direct labels.
//!
//! The model reads series through [`SeriesKey`], never through the marks themselves, so a new
//! mark kind reaches the legend through its keys alone. Direct labels are the one exception:
//! they belong to line marks, whose data gives the point each label is attached to.

use crate::model::{AxisData, LineMark};
use crate::resolve::{AxisLimits, ResolvedMark, SeriesKey, SeriesKind};
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

/// The x value of a line's point, as the line holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AnchorX<'a> {
    /// A number on a linear or log axis; finite.
    Number(f64),
    /// A category name.
    Category(&'a str),
}

/// The point of a line that its direct label is attached to: the last point whose x and y are
/// finite and whose x lies inside the x domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Anchor<'a> {
    /// The x value.
    pub x: AnchorX<'a>,
    /// The y value; finite.
    pub y: f64,
}

impl<'a> Anchor<'a> {
    /// The anchor of `line`: its last point with finite x and y whose x lies inside
    /// `x_range` (both ends included), or inside the x domain when `x_range` is `None`. `None`
    /// when the line has no such point.
    ///
    /// Only explicit limits can leave a finite point outside the x domain: auto limits always
    /// contain every finite x value, and a category axis holds every category of its marks. So
    /// `x_range` is the explicit limits when they are set, and the anchor is the same in every
    /// layout pass.
    fn of(line: &'a LineMark, x_range: Option<(f64, f64)>) -> Option<Anchor<'a>> {
        let y = line.y();
        match line.x() {
            AxisData::Numbers(xs) => xs
                .iter()
                .zip(y)
                .rev()
                .find(|&(&x, &y)| {
                    x.is_finite()
                        && y.is_finite()
                        && x_range.is_none_or(|(lo, hi)| lo <= x && x <= hi)
                })
                .map(|(&x, &y)| Anchor {
                    x: AnchorX::Number(x),
                    y,
                }),
            AxisData::Categories(names) => names
                .iter()
                .zip(y)
                .rev()
                .find(|&(_, &y)| y.is_finite())
                .map(|(name, &y)| Anchor {
                    x: AnchorX::Category(name),
                    y,
                }),
        }
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
    /// theme's. `x` is the x axis's limits, which decide which points of a line can carry its
    /// direct label.
    ///
    /// - `Header` gives one row entry per labelled series, in series order, drawn
    ///   [`RowStyle::Plain`]. Unlabelled series are skipped; duplicate labels are kept.
    /// - `Chips` gives the same entries, drawn [`RowStyle::Chips`].
    /// - `Direct` gives a direct label to every labelled line that has an [`Anchor`], and puts
    ///   every other labelled series, lines without an anchor included, into a row drawn
    ///   [`RowStyle::Plain`], exactly as `Header` would show them. So every labelled series is
    ///   labelled somewhere.
    /// - `None` gives nothing.
    pub fn new(style: LegendStyle, marks: &[ResolvedMark<'a>], x: &AxisLimits) -> Legend<'a> {
        let entries = || {
            marks
                .iter()
                .flat_map(ResolvedMark::keys)
                .filter_map(|key| LegendEntry::of(&key))
                .collect()
        };
        let (row, row_style, direct) = match style {
            LegendStyle::Header => (entries(), RowStyle::Plain, Vec::new()),
            LegendStyle::Chips => (entries(), RowStyle::Chips, Vec::new()),
            LegendStyle::Direct => {
                let (row, direct) = direct_split(marks, x);
                (row, RowStyle::Plain, direct)
            }
            LegendStyle::None => (Vec::new(), RowStyle::Plain, Vec::new()),
        };
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

/// The row entries and the direct labels of `marks` in the `Direct` style, each in series
/// order: a labelled line with an anchor inside the domain of an x axis limited by `x` becomes
/// a direct label, and every other labelled series a row entry.
fn direct_split<'a>(
    marks: &[ResolvedMark<'a>],
    x: &AxisLimits,
) -> (Vec<LegendEntry>, Vec<DirectEntry<'a>>) {
    let x_range = match x {
        AxisLimits::Continuous {
            explicit: Some(scale),
            ..
        } => Some(scale.domain()),
        AxisLimits::Continuous { explicit: None, .. } | AxisLimits::Category => None,
    };
    let (mut row, mut direct) = (Vec::new(), Vec::new());
    for mark in marks {
        // Only a line mark can carry a direct label; it has a single series.
        let anchor = match *mark {
            ResolvedMark::Line { mark: line, .. } => Anchor::of(line, x_range),
            _ => None,
        };
        for entry in mark.keys().iter().filter_map(LegendEntry::of) {
            match anchor {
                Some(anchor) => direct.push(DirectEntry {
                    label: entry.label,
                    color: entry.color,
                    anchor,
                }),
                None => row.push(entry),
            }
        }
    }
    (row, direct)
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
        Legend::new(style, &marks, &Limits::of(axes).x)
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
        assert_eq!(target.anchor().x, AnchorX::Category("B"));
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

    /// The anchor of the one line of `f`, if it gets a direct label.
    fn anchor_of(f: &Figure) -> Option<Anchor<'_>> {
        let legend = legend_of(f, LegendStyle::Direct);
        assert!(legend.direct().len() + legend.row().len() == 1);
        legend.direct().first().map(DirectEntry::anchor)
    }

    fn at(x: f64, y: f64) -> Option<Anchor<'static>> {
        Some(Anchor {
            x: AnchorX::Number(x),
            y,
        })
    }

    #[test]
    fn anchor_is_last_visible_finite_point() {
        let nan = f64::NAN;
        let xs = |v: &[f64]| AxisData::Numbers(v.to_vec());
        // The last point.
        let f = line_figure(xs(&[0.0, 1.0, 2.0]), &[5.0, 6.0, 7.0], None);
        assert_eq!(anchor_of(&f), at(2.0, 7.0));
        // A NaN tail in y, and a NaN x, are skipped.
        let f = line_figure(xs(&[0.0, 1.0, 2.0, nan]), &[5.0, 6.0, nan, 8.0], None);
        assert_eq!(anchor_of(&f), at(1.0, 6.0));
        // Unsorted x: the last point in data order, not the largest x.
        let f = line_figure(xs(&[3.0, 1.0]), &[5.0, 6.0], None);
        assert_eq!(anchor_of(&f), at(1.0, 6.0));
        // Explicit limits cut the end: the last point inside them, ends included.
        let (x4, y4) = (xs(&[0.0, 1.0, 2.0, 3.0]), [5.0, 6.0, 7.0, 8.0]);
        let f = line_figure(x4.clone(), &y4, Some((0.0, 2.0)));
        assert_eq!(anchor_of(&f), at(2.0, 7.0));
        let f = line_figure(x4, &y4, Some((0.5, 1.5)));
        assert_eq!(anchor_of(&f), at(1.0, 6.0));
        // A category line: the last category with a finite y.
        let names = AxisData::Categories(["Jan", "Feb", "Mar"].map(String::from).to_vec());
        let f = line_figure(names, &[1.0, 2.0, nan], None);
        let feb = Anchor {
            x: AnchorX::Category("Feb"),
            y: 2.0,
        };
        assert_eq!(anchor_of(&f), Some(feb));
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
            assert!(Legend::new(style, &[], &limits.x).is_empty());
        }
    }
}
