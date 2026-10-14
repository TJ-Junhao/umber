//! The legend model: one entry per labelled series, with its swatch, and for each legend
//! style which entries go to the legend row and which become direct labels.
//!
//! Per-panel entries read [`SeriesKey`], including the anchors of direct labels. The merged
//! row reads [`ResolvedMark::legend_swatch`] without computing anchors. Resolve owns both
//! descriptions, so the legend model never inspects raw mark data.

use std::collections::HashSet;

use crate::model::Figure;
use crate::resolve::{Anchor, Limits, ResolvedMark, SeriesKey, SeriesKind};
use crate::{Dash, LegendStyle, Rgba};

/// The small figure drawn before a legend label, showing how its series is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Swatch {
    /// A short line in the series dash and width.
    Line {
        /// The dash style.
        dash: Dash,
        /// The stroke width the series is drawn at, in points.
        width: f32,
    },
    /// A dot, for point series.
    Dot {
        /// The marker diameter the series is drawn at, in points.
        diameter: f32,
    },
    /// A square, for filled bars.
    Square,
    /// A short bar with a cap at each end, for error bars.
    ErrorBar {
        /// Whether the bar runs along y; otherwise along x.
        vertical: bool,
        /// The stroke width the series is drawn at, in points.
        width: f32,
    },
}

impl Swatch {
    /// The swatch of a series drawn as `kind`: a line in its dash and width, a dot of its
    /// marker diameter, a square, or a bar with caps.
    fn of(kind: SeriesKind) -> Swatch {
        match kind {
            SeriesKind::Line { dash, width } | SeriesKind::Rule { dash, width } => {
                Swatch::Line { dash, width }
            }
            SeriesKind::Points { size, .. } => Swatch::Dot { diameter: size },
            SeriesKind::Bar | SeriesKind::Span => Swatch::Square,
            SeriesKind::ErrorBar { vertical, width } => Swatch::ErrorBar { vertical, width },
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
    /// An entry for a non-empty label; `None` for an unlabelled series.
    pub fn new(label: &str, color: Rgba, swatch: Swatch) -> Option<LegendEntry> {
        (!label.is_empty()).then(|| LegendEntry {
            label: label.to_owned(),
            color,
            swatch,
        })
    }

    /// The entry of `key`; `None` when the series is unlabelled.
    fn of(key: &SeriesKey<'_>) -> Option<LegendEntry> {
        Self::new(key.label, key.color, Swatch::of(key.kind))
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
    ///   line with a point to label) and whose kind takes one
    ///   ([`SeriesKind::takes_direct_label`]), and puts every other labelled series, lines
    ///   without an anchor, reference lines and reference spans included, into a row drawn
    ///   [`RowStyle::Plain`], exactly as `Header` would show them. So every labelled series is
    ///   labelled somewhere.
    /// - `None` gives nothing.
    pub fn new(style: LegendStyle, marks: &[ResolvedMark<'a>], limits: &Limits) -> Legend<'a> {
        Self::panel(style, marks, limits, false)
    }

    /// The panel's legend. With a merged figure legend, only direct series remain here,
    /// including the row fallback for a direct series without a visible anchor.
    pub fn panel(
        style: LegendStyle,
        marks: &[ResolvedMark<'a>],
        limits: &Limits,
        merged: bool,
    ) -> Legend<'a> {
        let keys = marks
            .iter()
            .flat_map(|mark| mark.keys(limits))
            .filter(|key| {
                !merged || (style == LegendStyle::Direct && key.kind.takes_direct_label())
            });
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
                (LegendStyle::Direct, Some(anchor)) if key.kind.takes_direct_label() => {
                    direct.push(DirectEntry {
                        label: entry.label,
                        color: entry.color,
                        anchor,
                    });
                }
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

    /// The figure's row entries in panel and series order, with one entry per label and swatch
    /// kind. The first swatch wins, including its color and stroke or marker size. Direct
    /// line-like series remain in their panels; hidden legends contribute no entries.
    pub fn merged<'m>(
        figure: &Figure,
        marks: impl IntoIterator<Item = &'m [ResolvedMark<'a>]>,
    ) -> Legend<'a>
    where
        'a: 'm,
    {
        let mut seen = HashSet::new();
        let mut row = Vec::new();
        for (axes, marks) in figure.panels().zip(marks) {
            let style = axes.legend_style().unwrap_or(figure.theme().legend_style());
            if style == LegendStyle::None {
                continue;
            }
            for entry in marks
                .iter()
                .filter(|mark| style != LegendStyle::Direct || mark.line_width().is_none())
                .flat_map(ResolvedMark::legend_swatch)
            {
                let key = (entry.label.clone(), std::mem::discriminant(&entry.swatch));
                if seen.insert(key) {
                    row.push(entry);
                }
            }
        }
        Legend {
            row,
            row_style: if figure.theme().legend_style() == LegendStyle::Chips {
                RowStyle::Chips
            } else {
                RowStyle::Plain
            },
            direct: Vec::new(),
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
        let axes = f.single_axes();
        let marks = resolve_marks(axes.marks(), f.theme());
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
        // Unlabelled colors follow all labels, and repeated labels share a slot.
        assert_eq!(colors, [palette[0], palette[1], palette[0]]);
        assert!(!legend.is_empty());
    }

    /// The line swatch of a pigment line in `dash` at the theme's width.
    fn pigment_line(dash: Dash) -> Swatch {
        Swatch::Line { dash, width: 2.0 }
    }

    /// The dot swatch of a pigment scatter at the theme's marker size.
    const PIGMENT_DOT: Swatch = Swatch::Dot { diameter: 6.0 };

    #[test]
    fn swatch_follows_series_kind() {
        let f = figure();
        let legend = legend_of(&f, LegendStyle::Header);
        let swatches: Vec<Swatch> = legend.row().iter().map(LegendEntry::swatch).collect();
        assert_eq!(
            swatches,
            [
                pigment_line(Dash::Solid),
                PIGMENT_DOT,
                pigment_line(Dash::Dashed),
            ]
        );
        assert_eq!(Swatch::of(SeriesKind::Bar), Swatch::Square);
        assert_eq!(
            Swatch::of(SeriesKind::Points {
                marker: Marker::Cross,
                size: 4.5,
            }),
            Swatch::Dot { diameter: 4.5 }
        );
    }

    #[test]
    fn swatches_take_the_width_and_size_of_their_series() {
        use crate::SizeSpec;
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                nums(&[0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    width: SizeSpec::times(2.5).unwrap(),
                    ..line("Thick", Dash::Dashed)
                },
            )
            .unwrap()
            .area(
                nums(&[0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    width: SizeSpec::pt(0.5).unwrap(),
                    ..line("Thin area", Dash::Solid)
                },
            )
            .unwrap()
            .scatter(
                nums(&[0.5]),
                &[1.5],
                ScatterOptions {
                    size: SizeSpec::times(0.5).unwrap(),
                    ..scatter("Small")
                },
            )
            .unwrap()
            .scatter(
                nums(&[0.5]),
                &[1.5],
                ScatterOptions {
                    size: SizeSpec::pt(12.0).unwrap(),
                    ..scatter("Large")
                },
            )
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[2.0, 3.0], line("Auto", Dash::Solid))
            .unwrap();
        // Graphite lines are 1.8 pt wide and its markers 5.6 pt across.
        let header = legend_of(&f, LegendStyle::Header);
        let swatches: Vec<Swatch> = header.row().iter().map(LegendEntry::swatch).collect();
        assert_eq!(
            swatches,
            [
                Swatch::Line {
                    dash: Dash::Dashed,
                    width: 2.5 * 1.8,
                },
                Swatch::Line {
                    dash: Dash::Solid,
                    width: 0.5,
                },
                Swatch::Dot { diameter: 2.8 },
                Swatch::Dot { diameter: 12.0 },
                Swatch::Line {
                    dash: Dash::Solid,
                    width: 1.8,
                },
            ]
        );
        // Chips show the same entries; the row layout draws them all as dots of one size.
        assert_eq!(legend_of(&f, LegendStyle::Chips).row(), header.row());
        // Direct labels carry no swatch; the scatters go to the row with their own sizes.
        let direct = legend_of(&f, LegendStyle::Direct);
        let row: Vec<Swatch> = direct.row().iter().map(LegendEntry::swatch).collect();
        assert_eq!(
            row,
            [
                Swatch::Dot { diameter: 2.8 },
                Swatch::Dot { diameter: 12.0 }
            ]
        );
    }

    #[test]
    fn areas_show_their_line_swatch_and_label_directly() {
        use crate::model::AreaOptions;
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .area(
                nums(&[0.0, 1.0]),
                &[1.0, 2.0],
                AreaOptions {
                    label: "Load".to_owned(),
                    dash: Dash::Dashed,
                    ..AreaOptions::default()
                },
            )
            .unwrap()
            .scatter(nums(&[0.5]), &[1.5], scatter("Samples"))
            .unwrap();
        let palette = f.theme().palette();
        // Header and chips: the area's entry is the line swatch in its dash and color.
        for style in [LegendStyle::Header, LegendStyle::Chips] {
            let legend = legend_of(&f, style);
            let entries: Vec<(&str, Rgba, Swatch)> = legend
                .row()
                .iter()
                .map(|e| (e.label(), e.color(), e.swatch()))
                .collect();
            assert_eq!(
                entries,
                [
                    ("Load", palette[0], pigment_line(Dash::Dashed)),
                    ("Samples", palette[1], PIGMENT_DOT),
                ]
            );
        }
        // Direct: the area is labelled at its line's last point; the scatter goes to the row.
        let legend = legend_of(&f, LegendStyle::Direct);
        let [load] = legend.direct() else {
            panic!("one direct label");
        };
        assert_eq!((load.label(), load.color()), ("Load", palette[0]));
        assert_eq!(load.anchor().x, crate::resolve::AnchorX::Number(1.0));
        assert_eq!(load.anchor().y, 2.0);
        let labels: Vec<&str> = legend.row().iter().map(LegendEntry::label).collect();
        assert_eq!(labels, ["Samples"]);
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
    fn series_get_square_swatches_in_series_order() {
        use crate::model::Series;
        let series = |labels: [&str; 3]| -> Vec<Series> {
            labels
                .map(|label| Series {
                    label: label.to_owned(),
                    values: vec![1.0, 2.0],
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
        // Label slots come first; the unlabelled series follows them.
        let palette = f.theme().palette();
        let colors: Vec<Rgba> = legend.row().iter().map(LegendEntry::color).collect();
        assert_eq!(colors, palette[..5]);
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
        assert_eq!(direct, [("North", palette[0]), ("North", palette[0])]);
        // The scatter goes to a row exactly as the header row shows it.
        let header = legend_of(&f, LegendStyle::Header);
        assert_eq!(legend.row_style(), RowStyle::Plain);
        assert_eq!(legend.row(), &header.row()[1..2]);
        assert_eq!(legend.row()[0].swatch(), PIGMENT_DOT);
        assert!(!legend.is_empty());

        // Bars go to the row too, with their square swatches, between the direct lines.
        use crate::model::{BarOptions, Series};
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
                vec![Series {
                    label: "North".to_owned(),
                    values: vec![1.0, 2.0],
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
        let mut axes = f.add_axes(kind, ScaleKind::Linear).unwrap();
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
            assert_eq!(
                legend.row()[0].swatch(),
                Swatch::Line {
                    dash: Dash::Solid,
                    width: 1.8
                }
            );
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
        let limits = Limits::of(f.single_axes());
        for style in [LegendStyle::Header, LegendStyle::Direct] {
            assert!(Legend::new(style, &[], &limits).is_empty());
        }
    }

    #[test]
    fn reference_lines_and_spans_take_a_row_entry_under_every_style_with_direct_labels_beside() {
        use crate::model::{AxisValue, RuleOptions, SpanOptions};
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(nums(&[0.0, 10.0]), &[1.0, 2.0], line("Signal", Dash::Solid))
            .unwrap()
            .hline(
                1.5,
                RuleOptions {
                    label: "Threshold".into(),
                    dash: Dash::Dashed,
                    ..RuleOptions::default()
                },
            )
            .unwrap()
            .vline(
                AxisValue::Number(5.0),
                RuleOptions {
                    label: "Event".into(),
                    ..RuleOptions::default()
                },
            )
            .unwrap()
            .hspan(
                1.0,
                1.2,
                SpanOptions {
                    label: "Range".into(),
                    ..SpanOptions::default()
                },
            )
            .unwrap()
            .vspan(
                2.0,
                3.0,
                SpanOptions {
                    label: "Window".into(),
                    ..SpanOptions::default()
                },
            )
            .unwrap()
            .hline(1.7, RuleOptions::default())
            .unwrap();
        let row_labels = |legend: &Legend<'_>| {
            legend
                .row()
                .iter()
                .map(|entry| entry.label().to_owned())
                .collect::<Vec<_>>()
        };
        // Direct: the line is labelled at its end; every reference mark is in the row.
        let legend = legend_of(&f, LegendStyle::Direct);
        let direct: Vec<&str> = legend.direct().iter().map(DirectEntry::label).collect();
        assert_eq!(direct, ["Signal"]);
        assert_eq!(
            row_labels(&legend),
            ["Threshold", "Event", "Range", "Window"]
        );
        // Header and Chips: all five in series order, and the same swatches under Direct.
        for style in [LegendStyle::Header, LegendStyle::Chips] {
            let legend = legend_of(&f, style);
            assert_eq!(
                row_labels(&legend),
                ["Signal", "Threshold", "Event", "Range", "Window"]
            );
        }
        let legend = legend_of(&f, LegendStyle::Direct);
        let theme = f.theme();
        let w = theme.drawn_line_width(crate::SizeSpec::Auto);
        let swatches: Vec<Swatch> = legend.row().iter().map(LegendEntry::swatch).collect();
        assert_eq!(
            swatches,
            [
                Swatch::Line {
                    dash: Dash::Dashed,
                    width: w
                },
                Swatch::Line {
                    dash: Dash::Solid,
                    width: w
                },
                Swatch::Square,
                Swatch::Square,
            ]
        );
        assert!(legend_of(&f, LegendStyle::None).is_empty());
    }

    #[test]
    fn a_band_takes_a_square_row_entry_under_every_style() {
        use crate::model::BandOptions;
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .band(
                &[0.0, 10.0],
                &[0.5, 1.5],
                &[1.5, 2.5],
                BandOptions {
                    label: "95% interval".into(),
                    ..BandOptions::default()
                },
            )
            .unwrap()
            .line(nums(&[0.0, 10.0]), &[1.0, 2.0], line("Mean", Dash::Solid))
            .unwrap();
        // Direct: the line is labelled at its end; the band is in the row.
        let legend = legend_of(&f, LegendStyle::Direct);
        let direct: Vec<&str> = legend.direct().iter().map(DirectEntry::label).collect();
        assert_eq!(direct, ["Mean"]);
        let row: Vec<(&str, Swatch)> = legend
            .row()
            .iter()
            .map(|entry| (entry.label(), entry.swatch()))
            .collect();
        assert_eq!(row, [("95% interval", Swatch::Square)]);
        for style in [LegendStyle::Header, LegendStyle::Chips] {
            let legend = legend_of(&f, style);
            let labels: Vec<&str> = legend.row().iter().map(LegendEntry::label).collect();
            assert_eq!(labels, ["95% interval", "Mean"]);
        }
    }

    #[test]
    fn error_bars_take_a_bar_row_entry_under_every_style() {
        use crate::model::ErrorBarOptions;
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .y_error_bars(
                nums(&[0.0, 10.0]),
                &[0.5, 1.5],
                &[1.5, 2.5],
                ErrorBarOptions {
                    label: "y interval".into(),
                    ..ErrorBarOptions::default()
                },
            )
            .unwrap()
            .x_error_bars(
                &[1.0, 2.0],
                &[0.0, 1.0],
                &[2.0, 3.0],
                ErrorBarOptions {
                    label: "x interval".into(),
                    ..ErrorBarOptions::default()
                },
            )
            .unwrap()
            .line(nums(&[0.0, 10.0]), &[1.0, 2.0], line("Mean", Dash::Solid))
            .unwrap();
        let bar = |vertical| Swatch::ErrorBar {
            vertical,
            width: 1.8,
        };
        // Direct: the line is labelled at its end; the bars are in the row, never direct.
        let legend = legend_of(&f, LegendStyle::Direct);
        let direct: Vec<&str> = legend.direct().iter().map(DirectEntry::label).collect();
        assert_eq!(direct, ["Mean"]);
        let row: Vec<(&str, Swatch)> = legend
            .row()
            .iter()
            .map(|entry| (entry.label(), entry.swatch()))
            .collect();
        assert_eq!(row, [("y interval", bar(true)), ("x interval", bar(false))]);
        for style in [LegendStyle::Header, LegendStyle::Chips] {
            let legend = legend_of(&f, style);
            let labels: Vec<&str> = legend.row().iter().map(LegendEntry::label).collect();
            assert_eq!(labels, ["y interval", "x interval", "Mean"]);
        }
        assert!(legend_of(&f, LegendStyle::None).is_empty());
    }
}
